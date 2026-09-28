//! Adversarial stress tests for compressed coredump stream reading and ELF header validation.
//!
//! Tests malformed/truncated zstd frames, invalid lz4 block sizes, and truncated ELF headers.

use sentry_driver::coredump::elf_extractor::extract_elf_crash_headers;
use sentry_driver::coredump::elf_header_parser::parse_elf_header;
use sentry_driver::coredump::stream_reader::{read_bounded_coredump_bytes, BoundedStreamReader};
use std::io::Cursor;

#[test]
fn test_adversarial_zstd_magic_only() {
    let mut cursor = Cursor::new(vec![0x28, 0xB5, 0x2F, 0xFD]);
    let res = read_bounded_coredump_bytes(&mut cursor);
    assert!(res.is_err(), "Truncated zstd stream with magic only must return Err");
}

#[test]
fn test_adversarial_zstd_corrupted_frame_header() {
    let mut data = vec![0x28, 0xB5, 0x2F, 0xFD];
    data.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xAA, 0xBB]);
    let mut cursor = Cursor::new(data);
    let res = read_bounded_coredump_bytes(&mut cursor);
    assert!(res.is_err(), "Corrupted zstd frame header must return Err");
}

#[test]
fn test_adversarial_zstd_truncated_payload() {
    // Valid 4-byte zstd magic + 1 garbage byte
    let mut cursor = Cursor::new(vec![0x28, 0xB5, 0x2F, 0xFD, 0x00]);
    let res = read_bounded_coredump_bytes(&mut cursor);
    assert!(res.is_err(), "Truncated zstd payload must return Err");
}

#[test]
fn test_adversarial_lz4_magic_only() {
    let mut cursor = Cursor::new(vec![0x04, 0x22, 0x4D, 0x18]);
    let res = extract_elf_crash_headers(&mut cursor);
    assert!(res.is_err(), "Truncated lz4 stream with magic only must return Err on ELF extraction");
}

#[test]
fn test_adversarial_lz4_corrupt_descriptor() {
    // LZ4 magic followed by corrupted flag and descriptor bytes
    let mut data = vec![0x04, 0x22, 0x4D, 0x18];
    data.extend_from_slice(&[0x00, 0x00, 0xFF, 0xFF]);
    let mut cursor = Cursor::new(data);
    let res = read_bounded_coredump_bytes(&mut cursor);
    assert!(res.is_err(), "Corrupt LZ4 descriptor must return Err");
}

#[test]
fn test_adversarial_lz4_invalid_block_size() {
    // LZ4 frame: magic + valid minimal header descriptor + huge 2GB block length
    let mut data = vec![0x04, 0x22, 0x4D, 0x18];
    data.extend_from_slice(&[0x64, 0x70, 0xB7]); // FLG, BD, HC
    data.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0x7F]); // 2GB block size
    data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]); // Truncated body
    let mut cursor = Cursor::new(data);
    let res = read_bounded_coredump_bytes(&mut cursor);
    assert!(res.is_err(), "Invalid LZ4 block size must return Err");
}

#[test]
fn test_adversarial_elf_empty_stream() {
    let mut cursor = Cursor::new(Vec::new());
    let res = read_bounded_coredump_bytes(&mut cursor);
    assert!(res.is_err(), "Empty stream must return Err");
}

#[test]
fn test_adversarial_elf_truncated_magic() {
    let mut cursor = Cursor::new(vec![0x7F, b'E']);
    let res = read_bounded_coredump_bytes(&mut cursor);
    assert!(res.is_err(), "Truncated magic must return Err");
}

#[test]
fn test_adversarial_elf_exact_magic_only() {
    let raw = vec![0x7F, b'E', b'L', b'F'];
    let mut cursor = Cursor::new(raw.clone());
    let bytes = read_bounded_coredump_bytes(&mut cursor).expect("Reads 4 bytes raw ELF");
    assert_eq!(bytes, raw);

    let err = parse_elf_header(&bytes).unwrap_err();
    assert!(err.to_string().contains("Buffer too small"));
}

#[test]
fn test_adversarial_elf_header_less_than_16_bytes() {
    let buf = vec![0x7F, b'E', b'L', b'F', 2, 1, 1];
    let err = parse_elf_header(&buf).unwrap_err();
    assert!(err.to_string().contains("Buffer too small"));
}

#[test]
fn test_adversarial_elf_header_invalid_class() {
    let mut buf = vec![0u8; 64];
    buf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
    buf[4] = 99; // Invalid class
    buf[5] = 1;  // LE
    let err = parse_elf_header(&buf).unwrap_err();
    assert!(err.to_string().contains("Unsupported ELF class"));
}

#[test]
fn test_adversarial_elf_header_invalid_endianness() {
    let mut buf = vec![0u8; 64];
    buf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
    buf[4] = 2; // 64-bit
    buf[5] = 0; // Invalid endianness
    let err = parse_elf_header(&buf).unwrap_err();
    assert!(err.to_string().contains("Unsupported ELF endianness"));
}

#[test]
fn test_adversarial_elf64_truncated_before_64_bytes() {
    for len in 16..64 {
        let mut buf = vec![0u8; len];
        buf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
        buf[4] = 2; // 64-bit
        buf[5] = 1; // LE
        let err = parse_elf_header(&buf).unwrap_err();
        assert!(
            err.to_string().contains("Truncated") || err.to_string().contains("Buffer too small"),
            "Expected truncation error at len {len}, got: {err}"
        );
    }
}

#[test]
fn test_adversarial_elf32_truncated_before_52_bytes() {
    for len in 16..52 {
        let mut buf = vec![0u8; len];
        buf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
        buf[4] = 1; // 32-bit
        buf[5] = 1; // LE
        let err = parse_elf_header(&buf).unwrap_err();
        assert!(
            err.to_string().contains("Truncated") || err.to_string().contains("Buffer too small"),
            "Expected truncation error at len {len}, got: {err}"
        );
    }
}

#[test]
fn test_adversarial_extract_headers_truncated_elf() {
    let mut cursor = Cursor::new(vec![0x7F, b'E', b'L', b'F', 2, 1, 1, 0]);
    let err = extract_elf_crash_headers(&mut cursor).unwrap_err();
    assert!(err.to_string().contains("Truncated") || err.to_string().contains("Buffer too small"));
}

#[test]
fn test_adversarial_bounded_open_reader_corrupt_zstd() {
    let cursor = Box::new(Cursor::new(vec![0x28, 0xB5, 0x2F, 0xFD, 0xDE, 0xAD]));
    let mut reader = match BoundedStreamReader::open_reader(cursor) {
        Ok(r) => r,
        Err(_) => return, // cleanly returned error
    };
    let mut out = Vec::new();
    let res = std::io::Read::read_to_end(&mut reader, &mut out);
    assert!(res.is_err(), "Reading corrupt zstd frame must return io::Error");
}
