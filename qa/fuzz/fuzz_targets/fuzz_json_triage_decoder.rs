#![no_main]
//! Fuzz target: diagnostic JSON triage decoder.
//!
//! Feeds arbitrary UTF-8 fuzzer bytes into the triage JSON pipeline:
//! markdown stripping, outermost-brace isolation, and schema parsing
//! must never panic or hang on adversarial input. A crash here is a
//! robustness bug in the sanitizer, never in the fuzzer input.
//!
//! Corpus: raw LLM output bytes, seeded with markdown-wrapped payloads.

use libfuzzer_sys::fuzz_target;
use sentry_fuzz::parse_and_validate_diagnostic_json;

fuzz_target!(|data: &[u8]| {
    // Tests markdown stripping, brace isolation, and schema parsing
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = parse_and_validate_diagnostic_json(text);
    }
});
