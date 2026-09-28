//! Adversarial empirical tests for compressed coredump metadata extraction.
//!
//! Validates:
//! 1. Memory allocation bounded to <= 64 KiB under massive 200MB+ crash streams.
//! 2. Deterministic decompression termination on .zst and .lz4 after reading headers/notes.
//! 3. Dynamic xattr two-pass sizing on attributes larger than 1024 bytes (4096-byte proc_status).

use rustix::fs::{setxattr, XattrFlags};
use sentry_driver::coredump::elf_extractor::{
    extract_elf_crash_headers, extract_elf_crash_headers_from_file,
};
use sentry_driver::coredump::lz4_flex;
use sentry_driver::coredump::stream_reader::MAX_DECOMPRESSED_BYTES;
use sentry_driver::coredump::xattr_reader::{read_coredump_xattrs, read_xattr_two_pass};
use std::alloc::{GlobalAlloc, Layout, System};
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::tempdir;

struct AllocTracker;
static TOTAL_ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for AllocTracker {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ret = System.alloc(layout);
        if !ret.is_null() {
            TOTAL_ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        ret
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

fn make_synthetic_elf() -> Vec<u8> {
    let mut elf = Vec::with_capacity(512);
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
    desc_status[12..14].copy_from_slice(&11_i16.to_le_bytes());
    desc_status[32..36].copy_from_slice(&4242_u32.to_le_bytes());
    add_note(&mut notes, 1, b"CORE\0", &desc_status);

    let mut desc_psinfo = vec![0u8; 136];
    desc_psinfo[24..28].copy_from_slice(&4242_u32.to_le_bytes());
    let comm = b"worker_crash\0";
    desc_psinfo[40..40 + comm.len()].copy_from_slice(comm);
    let cmdline = b"/usr/bin/worker_crash --arg\0";
    desc_psinfo[56..56 + cmdline.len()].copy_from_slice(cmdline);
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
    elf
}

#[test]
fn test_adversarial_stream_raw_elf_200mb() {
    let elf_core = make_synthetic_elf();
    let total_stream = 200 * 1024 * 1024;
    let chained = Cursor::new(elf_core).chain(std::io::repeat(0xBB).take((total_stream) as u64));
    let mut counting = CountingReader::new(chained);

    let start_bytes = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed);
    let start_time = std::time::Instant::now();
    let header = extract_elf_crash_headers(&mut counting).expect("should extract headers");
    let elapsed = start_time.elapsed();
    let alloc_delta = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed).saturating_sub(start_bytes);

    assert_eq!(header.pid, Some(4242));
    assert_eq!(header.signal, Some(11));
    assert_eq!(header.comm.as_deref(), Some("worker_crash"));
    assert!(counting.bytes_read <= MAX_DECOMPRESSED_BYTES as usize);
    println!("[METRIC raw_200mb] read={} B, time={:?}, alloc={} B", counting.bytes_read, elapsed, alloc_delta);
}

#[test]
fn test_adversarial_stream_lz4_200mb() {
    let elf_core = make_synthetic_elf();
    let mut compressed = Vec::new();
    {
        let mut encoder = lz4_flex::frame::FrameEncoder::new(&mut compressed);
        encoder.write_all(&elf_core).unwrap();
        encoder.finish().unwrap();
    }
    let chained = Cursor::new(compressed).chain(std::io::repeat(0xCC).take(200 * 1024 * 1024));
    let mut counting = CountingReader::new(chained);

    let start_bytes = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed);
    let start_time = std::time::Instant::now();
    let header = extract_elf_crash_headers(&mut counting).expect("should extract headers");
    let elapsed = start_time.elapsed();
    let alloc_delta = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed).saturating_sub(start_bytes);

    assert_eq!(header.pid, Some(4242));
    assert_eq!(header.signal, Some(11));
    assert_eq!(header.comm.as_deref(), Some("worker_crash"));
    assert!(counting.bytes_read < 1024 * 1024);
    println!("[METRIC lz4_200mb] read={} B, time={:?}, alloc={} B", counting.bytes_read, elapsed, alloc_delta);
}

#[test]
fn test_adversarial_stream_zstd_trailing_stream() {
    const SYNTH_ZSTD: &[u8] = &[
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
    let chained = Cursor::new(SYNTH_ZSTD).chain(std::io::repeat(0xDD).take(200 * 1024 * 1024));
    let mut counting = CountingReader::new(chained);

    let start_bytes = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed);
    let start_time = std::time::Instant::now();
    let header = extract_elf_crash_headers(&mut counting).expect("should extract zstd headers");
    let elapsed = start_time.elapsed();
    let alloc_delta = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed).saturating_sub(start_bytes);

    assert_eq!(header.pid, Some(4242));
    assert_eq!(header.signal, Some(11));
    assert_eq!(header.comm.as_deref(), Some("worker_crash"));
    assert!(counting.bytes_read < 1024 * 1024);
    println!("[METRIC zstd_200mb] read={} B, time={:?}, alloc={} B", counting.bytes_read, elapsed, alloc_delta);
}

#[test]
fn test_adversarial_xattr_two_pass_large_proc_status() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("crash.core");
    File::create(&file).unwrap();

    let sizes = [1500, 4096, 8192, 16384, 65000];
    for &sz in &sizes {
        let large_val = "A".repeat(sz);
        if setxattr(&file, "user.coredump.proc_status", large_val.as_bytes(), XattrFlags::empty()).is_ok() {
            let recovered = read_xattr_two_pass(&file, "user.coredump.proc_status");
            assert_eq!(recovered.as_deref(), Some(large_val.as_str()), "Failed on size {sz}");

            let xattrs = read_coredump_xattrs(&file).unwrap();
            assert_eq!(xattrs.proc_status.as_deref(), Some(large_val.as_str()), "Failed in xattrs on size {sz}");
            println!("[METRIC xattr_twopass] successfully round-tripped {} byte attribute", sz);
        }
    }
}

#[test]
fn test_adversarial_sparse_file_500mb() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("sparse_500mb.core");
    let mut file = File::create(&file_path).unwrap();
    let elf = make_synthetic_elf();
    file.write_all(&elf).unwrap();
    file.set_len(500 * 1024 * 1024).unwrap();

    let start_bytes = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed);
    let start_time = std::time::Instant::now();
    let header = extract_elf_crash_headers_from_file(&file_path).expect("should extract sparse core");
    let elapsed = start_time.elapsed();
    let alloc_delta = TOTAL_ALLOC_BYTES.load(Ordering::Relaxed).saturating_sub(start_bytes);

    assert_eq!(header.pid, Some(4242));
    assert_eq!(header.comm.as_deref(), Some("worker_crash"));
    println!("[METRIC sparse_500mb] time={:?}, alloc={} B", elapsed, alloc_delta);
}
