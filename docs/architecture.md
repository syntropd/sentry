# System Architecture

`systemd-sentry` is a zero-trust, autonomous failure supervisor and triage engine written in 100% pure Rust. It operates as a native `systemd` daemon, supervising services on Linux hosts while strictly adhering to a <15MB RSS memory budget.

---

## 1. High-Level Subsystems

```
+-----------------------------------------------------------------------------------+
|                                 Linux Host                                        |
|                                                                                   |
|  +--------------------+   +-----------------------+   +------------------------+  |
|  |   systemd-journald |   |  org.freedesktop      |   |  Linux Kernel          |  |
|  |   /run/systemd/    |   |  systemd1 (D-Bus)     |   |  /sys/fs/cgroup (v2)   |  |
|  |   journal/io...    |   |  Unit state signals   |   |  /proc/pressure (PSI)  |  |
|  +---------+----------+   +-----------+-----------+   +-----------+------------+  |
+------------|--------------------------|---------------------------|---------------+
             |                          |                           |
             v                          v                           v
+-----------------------------------------------------------------------------------+
| crates/sentry-driver                                                              |
|   • Journal Export Stream Parser (bounded chunking, resilient UTF-8 resync)       |
|   • D-Bus Signal Listener (UnitNew, JobRemoved, PropertiesChanged)                |
|   • Kernel Telemetry Collector (zero-alloc PSI stack reader, cgroups v2 memory)   |
|   • Extended Attribute Coredump Scanner (rustix getxattr, no binary buffering)   |
|   • Subsystem Drivers: inhibit, resolved, networkd, timesyncd, pstore, user-bus   |
+-----------------------------------------------------------------------------------+
                                        |
                                        v
+-----------------------------------------------------------------------------------+
| crates/sentry-safety (Deterministic Gatekeeper)                                   |
|   • Sliding-Window Circuit Breakers (Instant monotonic clock)                     |
|   • Exponential Cooldown Backoff (cooldown = base * 2^(trips-1))                  |
|   • Flapping Lockout Engine (locks out flappers after 3 trips in 900s)            |
|   • Drop-in Policy Engine (/etc/systemd-sentry/policy.d/*.toml)                   |
|   • Protected Units Blacklist (journald, logind, resolved, dbus, sshd, sentry)   |
+-----------------------------------------------------------------------------------+
                                        |
                   +--------------------+--------------------+
                   |                                         |
                   v                                         v
+------------------------------------+   +------------------------------------------+
| crates/sentry-diagnostic           |   | crates/sentry-mcp                        |
|   • Pluggable AI Inference         |   |   • Stdio Model Context Protocol         |
|     (routerd, llama.cpp, OpenAI)   |   |   • Bounded Ring Buffer Storage          |
|   • systemd-creds Secret Discovery |   |     (MAX_STORED_INCIDENTS = 100)         |
|   • Zero-Network Fallback Engine   |   |   • Tools, Prompts, and Resources        |
|   • Bounded HTTP Reader (<= 512KB) |   +------------------------------------------+
+------------------------------------+
                   |
                   v
+-----------------------------------------------------------------------------------+
| crates/sentry-daemon & Operator CLI Suite                                         |
|   • Local UNIX Socket (/run/systemd-sentry/sentry.sock, SO_PEERCRED auth)         |
|   • Native Socket Activation ($LISTEN_FDS fd 3) & Type=notify / WatchdogSec       |
|   • 2MB Hysteresis Load Shedder (sheds at 13MB RSS, recovers below 11MB RSS)      |
|   • Sysexits-Compliant Operator CLI (11 subcommands)                              |
+-----------------------------------------------------------------------------------+
```

---

## 2. Zero-Trust Security Guarantees

1. **Advisory AI Only**: Large Language Models (LLMs) and diagnostic providers have **zero shell access**, zero command execution permissions, and cannot invoke system APIs. All proposed remediations must pass through `sentry-safety` policy verification.
2. **Deterministic Circuit Breaking**: Even if an LLM advises continuous restarts, `sentry-safety` strictly enforces failure rate limits, exponential cooldown, and permanent flap lockouts.
3. **No Network Listening Ports**: Sentry does not open or listen on any TCP or UDP ports. All incoming queries use the local UNIX socket authorized by the kernel via `SO_PEERCRED`.
4. **Credential Isolation**: API keys are never stored in plaintext on disk. They are decrypted into a dedicated memory-mapped tmpfs using `systemd-creds` and TPM2 hardware sealing.
5. **Memory & Syscall Ceiling**: All buffers are bounded. The daemon continuously monitors `/proc/self/statm` and sheds background tasks at 13MB RSS to maintain strict operation on low-spec VPS hosts (512MB RAM).

---

## 3. Subsystem Integration Matrix

| Subsystem | Bus / Path | Primary Role |
| :--- | :--- | :--- |
| `systemd-journald` | `/run/systemd/journal/io...` | Zero-copy causal stream slicing & UTF-8 resync |
| `systemd-oomd` | `/proc/pressure/memory`, cgroups v2 | PSI pressure telemetry & OOM kill detection |
| `systemd-coredump` | `/var/lib/systemd/coredump/` | Bounded xattr metadata inspection (no binary reads) |
| `systemd-creds` | `$CREDENTIALS_DIRECTORY` | TPM2 encrypted secret discovery |
| `systemd-inhibit` | `org.freedesktop.login1` | Shutdown/sleep delay lock & alert suppression |
| `systemd-resolved` | `org.freedesktop.resolve1` | DNS outage correlation & lookup verification |
| `systemd-networkd` | `org.freedesktop.network1` | Link carrier state correlation (`OperationalState`) |
| `systemd-timesyncd`| `org.freedesktop.timesync1` | NTP synchronization & TLS validity verification |
| `systemd-pstore` | `/sys/fs/pstore/` | Bounded kernel panic log harvesting |
| `systemd --user` | `/run/user/<UID>/bus` | Rootless session discovery & per-user supervision |

