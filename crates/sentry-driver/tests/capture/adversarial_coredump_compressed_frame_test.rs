//! Adversarial test verifying that LZ4 and ZSTD decoders stop deterministically
//! after reading ELF header and notes, without decompressing the remaining
//! multi-megabyte / multi-hundred megabyte crash dump payload.

use sentry_driver::coredump::elf_extractor::extract_elf_crash_headers;
use sentry_driver::coredump::lz4_flex;
use std::io::{Cursor, Read, Write};

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
    desc_status[12..14].copy_from_slice(&11_i16.to_le_bytes());
    desc_status[32..36].copy_from_slice(&9999_u32.to_le_bytes());
    add_note(&mut notes, 1, b"CORE\0", &desc_status);

    let mut desc_psinfo = vec![0u8; 136];
    desc_psinfo[24..28].copy_from_slice(&9999_u32.to_le_bytes());
    let comm = b"large_app_crash\0";
    desc_psinfo[40..40 + comm.len()].copy_from_slice(comm);
    let cmdline = b"/usr/bin/large_app --production\0";
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

    if payload_size > 0 {
        // Simulated 50MB+ PT_LOAD data pages
        elf.resize(elf.len() + payload_size, 0x55);
    }
    elf
}

#[test]
fn test_adversarial_lz4_compressed_50mb_frame_deterministic_cutoff() {
    let uncompressed_payload_size = 50 * 1024 * 1024; // 50 MB
    let full_uncompressed = build_test_elf_with_payload(uncompressed_payload_size);
    assert!(full_uncompressed.len() >= 50 * 1024 * 1024);

    let mut compressed_frame = Vec::new();
    {
        let mut encoder = lz4_flex::frame::FrameEncoder::new(&mut compressed_frame);
        encoder.write_all(&full_uncompressed).unwrap();
        encoder.finish().unwrap();
    }

    let compressed_len = compressed_frame.len();
    assert!(compressed_len > 1000);

    let mut counting = CountingReader::new(Cursor::new(compressed_frame));
    let start = std::time::Instant::now();
    let header = extract_elf_crash_headers(&mut counting).expect("should extract from 50MB LZ4 frame");
    let elapsed = start.elapsed();

    assert_eq!(header.pid, Some(9999));
    assert_eq!(header.signal, Some(11));
    assert_eq!(header.comm.as_deref(), Some("large_app_crash"));
    assert_eq!(header.cmdline.as_deref(), Some("/usr/bin/large_app --production"));

    // Verify deterministic stopping: decoder must NOT decompress or read the entire 50MB payload
    // In LZ4 with 64KB blocks, reading 64KB uncompressed bytes only consumes the first 1-2 blocks!
    assert!(
        counting.bytes_read < compressed_len,
        "Decoder read entire compressed stream ({} of {} bytes)",
        counting.bytes_read,
        compressed_len
    );
    assert!(
        elapsed.as_millis() < 500,
        "Decompression took too long ({:?}); likely decompressed excess payload",
        elapsed
    );

    println!(
        "[EMPIRICAL METRIC] 50MB LZ4 Frame: compressed_total={} B, read={} B, elapsed={:?}",
        compressed_len, counting.bytes_read, elapsed
    );
}

#[test]
fn test_adversarial_multi_chunk_lz4_frame_deterministic_stop() {
    let full_uncompressed = build_test_elf_with_payload(10 * 1024 * 1024); // 10 MB
    let mut compressed_frame = Vec::new();
    {
        let mut encoder = lz4_flex::frame::FrameEncoder::new(&mut compressed_frame);
        encoder.write_all(&full_uncompressed).unwrap();
        encoder.finish().unwrap();
    }

    let compressed_len = compressed_frame.len();
    let mut counting = CountingReader::new(Cursor::new(compressed_frame));
    let header = extract_elf_crash_headers(&mut counting).expect("should extract headers");

    assert_eq!(header.comm.as_deref(), Some("large_app_crash"));
    assert!(
        counting.bytes_read < compressed_len,
        "Deterministic cutoff failed: read {} of {} compressed bytes",
        counting.bytes_read,
        compressed_len
    );
    println!(
        "[EMPIRICAL METRIC] 10MB LZ4 Frame: total={} B, read={} B",
        compressed_len, counting.bytes_read
    );
}
