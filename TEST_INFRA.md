# TEST_INFRA — Test Infrastructure & Coverage Specification

## 1. Testing Philosophy
systemd-sentry enforces strict opaque-box (black-box) testing principles:
- **Zero Ambient Authority**: LLM advisory recommendations are never trusted. Tests treat the daemon and subsystems as black boxes verified against external observable behaviors (D-Bus signals, socket datagrams, exit codes, JSON-RPC responses).
- **Authoritative Expected Outputs**: Every test derives expected outputs from normative specifications: Linux kernel cgroup v2 / PSI specs, systemd Journal Export Stream format, RFC 3339 / UUIDv4, JSON-RPC 2.0 / MCP spec, and `/etc/systemd-sentry/policy.toml` contracts.
- **Deterministic Isolation**: Tests do not access live external cloud networks or real systemd PID 1 during execution. Pure Rust mock sockets, virtual memory limits, and deterministic event injection ensure reproducible outcomes.
- **Progressive Testability**: Tests validate contracts incrementally across 4 distinct tiers from individual feature invariants to cascading failure storms.

## 2. Feature Inventory Coverage Matrix
| Requirement | Feature ID | Feature Description | Tier 1 (Coverage) | Tier 2 (Boundary) | Tier 3 (Cross) | Tier 4 (Scenario) |
|---|---|---|---|---|---|---|
| R1 | F01-F03 | Pure Rust, Zero-C, <=256 LOC, Single Function | 5 Tests (qa/e2e) | 5 Tests (qa/edge) | Verified | Validated |
| R2 | F04-F06 | sd_notify, Watchdog Ticker, Socket Activation | 5 Tests (qa/e2e) | 5 Tests (qa/edge) | Verified | Validated |
| R2 | F07-F12 | D-Bus Listener, Journal Ingest, PSI, Coredump | 5 Tests (qa/e2e) | 5 Tests (qa/edge) | 3 Tests | Validated |
| R3 | F13-F18 | LLM Providers (routerd/OpenAI), Schema, Triage | 5 Tests (qa/e2e) | 5 Tests (qa/edge) | 3 Tests | Validated |
| R3 | F19-F20 | MCP Protocol (JSON-RPC 2.0, Tools, Resources) | 5 Tests (qa/e2e) | 5 Tests (qa/edge) | 2 Tests | Validated |
| R4 | F21-F24 | Advisory Gate, Circuit Breaker, Flap Lockout | 5 Tests (qa/e2e) | 5 Tests (qa/edge) | 3 Tests | 3 Tests |
| R5 | F25-F28 | Desktop Toast, Terminal Wall, Setup Wizard, CLI | 5 Tests (qa/e2e) | 5 Tests (qa/edge) | 2 Tests | Validated |
| R6 | F29-F32 | 1:1 QA Mirror, Corrupt Stream, Fuzz, Fixtures | 5 Tests (qa/e2e) | 5 Tests (qa/edge) | Verified | 2 Tests |
| R7 | F33-F39 | Packaging, Installer, Sandboxing, Docs, Policy | 5 Tests (qa/e2e) | 5 Tests (qa/edge) | Verified | Validated |

## 3. Four-Tier Test Architecture
- **Tier 1: Feature Coverage (`qa/e2e/tier1_*.rs`)**:
  Covers primary behavior and contract semantics for R1 through R7 (>= 5 test cases per feature, 35+ total).
- **Tier 2: Boundary & Corner Cases (`qa/edge/tier2_*.rs`)**:
  Stresses non-UTF8 logs, truncated journal streams, malformed LLM JSON, 0/max limits, terminal injection strings, and permission limits (>= 5 test cases per feature, 35+ total).
- **Tier 3: Cross-Feature Interactions (`qa/e2e/tier3_*.rs`)**:
  Validates pairwise integrations: Coredump + Triage Synthesis, Circuit Breaker + Flap Lockout + Manual CLI Reset, Journal Ingestion + PSI Telemetry Correlation, Advisory LLM Gate + Policy Whitelist.
- **Tier 4: Real-World Scenarios (`qa/edge/tier4_*.rs`)**:
  Simulates operational extremes: 10,000 crashes/sec failure storm, OOM memory exhaustion (`oom_service`), flapper oscillation (`flapper_service`), segfault coredump triage (`segfault_service`), and cascading load shedding.

## 4. Fuzzing Infrastructure (`qa/fuzz/`)
- Harness using `libfuzzer-sys` targeting deserialization and decoding boundaries:
  1. `fuzz_journal_parser`: Journal Export Format streaming, binary length prefixes.
  2. `fuzz_dbus_decoder`: D-Bus binary wire framing, variant decoding.
  3. `fuzz_psi_parser`: Linux kernel `/proc/pressure/*` metric token stream.
  4. `fuzz_json_triage_decoder`: Markdown fence stripping, bracket repair, payload deserialization.

## 5. Crashing Fixtures (`qa/fixtures/`)
- Standalone crash binaries with deterministic triggers:
  1. `segfault_service`: Volatile null pointer dereference (`SIGSEGV`).
  2. `oom_service`: Unbounded memory dirtying page-by-page until kernel OOM killer (`SIGKILL`).
  3. `flapper_service`: High-frequency restart cycle with `READY=1` heartbeat and rapid exit 1.

## 6. Runner Invocations & Verification Commands
```bash
# 1. Execute full E2E & Edge test suite
./qa/run_e2e.sh

# 2. Run Cargo tests directly
cargo test --manifest-path qa/Cargo.toml

# 3. Verify cargo-fuzz targets compile cleanly
cargo check --manifest-path qa/fuzz/Cargo.toml --all-targets

# 4. Verify all files adhere to <= 256 LOC
wc -l $(find qa/ -type f \( -name "*.rs" -o -name "*.sh" -o -name "*.toml" \))
```

## 7. Pass Thresholds & Quality Criteria
- **Pass Rate**: 100% (0 failures, 0 panics, 0 flaky tests).
- **File Length Limit**: Strictly <= 256 lines of code per file across all files.
- **Fuzzing Build Cleanliness**: 0 compiler warnings or errors in `qa/fuzz/`.
- **Failure Storm Performance**: Memory RSS growth < 15MB under 10,000 synthetic crashes/sec.
- **Circuit Breaker Accuracy**: Breaker trips in <= 3 events under default 60s failure window.
