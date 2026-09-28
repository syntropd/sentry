//! Parses colon-separated file descriptor names from `$LISTEN_FDNAMES`.

use crate::activation::parser::MAX_ACTIVATED_FDS;
use std::env;

/// Parses `$LISTEN_FDNAMES` into a list of name strings matching `count` descriptors.
pub fn parse_listen_fdnames(count: usize) -> Vec<String> {
    let count = count.min(MAX_ACTIVATED_FDS);
    let raw = match env::var("LISTEN_FDNAMES") {
        Ok(v) if !v.is_empty() => v,
        _ => return (0..count).map(|i| format!("unknown:{i}")).collect(),
    };

    let mut names: Vec<String> = raw
        .split(':')
        .take(count)
        .map(|s| {
            if s.is_empty() {
                "unknown".to_string()
            } else {
                s.to_string()
            }
        })
        .collect();

    while names.len() < count {
        names.push(format!("unknown:{}", names.len()));
    }
    names
}
