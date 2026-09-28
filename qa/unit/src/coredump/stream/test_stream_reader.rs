//! 1:1 Unit QA tests for bounded stream reader (.zst, .lz4, raw ELF).

use sentry_driver::coredump::lz4_flex;
use sentry_driver::coredump::stream_reader::{
    read_bounded_coredump_bytes, BoundedStreamReader, MAX_DECOMPRESSED_BYTES,
};
use std::io::{Cursor, Read, Write};

const SYNTHETIC_ZSTD_CORE: &[u8] = &[
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

#[test]
fn test_stream_reader_raw_elf() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&[0x7F, b'E', b'L', b'F']);
    payload.extend_from_slice(b"sample_elf_content_bytes_here");

    let mut cursor = Cursor::new(payload.clone());
    let decompressed = read_bounded_coredump_bytes(&mut cursor).unwrap();
    assert_eq!(decompressed, payload);
}

#[test]
fn test_stream_reader_lz4_frame() {
    let mut original = Vec::new();
    original.extend_from_slice(&[0x7F, b'E', b'L', b'F']);
    original.extend_from_slice(b"lz4_decompressed_payload_test");

    let mut compressed = Vec::new();
    {
        let mut encoder = lz4_flex::frame::FrameEncoder::new(&mut compressed);
        encoder.write_all(&original).unwrap();
        encoder.finish().unwrap();
    }

    assert_eq!(&compressed[0..4], &[0x04, 0x22, 0x4D, 0x18]);

    let mut cursor = Cursor::new(compressed);
    let decompressed = read_bounded_coredump_bytes(&mut cursor).unwrap();
    assert_eq!(decompressed, original);
}

#[test]
fn test_stream_reader_zstd_frame() {
    let mut cursor = Cursor::new(SYNTHETIC_ZSTD_CORE);
    let decompressed = read_bounded_coredump_bytes(&mut cursor).unwrap();

    assert_eq!(&decompressed[0..4], &[0x7F, b'E', b'L', b'F']);
    assert_eq!(decompressed.len(), 908);
}

#[test]
fn test_stream_reader_invalid_magic() {
    let invalid = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
    let mut cursor = Cursor::new(invalid);
    let err = read_bounded_coredump_bytes(&mut cursor).unwrap_err();
    assert!(err.to_string().contains("Unrecognized coredump stream format"));
}

#[test]
fn test_stream_reader_truncated_stream() {
    let truncated = vec![0x7F, b'E'];
    let mut cursor = Cursor::new(truncated);
    let err = read_bounded_coredump_bytes(&mut cursor).unwrap_err();
    assert!(err.to_string().contains("Input stream too short"));
}

#[test]
fn test_stream_reader_bounded_memory_limit() {
    let mut large_elf = Vec::with_capacity(128 * 1024);
    large_elf.extend_from_slice(&[0x7F, b'E', b'L', b'F']);
    large_elf.resize(128 * 1024, 0xAA);

    let mut cursor = Cursor::new(large_elf);
    let result = read_bounded_coredump_bytes(&mut cursor).unwrap();
    assert_eq!(result.len() as u64, MAX_DECOMPRESSED_BYTES);
    assert_eq!(result.len(), 65536);
    assert!(
        result.capacity() <= MAX_DECOMPRESSED_BYTES as usize,
        "Capacity {} exceeds max allowed decompressed bytes {}",
        result.capacity(),
        MAX_DECOMPRESSED_BYTES
    );
}

#[test]
fn test_bounded_stream_reader_open_reader() {
    let mut original = Vec::new();
    original.extend_from_slice(&[0x7F, b'E', b'L', b'F']);
    original.extend_from_slice(b"chunk_reader_test");

    let cursor = Box::new(Cursor::new(original.clone()));
    let mut reader = BoundedStreamReader::open_reader(cursor).unwrap();

    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).unwrap();
    assert_eq!(buf, original);
}
