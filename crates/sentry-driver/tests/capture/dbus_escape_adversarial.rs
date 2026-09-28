//! Adversarial stress tests for D-Bus unit path hex-escaping and unescaping.

use sentry_driver::dbus::{
    escape_unit_name, object_path_to_unit_name, unescape_unit_name, unit_name_to_object_path,
    DbusDriverError,
};

#[test]
fn test_complex_unit_name_matches_systemd_specification() {
    let unit = "test-unit@sub:slice.service";
    let escaped = escape_unit_name(unit);
    // Verifies exact parity with systemd LoadUnit/GetUnit D-Bus output:
    // '-' -> _2d, '@' -> _40, ':' -> _3a, '.' -> _2e
    assert_eq!(escaped, "test_2dunit_40sub_3aslice_2eservice");

    let object_path = unit_name_to_object_path(unit).expect("Must form valid object path");
    assert_eq!(
        object_path.as_str(),
        "/org/freedesktop/systemd1/unit/test_2dunit_40sub_3aslice_2eservice"
    );

    let roundtrip_unit = object_path_to_unit_name(object_path.as_str())
        .expect("Must extract unit name from path");
    assert_eq!(roundtrip_unit, unit);
}

#[test]
fn test_non_ascii_multibyte_utf8_roundtrip() {
    let cases = [
        "test-unit-日本語.service",
        "café-crème@instance:1.service",
        "emoji-🚀-sentry-worker.service",
        "система-мониторинга@форум.slice",
        "服务_测试_守护进程.service",
        "خدمة-النظام@شبكة.socket",
    ];

    for unit in cases {
        let escaped = escape_unit_name(unit);
        // Ensure escaped path consists strictly of ASCII alphanumeric and underscores
        assert!(
            escaped.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "Escaped string '{escaped}' contains non-D-Bus path character"
        );

        let unescaped = unescape_unit_name(&escaped)
            .unwrap_or_else(|e| panic!("Failed to unescape '{escaped}' for '{unit}': {e}"));
        assert_eq!(unescaped, unit, "Roundtrip mismatch for '{unit}'");

        let path = unit_name_to_object_path(unit)
            .unwrap_or_else(|e| panic!("Failed to build D-Bus path for '{unit}': {e}"));
        let parsed = object_path_to_unit_name(path.as_str())
            .unwrap_or_else(|e| panic!("Failed to parse D-Bus path for '{unit}': {e}"));
        assert_eq!(parsed, unit);
    }
}

#[test]
fn test_edge_punctuation_and_symbols_roundtrip() {
    let symbols = "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";
    let escaped = escape_unit_name(symbols);
    let unescaped = unescape_unit_name(&escaped).unwrap();
    assert_eq!(unescaped, symbols);

    // Multiple underscores
    let underscores = "___unit___name___";
    let esc_under = escape_unit_name(underscores);
    assert_eq!(esc_under, "_5f_5f_5funit_5f_5f_5fname_5f_5f_5f");
    assert_eq!(unescape_unit_name(&esc_under).unwrap(), underscores);

    // Whitespace and escape chars
    let whitespace = "unit with spaces \tand \nnewlines.service";
    let esc_ws = escape_unit_name(whitespace);
    assert_eq!(unescape_unit_name(&esc_ws).unwrap(), whitespace);

    // Slashes (as in device / mount units)
    let device_unit = "sys-devices-virtual-net-eth0.device";
    let esc_dev = escape_unit_name(device_unit);
    assert_eq!(unescape_unit_name(&esc_dev).unwrap(), device_unit);
}

#[test]
fn test_adversarial_malformed_hex_sequences_rejected() {
    let truncated_cases = [
        "foo_",
        "foo_2",
        "_",
        "_1",
        "prefix_2suffix",
    ];

    for case in truncated_cases {
        let err = unescape_unit_name(case).unwrap_err();
        match err {
            DbusDriverError::InvalidPathEscape(val, msg) => {
                assert_eq!(val, case);
                assert!(msg.contains("Truncated hex") || msg.contains("invalid"));
            }
            other => panic!("Expected InvalidPathEscape for '{case}', got: {other:?}"),
        }
    }

    let non_hex_cases = [
        "foo_zz",
        "foo_ag",
        "foo_G1",
        "foo_-1",
        "foo_??",
    ];

    for case in non_hex_cases {
        let err = unescape_unit_name(case).unwrap_err();
        assert!(matches!(err, DbusDriverError::InvalidPathEscape(_, _)));
    }
}

#[test]
fn test_adversarial_invalid_utf8_hex_bytes_rejected() {
    // 0xFF is an illegal UTF-8 byte
    let err_ff = unescape_unit_name("unit_ff.service").unwrap_err();
    assert!(matches!(err_ff, DbusDriverError::InvalidPathEscape(_, _)));

    // 0x80 is an orphan continuation byte
    let err_80 = unescape_unit_name("unit_80.service").unwrap_err();
    assert!(matches!(err_80, DbusDriverError::InvalidPathEscape(_, _)));

    // 0xC0 0xAF is an overlong encoding of '/'
    let err_overlong = unescape_unit_name("unit_c0_af.service").unwrap_err();
    assert!(matches!(err_overlong, DbusDriverError::InvalidPathEscape(_, _)));
}

#[test]
fn test_adversarial_object_path_prefix_and_bounds() {
    // Wrong prefix
    let wrong_prefix = "/org/freedesktop/systemd1/job/123";
    let err = object_path_to_unit_name(wrong_prefix).unwrap_err();
    assert!(matches!(err, DbusDriverError::InvalidObjectPath(_, _)));

    // Empty object path
    let err_empty = object_path_to_unit_name("").unwrap_err();
    assert!(matches!(err_empty, DbusDriverError::InvalidObjectPath(_, _)));

    // Empty unit name in unit_name_to_object_path results in trailing slash (invalid in D-Bus)
    let err_empty_unit = unit_name_to_object_path("").unwrap_err();
    assert!(matches!(err_empty_unit, DbusDriverError::InvalidObjectPath(_, _)));
}
