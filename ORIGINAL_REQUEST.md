# Original User Request

## Initial Request — 2026-09-24T00:16:09Z

Build **systemd-sentry**, an autonomous, zero-trust system supervisor written in 100% pure Rust that brings intelligent root-cause triage, deterministic circuit-breaking, and guarded self-healing to `systemd`-managed Linux environments.

Working directory: `/home/jeryd/Projects/syntropd/sentry`
Integrity mode: development

## Requirements

### R1. Architectural Constraints & Code Structure
- Must be written in **100% pure Rust** with zero C dynamic library dependencies (pure Rust D-Bus via `zbus`, pure Rust socket writer for `sd_notify`/watchdog, pure Rust TLS via `rustls`, zero `libsystemd.so` or `libdbus-1.so`).
- **Strict File Limits**: Every source file must contain at most **256 lines of code**.
- **Single-Function Isolation**: Every file must be limited to a **single primary function** (or a single struct + constructor/handler).
- Logical, nested directory structure with clean module re-exports.

### R2. System Integration & Deep Host Connectivity
- **Native systemd Citizen**:
  - `Type=notify` and `WatchdogSec` heartbeat implementation writing directly to `$NOTIFY_SOCKET`.
  - Native socket activation support parsing `$LISTEN_FDS` (file descriptors `3..N`).
  - Standard packaging: `sysusers.d` (provisions `sentry` user in `systemd-journal` group), `tmpfiles.d` (provisions runtime/log/state dirs), `dbus-1/system.d` policy, and hardened systemd service unit.
- **Subsystem Hooks**:
  - Passive D-Bus listener (`org.freedesktop.systemd1`) tracking unit lifecycle and failure events.
  - Streaming journal ingestion from `systemd-journald` socket (`/run/systemd/journal/io.systemd.journal`).
  - Kernel telemetry parser for cgroups v2 (`/sys/fs/cgroup/`) and PSI (`/proc/pressure/{cpu,memory,io}`).
  - `systemd-coredump` extraction for top crash backtraces on `SIGSEGV`/`SIGABRT`.
  - `systemd-logind` session discovery for desktop notifications.

### R3. Diagnostic Engine & Dual LLM Providers
- Pluggable provider layer supporting:
  1. Local inference via the routerd OpenAI-compatible endpoint and `llama.cpp` server.
  2. Cloud OpenAI-compatible endpoints with API key support (`/v1/chat/completions`).
- Strict JSON schema enforcement for diagnostic payloads (root cause, evidence, severity, proposed remediation).
- Model Context Protocol (MCP) server implementation allowing external AI tools to query incident logs and unit telemetry.

### R4. Zero-Trust Safety Engine & Circuit Breaker
- The LLM is strictly advisory—it has **no shell access** and cannot execute arbitrary commands.
- Deterministic sliding-window circuit breaker that tracks failure rates and locks out flapping services.
- Declarative `/etc/systemd-sentry/policy.toml` controlling allowed actions (e.g. restart with backoff, reload, reset-failed).

### R5. User Experience, Notifications & Setup Wizard
- Desktop toast alerts via `org.freedesktop.Notifications` dispatched to active graphical sessions.
- Terminal wall alerts for headless sessions.
- Interactive CLI setup wizard (`systemd-sentry --setup`) that detects the local routerd endpoint, prompts for API keys, validates with a live ping, and writes configuration.
- Operator CLI subcommands: `status`, `incidents`, `inspect <id>`, `reset <unit>`.

### R6. QA, Edge Testing & Fuzzing
- **1:1 Function Tests**: Every production function has a corresponding unit QA test in `qa/unit/`.
- **Edge-Case Suite**: Stress tests for corrupted journal streams, non-UTF8 logs, 10,000 crashes/sec failure storms, and malformed LLM responses.
- **Fuzzing**: `cargo-fuzz` targets for journal parsing, D-Bus decoding, PSI text parsing, and JSON triage decoding.
- **Crash Fixtures**: Simulated crashing services (`segfault_service`, `oom_service`, `flapper_service`).

### R7. Installation, Documentation & Website
- `install/install.sh`: Idempotent installer managing binary deployment, sysusers, tmpfiles, D-Bus policy, unit files, completions, and man pages.
- `README.md` & `website/`: Built from first principles, zero trust, and designed for end-user clarity.
- `docs/`: Architecture, hardening, policy reference, provider guides, and MCP spec.

## Acceptance Criteria

### Compilation & Static Analysis
- [ ] `cargo check --workspace --all-targets` passes with 0 warnings/errors.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes cleanly.
- [ ] A verification script asserts that **zero files** exceed 256 lines of code.
- [ ] Binaries link with zero dynamic C dependencies (`ldd` shows no `libdbus` or `libsystemd`).

### Functional & Unit Verification
- [ ] `cargo test --workspace` passes 100% of 1:1 unit QA tests.
- [ ] Edge test suite passes (non-UTF8 log slicing, malformed LLM output, rapid flap rate limiting).
- [ ] All `cargo-fuzz` targets compile cleanly under `qa/fuzz/`.

### System & Packaging Verification
- [ ] `install/install.sh --dry-run` or validation passes syntax and permission checks.
- [ ] Systemd service, socket, sysusers, tmpfiles, and D-Bus policy files pass `systemd-analyze verify`.
- [ ] CLI `--setup` correctly validates local and OpenAI-compatible configuration.

## Follow-up — 2026-09-24T00:34:33Z

USER DIRECTIVE: Please do NOT use the name 'syntry' anywhere in the project. Do not use 'syntry' for binaries, symlinks, CLI commands, comments, documentation, logs, or test strings. The project and CLI name is strictly 'sentry' (or 'systemd-sentry'). Please ensure all current and future milestones adhere to this naming.

## Follow-up — 2026-09-24T00:58:44Z

USER DESIGN DIRECTIVE:
1. Unix Philosophy: Clean separation of mechanism and policy; composable standard streams (stdout/stderr/stdin); silence on success; standard exit codes; treat everything as a file/stream.
2. Linus Torvalds Pragmatism: Zero unnecessary abstraction layers or bloat; fast, clean, deterministic code; respect Linux kernel interfaces (/proc, /sys, cgroups v2, PSI); NEVER panic in runtime daemon paths; fail safely.
3. systemd Design Alignment: Support .d/ drop-in configuration directories (/etc/systemd-sentry/policy.d/*.toml); standard systemd exit codes; native D-Bus conventions; strict sandboxing. Ensure all upcoming milestones adhere to this.

## Follow-up — 2026-09-24T01:06:01Z

API / IPC CONTRACT FORMALIZED:
1. Local UNIX Socket IPC: /run/systemd-sentry/sentry.sock with SO_PEERCRED kernel credential authorization (for local CLI queries).
2. System D-Bus: org.freedesktop.SystemdSentry on system bus for systemd-idiomatic control.
3. AI Agent Protocol: Model Context Protocol (MCP) via stdio (systemd-sentry mcp) for agentic and tool integrations.
4. Zero Network Listeners: No TCP ports opened.
All assets created in systemd/ and documented in docs/ipc_api.md. Please ensure Milestone 3 (MCP/Inference) and Milestone 4 (CLI) adhere to this.

## Follow-up — 2026-09-24T01:16:15Z

CLI SUITE EXPANSION APPROVED:
Please incorporate the following 4 standard commands into the Milestone 4 (CLI & Operator Experience) specification and tests:
1. `systemd-sentry check`: Pre-flight syntax validation of config.toml and policy.d/*.toml drop-ins (nginx -t / systemd-analyze verify style, exit 0 or 78).
2. `systemd-sentry triage <unit>`: On-demand immediate AI triage of a running/degraded unit without waiting for failure.
3. `systemd-sentry monitor`: Live streaming event feed of crashes, trips, and triage events over UNIX socket / D-Bus signals.
4. `systemd-sentry completions <shell>`: Generates shell completion script (bash/zsh/fish) to stdout.
Updated docs/ipc_api.md and README.md accordingly.

## Follow-up — 2026-09-24T01:26:20Z

CRITICAL SYSTEMS AUDIT FINDINGS (from Opinionated Systems Engineer):
The auditor has identified key memory & syscall bottlenecks that must be remediated for low-spec hosts (512MB RAM VPS, <15MB RSS budget):
1. Bounded McpState: Replace unbounded Vec<DiagnosticPayload> in crates/sentry-mcp/src/storage/mcp_state.rs with a bounded VecDeque capped at MAX_STORED_INCIDENTS = 100 (evicting oldest) to prevent OOM kills during crash storms.
2. Clamp Journal Binary Field: Reduce MAX_FIELD_SIZE in crates/sentry-driver/src/journal/binary_field_reader.rs from 16 MiB down to 4 MiB (conforming to the low-memory budget).
3. Zero-Allocation D-Bus Signal Dispatch: In crates/sentry-driver/src/dbus/listener.rs:73-80, eliminate redundant String allocations; compare borrowed &str slices directly (m.as_str(), i.as_str()).
4. Persistent Watchdog Socket: In sentry-driver/src/notify, avoid opening/closing a UnixDatagram socket on every tick (eliminates 3 syscalls per tick).
5. Fixed-Buffer Procfs/Sysfs Reads: In psi_reader.rs and memory_reader.rs, read into a stack buffer [u8; 512] instead of fs::read_to_string (which reallocates because sysfs files report size 0).
6. Fix compilation in qa/unit: Create missing qa/unit/src/mcp/mod.rs and fix async trait / clone in test_engine.rs.
Please prioritize these fixes across Milestone 2 and Milestone 3.

## Follow-up — 2026-09-24T02:33:30Z

Teamwork Supervisor: Update from parent agent.
1. The zero-allocation LRU optimization for `CircuitBreakerRegistry` is implemented and verified (`1f6ee46`).
2. Milestone 4 (Operator CLI, Setup Wizard, Notifications, IPC Socket Client) and Milestone 5 (Daemon Supervisor, 2MB Hysteresis Load Shedder, Sysexits Exit Codes) are ALREADY fully implemented, tested, and passing all tests in `crates/sentry-daemon` and `qa/unit/src/daemon/`.
3. System integration packaging (`install/install.sh`), standard Unix roff manual pages (`man1`, `man5`, `man8`), shell completions (`bash`, `zsh`, `fish`), and comprehensive architecture/hardening/policy/provider/mcp docs are complete and verified.
4. The Opinionated Systems Engineer has completed an unsparing final audit and issued a "PRODUCTION READY & HARDENED" verdict.
5. All 253 tests pass, all 87 E2E tests pass, 4 cargo-fuzz targets compile cleanly, 282/282 source files strictly <= 256 LOC, and zero dynamic C dependencies.

## Follow-up — 2026-09-24T07:36:53Z

Harden and resolve edge cases across `systemd-sentry` in pure Rust, ensuring resilient operation in constrained environments (containers, compressed crash dumps, multi-socket systemd setups, and transient LLM latency).

Working directory: /home/jeryd/Projects/syntropd/sentry
Integrity mode: development

## Requirements

### R1. Compressed Coredump Metadata Extraction
Extract extended attributes (`user.coredump.*`) and bounded ELF crash headers from compressed coredump archives (`.zst` and `.lz4` files in `/var/lib/systemd/coredump`) without decompressing full multi-hundred megabyte crash binaries into heap memory.

### R2. Unprivileged Container & Virtualized Host Fallback Resilience
Ensure graceful, warning-free degradation when `/proc/pressure` (PSI) or `/sys/fs/cgroup` (cgroups v2) are absent, restricted, or unmounted (e.g. Docker, unprivileged LXC, or legacy kernels), providing synthetic fallback metrics so triage continues without failing.

### R3. Adaptive LLM Inference Timeout & Zero-Latency Fallback
Implement an adaptive timeout and failure circuit for LLM providers (llama.cpp, OpenAI-compatible APIs) that cleanly aborts slow inference calls and transfers execution immediately to `DeterministicFallbackEngine` without blocking the daemon or dropping events.

### R4. Multi-Socket Activation Disambiguation
Ensure socket activation supports complex environments with multiple listening descriptors by matching socket names from `$LISTEN_FDNAMES` (`sentry`, `systemd-sentry`, `sentry.socket`) or checking socket inode types, without relying on fixed descriptor indices.

## Acceptance Criteria

### Verification & Test Compliance
- [ ] `cargo test --workspace` passes with 100% success rate
- [ ] `cargo test --manifest-path qa/Cargo.toml` passes with 100% success rate across all test tiers
- [ ] `./scripts/check_loc.sh` passes with zero violations (all files strictly <= 256 physical LOC)
- [ ] `./scripts/check_deps.sh` passes with zero violations (zero dynamic C libraries linked; 100% pure Rust)
- [ ] Memory footprint remains strictly bounded within the <15MB RSS target on 512MB RAM VPS hosts
- [ ] All changes are committed and pushed to `origin main` on GitHub

## Verification Resources
- Test suites in `qa/unit/`, `qa/e2e/`, and `qa/edge/`
- Enforcing scripts in `scripts/check_loc.sh` and `scripts/check_deps.sh`
