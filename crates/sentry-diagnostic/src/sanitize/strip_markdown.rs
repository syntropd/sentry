//! Stage-1 raw-output preparation: markdown fence stripping and JSON slicing.

/// Strips markdown code fences (```json or ```) and surrounding whitespace.
pub fn strip_markdown_fences(input: &str) -> &str {
    let trimmed = input.trim();

    // Find starting code fence (either at start or after conversational preface)
    let after_open = if let Some(pos) = trimmed.find("```json") {
        &trimmed[pos + 7..]
    } else if let Some(pos) = trimmed.find("```JSON") {
        &trimmed[pos + 7..]
    } else if let Some(pos) = trimmed.find("```") {
        &trimmed[pos + 3..]
    } else {
        trimmed
    };

    let content = after_open.trim_start();

    // Strip trailing markdown fence if present
    if let Some(end_pos) = content.rfind("```") {
        content[..end_pos].trim()
    } else {
        content.trim()
    }
}

/// Slices the outermost JSON object between the first `{` and last `}`.
///
/// Returns `None` if no matching pair of braces exists.
pub fn slice_outermost_json(input: &str) -> Option<&str> {
    let first = input.find('{')?;
    let last = input.rfind('}')?;

    if first <= last {
        Some(&input[first..=last])
    } else {
        None
    }
}
