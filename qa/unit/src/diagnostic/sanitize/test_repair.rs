//! Unit tests for heuristic JSON repair.

use sentry_diagnostic::sanitize::repair_json;

#[test]
fn test_repair_unclosed_brace() {
    let input = "{\"key\": \"val\"";
    let repaired = repair_json(input);
    assert_eq!(repaired, "{\"key\": \"val\"}");
}

#[test]
fn test_repair_unclosed_nested_structures() {
    let input = "{\"root\": {\"items\": [1, 2";
    let repaired = repair_json(input);
    assert_eq!(repaired, "{\"root\": {\"items\": [1, 2]}}");
}

#[test]
fn test_repair_unclosed_string_quote() {
    let input = "{\"message\": \"truncated text";
    let repaired = repair_json(input);
    assert_eq!(repaired, "{\"message\": \"truncated text\"}");
}

#[test]
fn test_repair_trailing_comma() {
    let input = "{\"key\": \"val\", }";
    let repaired = repair_json(input);
    assert_eq!(repaired, "{\"key\": \"val\"}");
}

#[test]
fn test_repair_trailing_comma_at_eof() {
    let input = "{\"items\": [1, 2, ],";
    let repaired = repair_json(input);
    assert_eq!(repaired, "{\"items\": [1, 2]}");
}

#[test]
fn test_repair_empty_input() {
    assert_eq!(repair_json(""), "{}");
}
