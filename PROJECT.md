# Project: systemd-sentry (Hardening & Edge Cases)

## Architecture
`systemd-sentry` (`sentry`) is an autonomous, zero-trust system supervisor written in 100% pure Rust that brings intelligent root-cause triage, deterministic circuit-breaking, and guarded self-healing to systemd-managed Linux environments.

### Core Principles & Hard Constraints
1. **100% Pure Rust**: Zero dynamic C library dependencies. No linkage to `libsystemd.so`, `libdbus-1.so`, `libssl.so`, `libcrypto.so`, `libzstd.so`, or `liblz4.so`. Pure Rust decompression (`ruzstd`, `lz4_flex`), pure Rust D-Bus via `zbus`, pure Rust socket writer for `sd_notify`/watchdog, pure Rust TLS via `rustls`.
2. **Strict File Limits**: Every source file must contain strictly at most **256 physical lines of code**. Enforced by `./scripts/check_loc.sh`.
3. **Single-Function Isolation**: Every file must be limited to a **single primary function** (or a single struct + constructor/handler).
4. **Zero-Trust Safety & Container Resilience**: Resilient and warning-free operation in unprivileged containers (Docker, LXC) and virtualized hypervisors. Bounded memory (<15MB RSS target).
5. **Zero Tolerance on Naming**: Absolute ban on 'syntry' repo-wide. CLI and project name is strictly 'sentry' or 'systemd-sentry'.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| F01 | Zero C Dynamic Deps | 100% pure Rust binaries linking zero dynamic C libraries | HM5 | Base |
| F02 | Strict File Line Limit | Automated verification asserting every source file <= 256 physical LOC | HM5 | Base |
| F03 | Single-Function Isolation | Every source file limited to a single primary function or struct | HM5 | Base |
| F04 | Pure Rust `sd_notify` | Non-blocking Unix datagram socket writer | HM5 | Base |
| F05 | Pure Rust Watchdog Heartbeat | Automated background ticker emitting WATCHDOG=1 at T/2 | HM5 | Base |
| F06 | Native Socket Activation | Base parsing of $LISTEN_FDS and $LISTEN_FDNAMES | HM4 | Base |
| F07 | Pure Rust D-Bus Listener | Passive zbus listener for systemd unit events | HM5 | Base |
| F08 | Journal Export Ingestion | Pure Rust stream parser for Journal Export Format | HM5 | Base |
| F09 | cgroups v2 Telemetry | Base parser for /sys/fs/cgroup/ | HM2 | Base |
| F10 | PSI Pressure Telemetry | Base parser for /proc/pressure/ | HM2 | Base |
| F11 | systemd-coredump Extraction | Base coredump stack trace extraction | HM1 | Base |
| F12 | Logind Session Discovery | zbus discovery of active graphical user sessions | HM5 | Base |
| F13 | Local routerd Provider | HTTP client for routerd OpenAI-compatible /v1 | HM3 | Base |
| F14 | Local llama.cpp Provider | Compatibility with llama.cpp server | HM3 | Base |
| F15 | Cloud OpenAI Provider | Pure Rust TLS client for OpenAI-compatible endpoints | HM3 | Base |
| F16 | Strict Diagnostic Payload | Serde schema for triage payloads | HM3 | Base |
| F17 | JSON Sanitization & Repair | Resilient markdown stripping & bracket slicing pipeline | HM3 | Base |
| F18 | Deterministic Fallback Triage | Heuristic triage for signals and exit codes | HM3 | Base |
| F19 | MCP Server Protocol | JSON-RPC 2.0 stdio server | HM5 | Base |
| F20 | MCP Tools & Resources | Tools (get_incident, etc.) and URI resources | HM5 | Base |
| F21 | Advisory LLM Zero-Trust Gate | Closed enum enforcement for remediation actions | HM5 | Base |
| F22 | Sliding-Window Circuit Breaker | Deterministic unit failure tracker & state machine | HM5 | Base |
| F23 | Flapping Service Lockout | 15-minute extended window detecting flapping services | HM5 | Base |
| F24 | Declarative Policy Engine | Parser & enforcer for /etc/systemd-sentry/policy.toml | HM5 | Base |
| F25 | Desktop Toast Notifications | Delivery of desktop toast alerts via user session bus | HM5 | Base |
| F26 | Terminal Wall Alerts | Non-blocking ANSI-sanitized broadcast to /dev/pts/* | HM5 | Base |
| F27 | Interactive CLI Setup Wizard | `sentry --setup` probing routerd/llama.cpp, live ping validation | HM5 | Base |
| F28 | Operator CLI Subcommands | Subcommands: status, incidents, inspect, reset, check, triage, monitor, completions | HM5 | Base |
| F29 | 1:1 Unit QA Test Suite | 1:1 mirroring of production functions into qa/unit/ | HM5 | Base |
| F30 | Edge-Case Stress Suite | Stress tests for corrupted streams, storms, malformed JSON | HM5 | Base |
| F31 | Cargo-Fuzz Targets | 4 fuzz targets in qa/fuzz/ | HM5 | Base |
| F32 | Crashing Service Fixtures | Simulated crashing services (segfault, oom, flapper) | HM5 | Base |
| F33 | Idempotent Installer | install/install.sh deploying binaries, units, configs | HM5 | Base |
| F34 | Hardened Systemd Service Unit | systemd-sentry.service with strict sandboxing | HM5 | Base |
| F35 | Systemd Socket Activation Unit | systemd-sentry.socket configuring socket activation | HM4 | Base |
| F36 | Sysusers & Tmpfiles | sysusers.d and tmpfiles.d configuration | HM5 | Base |
| F37 | D-Bus System Policy | dbus-1 policy governing sentry D-Bus permissions | HM5 | Base |
| F38 | Verification with systemd-analyze | Automated unit file verification | HM5 | Base |
| F39 | Comprehensive Docs & Website | Architecture, policy reference, provider guides | HM5 | Base |
| F40 | 100% E2E Verification Pass | Complete execution and 100% pass of Tiers 1-4 E2E suite | HM5 | Base |
| F41 | Adversarial Coverage Hardening | Tier 5 adversarial verification loop | HM5 | Base |
| F42 | Compressed Coredump Metadata Extraction | Pure Rust streaming decompression (.zst/.lz4) bounded <64KB, ELF headers & notes parsing, dynamic two-pass xattr reader | HM1 | R1 |
| F43 | Container & Virtualized Host Fallback | Graceful warning-free degradation when PSI/cgroups absent, synthetic /proc/loadavg & /proc/<pid>/statm metrics | HM2 | R2 |
| F44 | Adaptive LLM Inference Timeout & Fallback | LatencyTracker (EMA/p95), ProviderBreaker circuit, clean Tokio timeout abort, zero-latency DeterministicFallbackEngine | HM3 | R3 |
| F45 | Multi-Socket Activation Disambiguation | Multi-tier matching ($LISTEN_FDNAMES, socket bound path, rustix S_IFSOCK, SOCK_STREAM, AF_UNIX, SO_ACCEPTCONN) | HM4 | R4 |
| F46 | Hardening Integration & Acceptance Verification | 100% pass cargo test --workspace, qa test suite, check_loc, check_deps, <15MB RSS verification, git push origin main | HM5 | Acceptance |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| HM1 | Compressed Coredump Metadata Extraction | R1: `crates/sentry-driver/src/coredump/`, `crates/sentry-core/src/models/coredump.rs`, `qa/unit/src/coredump/` | None | DONE |
| HM2 | Unprivileged Container & Virtualized Host Fallback | R2: `crates/sentry-driver/src/psi/`, `crates/sentry-driver/src/cgroup/`, `crates/sentry-daemon/`, `qa/unit/src/psi/`, `qa/unit/src/cgroup/` | None | DONE |
| HM3 | Adaptive LLM Inference Timeout & Fallback | R3: `crates/sentry-diagnostic/src/circuit/`, `crates/sentry-diagnostic/src/engine.rs`, `qa/unit/src/diagnostic/` | None | DONE |
| HM4 | Multi-Socket Activation Disambiguation | R4: `crates/sentry-driver/src/activation/`, `crates/sentry-daemon/src/ipc/listener.rs`, `qa/unit/src/activation/` | None | DONE |
| HM5 | Hardening Integration, Acceptance & Push | Full workspace & QA tests, check_loc.sh, check_deps.sh, RSS check, git commit & push origin main | HM1, HM2, HM3, HM4 | DONE |

## Interface Contracts

### Coredump Extraction ↔ Core Models (HM1)
- `CoredumpXattrs`: Extended with `proc_status: Option<String>`, `cmdline: Option<String>`, `uid: Option<u32>`, `gid: Option<u32>`, `hostname: Option<String>`, `rlimit: Option<String>`, `timestamp: Option<u64>`, `extra: HashMap<String, String>`.
- `ElfCrashHeader`: Struct containing `class: u8`, `endian: u8`, `machine: u16`, `signal: Option<i32>`, `signal_code: Option<i32>`, `fault_addr: Option<u64>`, `pid: Option<u32>`, `comm: Option<String>`, `cmdline: Option<String>`, `mapped_files: Vec<String>`.
- `extract_elf_crash_headers(reader: &mut impl Read) -> Result<ElfCrashHeader, CoredumpError>`: Reads at most 64 KiB (`reader.take(65536)`).
- Streaming decoders: `ruzstd::decoding::StreamingDecoder` (.zst) and `lz4_flex::frame::FrameDecoder` (.lz4). Pure Rust, 0 dynamic C libs.

### Resilient Telemetry ↔ Daemon / Triage (HM2)
- `collect_system_psi_resilient(proc_path: &Path) -> PressureTelemetry`: Falls back to `/proc/loadavg` and `/proc/meminfo` before `PressureTelemetry::synthetic_zero(None)`.
- `collect_cgroup_telemetry_resilient(base_cgroup: &Path, unit_name: &str, pid: Option<u32>) -> CgroupTelemetry`: Probes multiple cgroup paths (`system.slice/<unit>`, `<unit>`, `docker/<unit>`), procfs (`/proc/<pid>/statm`, `/proc/<pid>/stat`), or `CgroupTelemetry::synthetic_fallback(...)`.
- `PressureTelemetry::is_synthetic(&self) -> bool`, `CgroupTelemetry::is_synthetic(&self) -> bool`.

### Adaptive Timeout & Provider Breaker ↔ Diagnostic Engine (HM3)
- `LatencyTracker`: Bounded `[u32; 64]` ring buffer. Tracks moving average (EMA, $\alpha=0.2$) and 95th percentile (p95).
- `AdaptiveTimeoutConfig`: `min_timeout: 1.5s`, `max_timeout: 15.0s`, `initial_timeout: 5.0s`, `failure_threshold: 3`, `cooldown: 30s..300s`.
- `ProviderBreaker`: Closed, Open, HalfOpen. When Open, immediately transfers execution to `DeterministicFallbackEngine::triage` in $<10\mu$s with 0 network calls.
- `tokio::time::timeout(adaptive_timeout, provider.complete(prompt))`: Dropping future aborts TCP connection cleanly without blocking Tokio worker threads.

### Multi-Socket Disambiguation ↔ Daemon Listener (HM4)
- `disambiguate_socket(sockets: &mut Vec<ActivatedSocket>, target_names: &[&str], target_path: Option<&Path>) -> Option<ActivatedSocket>`:
  - Tier 1: Filesystem path match via `getsockname` on AF_UNIX socket.
  - Tier 2: Name match against target names ($LISTEN_FDNAMES).
  - Tier 3: Inode & type check: `S_IFSOCK`, `AF_UNIX`, `SOCK_STREAM`, `SO_ACCEPTCONN == true`.
  - Tier 4: Fallback to manual bind if no candidate matches.

## Code Layout
```
/home/jeryd/Projects/syntropd/sentry/
├── Cargo.toml
├── crates/
│   ├── sentry-core/src/models/{coredump.rs, psi.rs, cgroup.rs}
│   ├── sentry-driver/src/
│   │   ├── coredump/{xattr_reader.rs, stream_reader.rs, elf_header_parser.rs, elf_note_parser.rs, elf_extractor.rs, dir_scanner.rs, mod.rs}
│   │   ├── psi/{psi_resilient_collector.rs, proc_loadavg_reader.rs, psi_reader.rs, psi_collector.rs, mod.rs}
│   │   ├── cgroup/{cgroup_resilient_collector.rs, proc_statm_extractor.rs, cgroup_locator.rs, cpu_reader.rs, memory_reader.rs, io_reader.rs, mod.rs}
│   │   └── activation/{socket_inspector.rs, disambiguate.rs, model.rs, parser.rs, name_parser.rs, mod.rs}
│   ├── sentry-diagnostic/src/
│   │   ├── circuit/{mod.rs, config.rs, state.rs, latency_tracker.rs, breaker.rs}
│   │   ├── engine.rs
│   │   └── provider/factory.rs
│   └── sentry-daemon/src/
│       ├── daemon/incident_manager.rs
│       ├── commands/cmd_triage.rs
│       └── ipc/listener.rs
├── qa/unit/src/
│   ├── coredump/
│   ├── psi/
│   ├── cgroup/
│   ├── diagnostic/
│   └── activation/
└── scripts/{check_loc.sh, check_deps.sh}
```
