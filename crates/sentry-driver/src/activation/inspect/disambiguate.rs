//! Multi-tier disambiguation of systemd-activated listening sockets.

use crate::activation::model::ActivatedSocket;
use crate::activation::socket_inspector::{is_unix_stream_listener, matches_bound_path};
use std::path::Path;

/// Selects the intended UNIX domain stream listener socket from activated descriptors.
///
/// Multi-tier resolution:
/// - **Tier 1 (Bound Path Match)**: Checks if any stream listener socket is bound to `target_path`.
/// - **Tier 2 (Name Match)**: Matches socket names against `target_names` and verifies stream listener state.
/// - **Tier 3 (Inode & Type Filtering)**: Selects the first UNIX domain stream listener descriptor.
/// - **Tier 4 (Fallback)**: Returns `None` if no descriptor satisfies the criteria.
pub fn disambiguate_socket(
    sockets: &mut Vec<ActivatedSocket>,
    target_names: &[&str],
    target_path: Option<&Path>,
) -> Option<ActivatedSocket> {
    if sockets.is_empty() {
        return None;
    }

    // Tier 1: Bound Path Match
    if let Some(expected_path) = target_path {
        if let Some(pos) = sockets.iter().position(|s| {
            is_unix_stream_listener(s.fd) && matches_bound_path(s.fd, expected_path)
        }) {
            return Some(sockets.remove(pos));
        }
    }

    // Tier 2: Name Match with Stream Listener Check
    for target in target_names {
        let target_trimmed = target.trim_end_matches(".socket");
        if let Some(pos) = sockets.iter().position(|s| {
            let name_trimmed = s.name.trim_end_matches(".socket");
            (s.name == *target || name_trimmed == target_trimmed)
                && is_unix_stream_listener(s.fd)
        }) {
            return Some(sockets.remove(pos));
        }
    }

    // Tier 3: Inode & Type Filtering (first valid UNIX domain stream listener)
    if let Some(pos) = sockets.iter().position(|s| is_unix_stream_listener(s.fd)) {
        return Some(sockets.remove(pos));
    }

    // Tier 4: Fallback
    None
}
