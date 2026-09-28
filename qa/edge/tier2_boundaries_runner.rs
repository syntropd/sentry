//! Tier 2: Boundary & Corner Cases Integration Runner
//!
//! Executes all Tier 2 tests covering boundaries and edge cases for R1-R7.

#[path = "bounds/r1_limits.rs"]
mod r1_limits;
#[path = "bounds/r2_corrupt_stream.rs"]
mod r2_corrupt_stream;
#[path = "bounds/r3_malformed_json.rs"]
mod r3_malformed_json;
#[path = "bounds/r4_circuit_bounds.rs"]
mod r4_circuit_bounds;
#[path = "bounds/r5_terminal_fuzz.rs"]
mod r5_terminal_fuzz;
#[path = "bounds/r6_non_utf8.rs"]
mod r6_non_utf8;
#[path = "bounds/r7_package_bounds.rs"]
mod r7_package_bounds;
