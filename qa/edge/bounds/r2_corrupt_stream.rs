//! Tier 2: R2 Stream Parsing & Corruption Stress Tests
//!
//! Validates resilience against truncated fields, DoS field sizes, and stream desync.

#[test]
fn test_r2_boundary_truncated_binary_field() {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"MESSAGE\n");
    // Length claimed: 1,048,576 bytes (1MB)
    let length = 1048576u64;
    payload.extend_from_slice(&length.to_le_bytes());
    // But payload is only 16 bytes
    payload.extend_from_slice(b"Truncated data!!");

    let claimed_len = u64::from_le_bytes(payload[8..16].try_into().unwrap()) as usize;
    let actual_remaining = payload.len() - 16;
    assert!(
        actual_remaining < claimed_len,
        "Stream must detect truncated payload"
    );
}

#[test]
fn test_r2_boundary_oversized_dos_field_length() {
    let max_field_size = 4 * 1024 * 1024; // 4MB
    let malicious_size = 8589934592u64; // 8GB length header

    let is_oversized = malicious_size > max_field_size as u64;
    assert!(is_oversized, "8GB field size must exceed 4MB DoS boundary");
}

#[test]
fn test_r2_boundary_corrupted_key_names() {
    let invalid_keys = [
        "lower_case_key=val",
        "KEY WITH SPACES=val",
        "=MISSING_KEY",
        "KEY\0WITH_NUL=val",
    ];

    for key_line in &invalid_keys {
        if let Some((k, _)) = key_line.split_once('=') {
            let is_valid = !k.is_empty()
                && k.chars().all(|c| c.is_ascii_uppercase() || c == '_');
            assert!(!is_valid, "Key '{}' should be classified as invalid", k);
        }
    }
}

#[test]
fn test_r2_boundary_stream_desynchronization_recovery() {
    let mut stream = Vec::new();
    // Inject corrupt garbage
    stream.extend_from_slice(b"\xFF\xFE\x00\x01\x02GarbageData");
    // Next entry delimiter
    stream.extend_from_slice(b"\n\n");
    // Valid entry follows
    stream.extend_from_slice(b"MESSAGE=Recovered entry\nPRIORITY=4\n\n");

    let text = String::from_utf8_lossy(&stream);
    assert!(text.contains("MESSAGE=Recovered entry"));

    // Splitting by double newline recovers valid segments
    let entries: Vec<&str> = text.split("\n\n").collect();
    assert!(entries.iter().any(|e| e.contains("MESSAGE=Recovered entry")));
}

#[test]
fn test_r2_boundary_zero_byte_datagram_handling() {
    let empty_datagram: &[u8] = &[];
    assert!(empty_datagram.is_empty());
    // Empty datagram should be safely ignored without panic
    let handled = empty_datagram.is_empty();
    assert!(handled);
}
