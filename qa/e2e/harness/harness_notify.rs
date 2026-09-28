#![allow(dead_code)]
use std::collections::HashMap;

pub fn sanitize_terminal_string(input: &str) -> String {
    let mut sanitized = String::with_capacity(input.len());
    let mut in_escape = false;

    for ch in input.chars() {
        if in_escape {
            // Escape sequence ends at terminating alphabetic character or @ through ~
            if ch.is_ascii_alphabetic() || ch == 'm' || ch == 'K' || ch == 'J' {
                in_escape = false;
            }
            continue;
        }

        if ch == '\x1b' {
            in_escape = true;
            continue;
        }

        // Allow tab and newline, drop all other control characters
        if ch == '\n' || ch == '\t' || (!ch.is_control() && (ch as u32) >= 32) {
            sanitized.push(ch);
        }
    }
    sanitized
}

pub struct NotificationRateLimiter {
    pub per_unit_cooldown_secs: u64,
    pub global_burst_capacity: usize,
    pub last_alert_times: HashMap<String, u64>,
    pub recent_alert_timestamps: Vec<u64>,
}

impl NotificationRateLimiter {
    pub fn new(cooldown_secs: u64, burst_cap: usize) -> Self {
        Self {
            per_unit_cooldown_secs: cooldown_secs,
            global_burst_capacity: burst_cap,
            last_alert_times: HashMap::new(),
            recent_alert_timestamps: Vec::new(),
        }
    }

    pub fn should_emit(&mut self, unit: &str, now_secs: u64) -> bool {
        // 1. Check per-unit cooldown
        if let Some(&last) = self.last_alert_times.get(unit) {
            if now_secs < last + self.per_unit_cooldown_secs {
                return false;
            }
        }

        // 2. Check global leaky bucket (burst capacity over 10s window)
        let cutoff = now_secs.saturating_sub(10);
        self.recent_alert_timestamps.retain(|&t| t >= cutoff);
        if self.recent_alert_timestamps.len() >= self.global_burst_capacity {
            return false;
        }

        self.last_alert_times.insert(unit.to_string(), now_secs);
        self.recent_alert_timestamps.push(now_secs);
        true
    }
}
