#![no_main]
//! Fuzz target: D-Bus message header decoder.
//!
//! Feeds arbitrary fuzzer bytes into `decode_dbus_message_header`: the
//! decoder must never panic, loop forever, or read out of bounds, no
//! matter how malformed the framing is. Any crash found here is a
//! soundness bug in the reference decoder, never in the fuzzer input.
//!
//! Corpus: raw D-Bus message bytes, optionally seeded from real traffic.

use libfuzzer_sys::fuzz_target;
use sentry_fuzz::decode_dbus_message_header;

fuzz_target!(|data: &[u8]| {
    // Fuzz binary header decoder: validates safe parsing without panics
    let _ = decode_dbus_message_header(data);
});
