//! Tier 4: Real-World Scenarios Integration Runner
//!
//! Executes all Tier 4 real-world scenario tests:
//! - 10,000 crashes/sec failure storm
//! - OOM crash simulation
//! - Flap lockout enforcement
//! - Segfault coredump extraction
//! - Cascading failure load shedding

#[path = "scenarios/failure_storm.rs"]
mod failure_storm;
#[path = "scenarios/oom_simulation.rs"]
mod oom_simulation;
#[path = "scenarios/flap_lockout.rs"]
mod flap_lockout;
#[path = "scenarios/segfault_coredump.rs"]
mod segfault_coredump;
#[path = "scenarios/cascading_load_shed.rs"]
mod cascading_load_shed;
