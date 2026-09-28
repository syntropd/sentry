//! Extracts crash stack trace frames from systemd-coredump message payloads.

/// Extracts and bounds demangled backtrace frames from a coredump `MESSAGE` text payload.
pub fn extract_backtrace(message: &str, max_frames: usize) -> Option<String> {
    let mut collecting = false;
    let mut frames = Vec::new();

    for line in message.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Stack trace of thread") {
            collecting = true;
            continue;
        }

        if collecting {
            if trimmed.starts_with('#') {
                frames.push(trimmed.to_string());
                if frames.len() >= max_frames {
                    frames.push("... [backtrace truncated by sentry]".to_string());
                    break;
                }
            } else if !frames.is_empty() && trimmed.is_empty() {
                // Empty line concludes stack trace block
                break;
            }
        }
    }

    // Fallback: search for lines anywhere in message matching '#<digit>'
    if frames.is_empty() {
        for line in message.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#')
                && trimmed
                    .chars()
                    .nth(1)
                    .map_or(false, |c| c.is_ascii_digit())
            {
                frames.push(trimmed.to_string());
                if frames.len() >= max_frames {
                    frames.push("... [backtrace truncated by sentry]".to_string());
                    break;
                }
            }
        }
    }

    if frames.is_empty() {
        None
    } else {
        Some(frames.join("\n"))
    }
}
