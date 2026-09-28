//! Adversarial benchmark measuring capacity, heap allocations, and deterministic cutoffs.

use sentry_driver::coredump::lz4_flex;
use sentry_driver::coredump::stream_reader::{read_bounded_coredump_bytes, MAX_DECOMPRESSED_BYTES};
use std::alloc::{GlobalAlloc, Layout, System};
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::tempdir;

struct AllocTracker;
static TOTAL_ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for AllocTracker {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            TOTAL_ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }
}

#[global_allocator]
static GLOBAL: AllocTracker = AllocTracker;

struct CountingReader<R> {
    inner: R,
    bytes_read: usize,
}

impl<R: Read> CountingReader<R> {
    fn new(inner: R) -> Self {
        Self { inner, bytes_read: 0 }
    }
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.bytes_read += n;
        Ok(n)
    }
}

fn build_test_elf_with_payload(payload_size: usize) -> Vec<u8> {
    let mut elf = Vec::with_capacity(512 + payload_size);
    elf.extend_from_slice(&[0x7F, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    elf.extend_from_slice(&4_u16.to_le_bytes()); // ET_CORE
    elf.extend_from_slice(&0x3E_u16.to_le_bytes()); // x86_64
    elf.extend_from_slice(&1_u32.to_le_bytes());
    elf.extend_from_slice(&0_u64.to_le_bytes());
    elf.extend_from_slice(&64_u64.to_le_bytes()); // e_phoff = 64
    elf.extend_from_slice(&0_u64.to_le_bytes());
    elf.extend_from_slice(&0_u32.to_le_bytes());
    elf.extend_from_slice(&64_u16.to_le_bytes());
    elf.extend_from_slice(&56_u16.to_le_bytes());
    elf.extend_from_slice(&1_u16.to_le_bytes()); // phnum = 1
    elf.extend_from_slice(&[0u8; 6]);

    let mut notes = Vec::new();
    let add_note = |buf: &mut Vec<u8>, ntype: u32, name: &[u8], desc: &[u8]| {
        buf.extend_from_slice(&(name.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(desc.len() as u32).to_le_bytes());
        buf.extend_from_slice(&ntype.to_le_bytes());
        buf.extend_from_slice(name);
        let npad = (name.len() + 3) & !3;
        buf.resize(buf.len() + (npad - name.len()), 0);
        buf.extend_from_slice(desc);
        let dpad = (desc.len() + 3) & !3;
        buf.resize(buf.len() + (dpad - desc.len()), 0);
    };

    let mut desc_status = vec![0u8; 336];
    desc_status[0..4].copy_from_slice(&11_i32.to_le_bytes());
    desc_status[32..36].copy_from_slice(&4242_u32.to_le_bytes());
    add_note(&mut notes, 1, b"CORE\0", &desc_status);

    let mut desc_psinfo = vec![0u8; 136];
    desc_psinfo[24..28].copy_from_slice(&4242_u32.to_le_bytes());
    let comm = b"bench_crash\0";
    desc_psinfo[40..40 + comm.len()].copy_from_slice(comm);
    add_note(&mut notes, 3, b"CORE\0", &desc_psinfo);

    elf.extend_from_slice(&4_u32.to_le_bytes()); // PT_NOTE
    elf.extend_from_slice(&0_u32.to_le_bytes());
    elf.extend_from_slice(&120_u64.to_le_bytes()); // p_offset
    elf.extend_from_slice(&0_u64.to_le_bytes());
    elf.extend_from_slice(&0_u64.to_le_bytes());
    elf.extend_from_slice(&(notes.len() as u64).to_le_bytes());
    elf.extend_from_slice(&0_u64.to_le_bytes());
    elf.extend_from_slice(&4_u64.to_le_bytes());
    elf.extend_from_slice(&notes);

    if payload_size > 0 {
        elf.resize(elf.len() + payload_size, 0x55);
    }
    elf
}

#[test]
fn test_measure_raw_elf_200mb_capacity_and_alloc() {
    let elf_core = build_test_elf_with_payload(0);
    let total_stream = 200 * 1024 * 1024;
    let chained = Cursor::new(elf_core).chain(std::io::repeat(0xBB).take(total_stream as u64));
    let mut counting = CountingReader::new(chained);

    let start_bytes = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed);
    let start_time = std::time::Instant::now();
    let out = read_bounded_coredump_bytes(&mut counting).expect("read raw stream");
    let elapsed = start_time.elapsed();
    let alloc_delta = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed).saturating_sub(start_bytes);

    println!(
        "[EMPIRICAL] RAW 200MB: len={} B, cap={} B, read={} B, alloc={} B, time={:?}",
        out.len(),
        out.capacity(),
        counting.bytes_read,
        alloc_delta,
        elapsed
    );

    assert_eq!(out.len(), MAX_DECOMPRESSED_BYTES as usize);
    assert!(
        out.capacity() <= MAX_DECOMPRESSED_BYTES as usize,
        "Capacity {} exceeded 65536",
        out.capacity()
    );
}

#[test]
fn test_measure_lz4_50mb_frame_capacity_and_alloc() {
    let uncompressed_payload_size = 50 * 1024 * 1024; // 50 MB
    let full_uncompressed = build_test_elf_with_payload(uncompressed_payload_size);

    let mut compressed_frame = Vec::new();
    {
        let mut encoder = lz4_flex::frame::FrameEncoder::new(&mut compressed_frame);
        encoder.write_all(&full_uncompressed).unwrap();
        encoder.finish().unwrap();
    }

    let compressed_len = compressed_frame.len();
    let mut counting = CountingReader::new(Cursor::new(compressed_frame));

    let start_bytes = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed);
    let start_time = std::time::Instant::now();
    let out = read_bounded_coredump_bytes(&mut counting).expect("read lz4 frame");
    let elapsed = start_time.elapsed();
    let alloc_delta = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed).saturating_sub(start_bytes);

    println!(
        "[EMPIRICAL] LZ4 50MB: len={} B, cap={} B, comp_total={} B, comp_read={} B, alloc={} B, time={:?}",
        out.len(),
        out.capacity(),
        compressed_len,
        counting.bytes_read,
        alloc_delta,
        elapsed
    );

    assert_eq!(out.len(), MAX_DECOMPRESSED_BYTES as usize);
    assert!(
        out.capacity() <= MAX_DECOMPRESSED_BYTES as usize,
        "Capacity {} exceeded 65536",
        out.capacity()
    );
    assert!(
        counting.bytes_read < compressed_len,
        "Read entire compressed frame ({} bytes)",
        counting.bytes_read
    );
}

#[test]
fn test_measure_sparse_file_500mb_capacity_and_alloc() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("sparse_500mb.core");
    let mut file = File::create(&file_path).unwrap();
    let elf = build_test_elf_with_payload(0);
    file.write_all(&elf).unwrap();
    file.set_len(500 * 1024 * 1024).unwrap();

    let mut reader = File::open(&file_path).unwrap();
    let start_bytes = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed);
    let start_time = std::time::Instant::now();
    let out = read_bounded_coredump_bytes(&mut reader).expect("read sparse file");
    let elapsed = start_time.elapsed();
    let alloc_delta = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed).saturating_sub(start_bytes);

    println!(
        "[EMPIRICAL] SPARSE 500MB: len={} B, cap={} B, alloc={} B, time={:?}",
        out.len(),
        out.capacity(),
        alloc_delta,
        elapsed
    );

    assert_eq!(out.len(), MAX_DECOMPRESSED_BYTES as usize);
    assert!(
        out.capacity() <= MAX_DECOMPRESSED_BYTES as usize,
        "Capacity {} exceeded 65536",
        out.capacity()
    );
}

#[test]
fn test_measure_zstd_capacity_and_alloc() {
    const ZSTD_HDR: &[u8] = &[
        40, 181, 47, 253, 4, 88, 149, 6, 0, 130, 136, 30, 41, 128, 197, 173, 6, 115, 9, 77, 44,
        118, 64, 163, 52, 172, 49, 134, 167, 126, 115, 116, 129, 183, 70, 21, 133, 182, 116, 5,
        165, 144, 204, 224, 18, 224, 66, 40, 221, 182, 108, 55, 153, 2, 255, 131, 232, 194, 162,
        170, 90, 43, 86, 107, 101, 48, 147, 16, 222, 72, 116, 154, 191, 253, 31, 0, 38, 19, 240,
        11, 122, 160, 127, 229, 127, 142, 75, 65, 144, 32, 70, 108, 141, 23, 147, 196, 7, 70,
        221, 138, 113, 27, 144, 123, 77, 156, 223, 242, 63, 120, 121, 254, 249, 159, 10, 132,
        225, 150, 248, 159, 115, 2, 20, 71, 88, 46, 255, 194, 238, 214, 26, 105, 154, 89, 42,
        32, 32, 131, 153, 217, 180, 1, 57, 3, 236, 248, 192, 193, 195, 130, 37, 131, 21, 229,
        144, 224, 202, 236, 146, 160, 52, 45, 147, 112, 224, 111, 173, 118, 64, 114, 169, 43, 1,
        71, 185, 112, 62, 96, 229, 23, 4, 2, 128, 51, 138, 47, 164, 10, 0, 100, 225, 1, 186,
        187, 189, 250, 101, 179, 126, 54, 44, 0, 99, 18, 8, 72, 190, 113, 102, 71, 113, 69, 132,
        121, 143, 184, 9, 1, 184, 227, 227, 178, 101,
    ];
    let chained = Cursor::new(ZSTD_HDR).chain(std::io::repeat(0xDD).take(200 * 1024 * 1024));
    let mut counting = CountingReader::new(chained);
    let start_bytes = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed);
    let start_time = std::time::Instant::now();
    let out = read_bounded_coredump_bytes(&mut counting).expect("read zstd stream");
    let elapsed = start_time.elapsed();
    let alloc_delta = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed).saturating_sub(start_bytes);

    println!(
        "[EMPIRICAL] ZSTD 200MB: len={} B, cap={} B, read={} B, alloc={} B, time={:?}",
        out.len(), out.capacity(), counting.bytes_read, alloc_delta, elapsed
    );
    assert_eq!(out.len(), 908);
    assert!(out.capacity() <= MAX_DECOMPRESSED_BYTES as usize);
    assert_eq!(out.capacity(), 908);
}

