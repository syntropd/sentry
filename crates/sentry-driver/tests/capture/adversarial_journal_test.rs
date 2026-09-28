//! Adversarial stress test harness for systemd Journal Export stream parser.
//!
//! Evaluates parser resilience against truncated payloads, DoS field sizes,
//! non-UTF8 binary data, stream desynchronization, and random fuzzing inputs.

use sentry_core::error::JournalError;
use sentry_driver::journal::{JournalExportParser, MAX_FIELD_SIZE};
use std::io::Cursor;

#[test]
fn test_adversarial_truncated_at_all_offsets() {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"MESSAGE=systemd unit failed\n");
    payload.extend_from_slice(b"RAW_DATA\n");
    let bin_len = 16u64;
    payload.extend_from_slice(&bin_len.to_le_bytes());
    payload.extend_from_slice(b"0123456789abcdef\n\n");

    // Truncate at every single byte offset and assert zero panics
    for len in 0..payload.len() {
        let truncated = &payload[..len];
        let cursor = Cursor::new(truncated);
        let mut parser = JournalExportParser::new(cursor);
        let result = parser.parse_next_entry();
        match result {
            Ok(Some(entry)) => {
                // If it succeeded, it must only have completed fields
                assert!(entry.fields.contains_key("MESSAGE") || entry.fields.is_empty());
            }
            Ok(None) => {}
            Err(e) => {
                // Must be expected error, no panic
                assert!(matches!(
                    e,
                    JournalError::UnexpectedEof
                        | JournalError::InvalidBinaryDelimiter { .. }
                        | JournalError::InvalidFormat(_)
                        | JournalError::Io(_)
                ));
            }
        }
    }
}

#[test]
fn test_adversarial_oversized_dos_binary_field() {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"BIG_PAYLOAD\n");
    // Size larger than MAX_FIELD_SIZE (16 MiB + 1)
    let too_large = (MAX_FIELD_SIZE as u64) + 1;
    payload.extend_from_slice(&too_large.to_le_bytes());
    payload.extend_from_slice(b"data");

    let cursor = Cursor::new(payload);
    let mut parser = JournalExportParser::new(cursor);
    let res = parser.parse_next_entry();
    assert!(
        matches!(res, Err(JournalError::FieldTooLarge { size, max, .. }) if size == too_large as usize && max == MAX_FIELD_SIZE),
        "Expected FieldTooLarge error, got {res:?}"
    );
}

#[test]
fn test_adversarial_invalid_binary_delimiter() {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"BINARY_FIELD\n");
    let len = 4u64;
    payload.extend_from_slice(&len.to_le_bytes());
    payload.extend_from_slice(b"test");
    payload.push(b'X'); // Invalid delimiter: should be '\n'

    let cursor = Cursor::new(payload);
    let mut parser = JournalExportParser::new(cursor);
    let res = parser.parse_next_entry();
    assert!(
        matches!(res, Err(JournalError::InvalidBinaryDelimiter { expected: b'\n', found: b'X', .. })),
        "Expected InvalidBinaryDelimiter, got {res:?}"
    );
}

#[test]
fn test_adversarial_non_utf8_binary_field_handling() {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"NON_UTF8\n");
    let raw_bytes: Vec<u8> = vec![0xFF, 0xFE, 0x00, 0x80, 0xAA, 0xBB, 0xCC, 0xDD];
    let len = raw_bytes.len() as u64;
    payload.extend_from_slice(&len.to_le_bytes());
    payload.extend_from_slice(&raw_bytes);
    payload.push(b'\n');
    payload.extend_from_slice(b"MESSAGE=normal message\n\n");

    let cursor = Cursor::new(payload);
    let mut parser = JournalExportParser::new(cursor);
    let entry = parser.parse_next_entry().expect("should parse").expect("entry present");

    // Raw bytes match exactly
    assert_eq!(entry.get("NON_UTF8"), Some(raw_bytes.as_slice()));
    // Valid UTF-8 getter returns None
    assert_eq!(entry.get_str("NON_UTF8"), None);
    // Lossy getter does not panic
    let lossy = entry.get_str_lossy("NON_UTF8").unwrap();
    assert!(lossy.contains('\u{FFFD}'));
}

#[test]
fn test_adversarial_stream_resynchronization_after_corruption() {
    let mut payload = Vec::new();
    // Corrupt garbage entry with invalid key
    payload.extend_from_slice(b"bad key without equal sign or valid name\n\n");
    // Followed by valid entry
    payload.extend_from_slice(b"MESSAGE=recovered\nPRIORITY=6\n\n");

    let cursor = Cursor::new(payload);
    let mut parser = JournalExportParser::new(cursor);

    // First call fails due to invalid format and resyncs
    let err = parser.parse_next_entry();
    println!("Resync test first call: {err:?}");
    assert!(err.is_err(), "Expected first corrupted entry to error");

    // Second call successfully recovers the next entry
    let next = parser.parse_next_entry();
    println!("Resync test second call: {next:?}");
    let entry = next.expect("resync failed to recover").expect("expected recovered entry");
    assert_eq!(entry.get_str("MESSAGE"), Some("recovered"));
    assert_eq!(entry.get_str("PRIORITY"), Some("6"));
}

#[test]
fn test_adversarial_pseudo_fuzzing_random_byte_streams() {
    // Deterministic pseudo-random generation to stress parser state machine
    let mut seed: u64 = 0xDEADBEEFCAFE;
    let mut prng = || -> u8 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 33) as u8
    };

    for _ in 0..100 {
        let mut random_buf = vec![0u8; 512];
        for b in random_buf.iter_mut() {
            *b = prng();
        }

        let cursor = Cursor::new(random_buf);
        let mut parser = JournalExportParser::new(cursor);

        // Repeatedly parse until EOF or error without panic
        for _ in 0..20 {
            match parser.parse_next_entry() {
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => {}
            }
        }
    }
}

#[test]
fn test_adversarial_consecutive_blank_lines() {
    let payload = b"\n\n\n\n\n\n\n\nMESSAGE=valid\n\n\n\n\n";
    let cursor = Cursor::new(payload);
    let mut parser = JournalExportParser::new(cursor);

    let entry = parser.parse_next_entry().expect("should parse").expect("entry present");
    assert_eq!(entry.get_str("MESSAGE"), Some("valid"));

    let next = parser.parse_next_entry().expect("should parse EOF");
    assert!(next.is_none());
}

#[test]
fn test_adversarial_non_utf8_in_text_field() {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"MESSAGE=hello\xFF\xFEworld\n\n");
    payload.extend_from_slice(b"PRIORITY=6\n\n");

    let cursor = Cursor::new(payload);
    let mut parser = JournalExportParser::new(cursor);

    let res = parser.parse_next_entry();
    assert!(res.is_err(), "Expected error for non-UTF8 in text line");

    let next = parser.parse_next_entry();
    let entry = next.expect("resync failed after non-utf8").expect("expected recovered entry");
    assert_eq!(entry.get_str("PRIORITY"), Some("6"));
}

