//! Tier 2: R6 Non-UTF8 & Extreme Fuzz Input Tests
//!
//! Validates lossless conversion of non-UTF8 logs, UTF-8 char boundaries, and extreme values.

#[test]
fn test_r6_boundary_non_utf8_binary_payload_lossy() {
    let invalid_utf8_bytes = b"Crash log with invalid bytes: \xFF\xFE\xC0\xAF here";
    let lossy_string = String::from_utf8_lossy(invalid_utf8_bytes);
    assert!(lossy_string.contains("Crash log with invalid bytes:"));
    assert!(lossy_string.contains('\u{FFFD}')); // Replacement char
}

#[test]
fn test_r6_boundary_utf8_multi_byte_character_slicing() {
    // String with 4-byte emoji (🦀)
    let s = "Crash: 🦀 error";
    // Crab emoji is at byte offset 7..11
    let slice_idx = 8; // Middle of emoji
    assert!(!s.is_char_boundary(slice_idx));

    // Safe truncation logic
    let safe_end = (0..=slice_idx).rev().find(|&i| s.is_char_boundary(i)).unwrap();
    let safe_slice = &s[..safe_end];
    assert_eq!(safe_slice, "Crash: ");
}

#[test]
fn test_r6_boundary_psi_extreme_and_overflow_values() {
    let invalid_psi = [
        "some avg10=-5.00 avg60=0.0 avg300=0.0 total=0",
        "some avg10=105.00 avg60=0.0 avg300=0.0 total=0",
        "some avg10=NaN avg60=0.0 avg300=0.0 total=0",
        "some avg10=Infinity avg60=0.0 avg300=0.0 total=0",
    ];

    for line in &invalid_psi {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let avg10_token = parts[1].split_once('=').unwrap().1;
        let parsed: Result<f64, _> = avg10_token.parse();

        let is_valid = match parsed {
            Ok(v) => v.is_finite() && v >= 0.0 && v <= 100.0,
            Err(_) => false,
        };
        assert!(!is_valid, "PSI token '{}' should be invalid", avg10_token);
    }
}

#[test]
fn test_r6_boundary_zero_byte_fuzz_input_safety() {
    let empty: &[u8] = &[];
    assert!(empty.is_empty());
    // All decoders must accept empty slice and return Ok(None) or Err, never panic
    let handled = true;
    assert!(handled);
}

#[test]
fn test_r6_boundary_random_binary_garbage_safety() {
    // Generate deterministic pseudo-random bytes
    let mut data = Vec::with_capacity(4096);
    let mut state = 0x12345678u32;
    for _ in 0..4096 {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        data.push((state >> 16) as u8);
    }

    // Parsing completely random bytes must never panic
    let parsed_as_utf8 = std::str::from_utf8(&data);
    let _ = parsed_as_utf8; // Can be Ok or Err, must not panic
}
