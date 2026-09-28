//! Unit tests for markdown fence stripping.

use sentry_diagnostic::sanitize::strip_markdown_fences;

#[test]
fn test_strip_markdown_with_json_fence() {
    let input = "```json\n{\"incident_id\": \"123\"}\n```";
    assert_eq!(strip_markdown_fences(input), "{\"incident_id\": \"123\"}");
}

#[test]
fn test_strip_markdown_with_generic_fence() {
    let input = "```\n{\"key\": \"val\"}\n```";
    assert_eq!(strip_markdown_fences(input), "{\"key\": \"val\"}");
}

#[test]
fn test_strip_markdown_plain_text() {
    let input = "{\"key\": \"val\"}";
    assert_eq!(strip_markdown_fences(input), "{\"key\": \"val\"}");
}

#[test]
fn test_strip_markdown_with_leading_and_trailing_spaces() {
    let input = "   ```json\n{\"test\": 1}\n```   ";
    assert_eq!(strip_markdown_fences(input), "{\"test\": 1}");
}

#[test]
fn test_strip_markdown_conversational_preface_and_postscript() {
    let input = "Here is the diagnosis you requested for unit {service}:\n```json\n{\"incident_id\": \"456\", \"status\": \"ok\"}\n```\nHope this helps!";
    assert_eq!(
        strip_markdown_fences(input),
        "{\"incident_id\": \"456\", \"status\": \"ok\"}"
    );
}
