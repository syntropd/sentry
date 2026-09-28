//! Unit tests for outermost JSON bracket boundary slicing.

use sentry_diagnostic::sanitize::slice_outermost_json;

#[test]
fn test_slice_clean_json() {
    let input = "{\"a\": 1}";
    assert_eq!(slice_outermost_json(input), Some("{\"a\": 1}"));
}

#[test]
fn test_slice_json_with_prefix_and_suffix_text() {
    let input = "Here is the diagnosis: {\"unit\": \"test.service\"} hope this helps!";
    assert_eq!(
        slice_outermost_json(input),
        Some("{\"unit\": \"test.service\"}")
    );
}

#[test]
fn test_slice_json_with_nested_objects() {
    let input = "Prefix {\"root\": {\"inner\": 42}} Suffix";
    assert_eq!(
        slice_outermost_json(input),
        Some("{\"root\": {\"inner\": 42}}")
    );
}

#[test]
fn test_slice_without_braces() {
    let input = "no braces here at all";
    assert_eq!(slice_outermost_json(input), None);
}

#[test]
fn test_slice_with_reversed_braces() {
    let input = "closing } first then opening {";
    assert_eq!(slice_outermost_json(input), None);
}
