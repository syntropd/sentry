#![no_main]
//! Fuzz target: PSI pressure-record parser.
//!
//! Feeds arbitrary UTF-8 fuzzer bytes into `parse_psi_record`: metric
//! parsing must never panic or mis-slice on malformed `/proc/pressure`
//! text. A crash here is a robustness bug in the reference parser,
//! never in the fuzzer input.
//!
//! Corpus: pressure-stall text lines, seeded from real `/proc` samples.

use libfuzzer_sys::fuzz_target;
use sentry_fuzz::parse_psi_record;

fuzz_target!(|data: &[u8]| {
    // Convert arbitrary bytes to UTF-8 lossy and test PSI metric parser
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = parse_psi_record(text);
    }
});
