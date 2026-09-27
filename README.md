# systemd-sentry

> **Autonomous, zero-trust systemd supervisor in pure Rust.**  
> Intercepts crashes at the D-Bus and kernel layer, slices causal logs, provides instant root-cause analysis via local or cloud AI, and enforces deterministic circuit-breaking to halt flapping.

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Language: Rust](https://img.shields.io/badge/Language-Pure%20Rust-orange.svg)](https://www.rust-lang.org/)
[![Platform: Linux / systemd](https://img.shields.io/badge/Platform-Linux%20%2F%20systemd-red.svg)](https://systemd.io/)
[![Zero C Dependencies](https://img.shields.io/badge/Dependencies-Zero%20C%20Libs-green.svg)](#architecture)
[![Website: Online](https://img.shields.io/badge/Website-syntropd.github.io%2Fsentry-blue.svg)](https://syntropd.github.io/sentry/)

> 🌐 **Official Website & Architecture Guide:** [https://syntropd.github.io/sentry/](https://syntropd.github.io/sentry/)

---

## Why Sentry? (First Principles)

1. **The Flapping Trap**: When a service crashes, standard supervisors (`systemd`, `supervisord`, `k8s`) blindly restart it. If the cause is unrecoverable (syntax error in config, missing database migration, out of disk), the service restarts in a tight loop—wasting CPU, thrashing logs, and hammering dependencies.
2. **The 3 AM Triage Gap**: Operators are woken up to manually run `journalctl -u foo --since "5 min ago"`, inspect `/proc`, parse stack traces, and guess what broke.
3. **The Solution**: Sentry operates natively on the system D-Bus. When a failure occurs, it slices the causal binary journal window, extracts cgroup v2 & kernel PSI telemetry, diagnoses root cause using local or cloud AI, and stops retries if a permanent defect is detected.

---

## Zero-Trust Architecture (The AI Never Gets Root)

Sentry treats LLM inference strictly as an **untrusted advisory component**:

* **No Shell Execution**: The AI engine has zero access to shell execution (`sh`, `bash`, `exec`). It can never run arbitrary scripts or modify files.
* **Deterministic Whitelists**: Remediation is limited to hardcoded state transitions (`RestartUnit`, `ReloadUnit`, `ResetFailedUnit`) executed via typed D-Bus method calls if—and only if—explicitly permitted by `/etc/systemd-sentry/policy.toml`.
* **Kernel & Host Sandboxing**: `systemd-sentry` runs under an unprivileged `sentry` user with dropped capabilities (`ProtectSystem=strict`, `NoNewPrivileges=yes`, `MemoryDenyWriteExecute=yes`, `ProtectHome=yes`).

```
Crash Signal (D-Bus)
       │
       ▼
Telemetry Slicer (Journal + cgroups v2 + PSI)
       │
       ▼
Advisory AI Engine (routerd / llama.cpp / Cloud API)
       │  (JSON Diagnosis Only)
       ▼
Deterministic Policy Engine (policy.toml + Circuit Breaker)
       │  (Verified Against Whitelist)
       ▼
Guarded D-Bus Action (Restart, Backoff, or Lockout) ──> Desktop Toast / Alert
```

---

## Quickstart (60 Seconds)

### 1. Installation
Clone and run the zero-trust installer:
```bash
git clone https://github.com/syntropd/sentry.git
cd sentry
sudo ./install/install.sh
```

The installer:
* Provisions the unprivileged `sentry` user and adds it to `systemd-journal`.
* Sets up `/run`, `/var/log`, and `/var/lib` paths via `tmpfiles.d`.
* Installs systemd units, D-Bus system bus policies, shell completions, and man pages.

### 2. Interactive Setup
Run the guided configuration wizard:
```bash
systemd-sentry --setup
```
The wizard auto-detects local routerd/llama.cpp instances, configures API keys with `systemd-creds` encryption, and tests connection health.

### 3. Start the Supervisor
```bash
sudo systemctl enable --now systemd-sentry.service
```

---

## Configuration

Sentry separates global daemon settings from per-unit safety policies.

### Global Configuration (`/etc/systemd-sentry/config.toml`)
```toml
[daemon]
socket_path = "/run/systemd-sentry/sentry.sock"
watchdog_sec = 15
max_rss_bytes = 13631488    # 13 MiB limit before load shedding

[provider]
provider_type = "openai"    # "llamacpp", "openai", "fallback"
endpoint = "http://127.0.0.1:32768/v1"
model = "fast"
timeout_secs = 10
# api_key = "..."           # When using systemd-creds, loaded automatically
```

### Safety Policy & Circuit Breakers (`/etc/systemd-sentry/policy.toml`)
```toml
[global]
protected_units = [
    "systemd-journald.service",
    "systemd-logind.service",
    "systemd-udevd.service",
    "systemd-resolved.service",
    "dbus.service",
    "sshd.service",
    "systemd-sentry.service"
]
allowed_actions = ["RestartWithBackoff", "Reload", "ResetFailed", "NotifyOnly"]
rate_limit_per_minute = 10

[circuit]
max_failures = 3
window_secs = 60
cooldown_secs = 30          # Exponential backoff up to max_cooldown_secs
max_cooldown_secs = 1800
flap_threshold = 3          # Permanent lockout after 3 trips
flap_window_secs = 900

[units."nginx.service"]
allowed_actions = ["Reload", "RestartWithBackoff"]
max_failures = 5
auto_remediate = true
```

---

## Documentation

* 🏛️ [System Architecture](docs/architecture.md): Subsystems, dataflow, and zero-trust guarantees.
* 🛡️ [Hardening & Systemd Integrations](docs/hardening.md): Kernel capabilities, `systemd-creds`, `systemd-oomd`, `systemd-coredump`, `systemd-inhibit`, `systemd-resolved`, `systemd-networkd`, `systemd-timesyncd`, `systemd-pstore`, and `systemd --user`.
* 📜 [Policy Reference Guide](docs/policy_reference.md): Drop-in `.d/` precedence, circuit state machines, and flap lockout.
* 🧠 [Diagnostic Providers & Inference](docs/providers.md): Edge routerd, llama.cpp, OpenAI-compatible APIs, and fallback heuristics.
* 🔌 [Model Context Protocol (MCP)](docs/mcp.md): Stdio MCP server setup for Claude Desktop, Cursor, and agent runtimes.
* 🔌 [Local IPC & API Specification](docs/ipc_api.md): UNIX socket RPC and `SO_PEERCRED` authorization.
* 🐧 [Unix & Torvalds Design Principles](docs/design_principles.md): Mechanism vs. policy and resource thrift.

---

## CLI & Operator Tooling

Inspect system health, review post-mortems, stream real-time events, and manage circuit breakers:

```bash
# Check daemon status and supervised units (--json supported)
systemd-sentry status

# Pre-flight syntax validation of config.toml and policy.d/*.toml (nginx -t style)
systemd-sentry check

# Stream real-time failure events, triage reports, and circuit breaker state
systemd-sentry monitor

# Run immediate on-demand AI triage on a live or degraded service
systemd-sentry triage nginx.service --since "10 min ago"

# List recent incident post-mortems
systemd-sentry incidents

# View full forensic triage report for a specific incident
systemd-sentry inspect inc-20260923-01

# Manually reset a tripped circuit breaker
systemd-sentry reset nginx.service

# Generate shell completions (bash, zsh, fish)
source <(systemd-sentry completions bash)
```

---

## Desktop Toasts & Alerts

When a service fails or a circuit breaker trips, Sentry queries `systemd-logind` to find active graphical sessions (Wayland / X11) and dispatches native desktop toasts:

> **🚨 Service Failure: nginx.service**  
> *Root Cause*: Syntax error in `/etc/nginx/nginx.conf` line 42.  
> *Action*: Restart prevented to avoid flapping. Circuit breaker active.  
> `[ Inspect Report ]` `[ Mute ]`

---

## Specifications

* **Language**: 100% Pure Rust (no C dynamic libraries: `zbus`, pure Rust socket writer, `rustls`).
* **Source Constraints**: Strictly $\le 256$ lines per file; single function per file.
* **Testing**: 1:1 unit QA tests for every function, edge test suite, and `cargo-fuzz` targets.
* **System Integration**: `Type=notify`, `WatchdogSec`, socket activation, `sysusers.d`, `tmpfiles.d`, `dbus-1` policies, cgroups v2, kernel PSI, `systemd-coredump`, `systemd-inhibit`, `systemd-resolved`, `systemd-networkd`, `systemd-timesyncd`, `systemd-pstore`, and `systemd --user`.
* **Design Principles**: Built on [Unix Philosophy, Torvalds Pragmatism, and systemd Integration](docs/design_principles.md).

---

## License

[Apache 2.0](LICENSE) © syntropd & Contributors
