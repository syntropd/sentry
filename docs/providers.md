# Diagnostic Providers & Inference Setup

`systemd-sentry` includes a pluggable diagnostic engine supporting local edge inference, cloud OpenAI-compatible endpoints, and a zero-network deterministic fallback engine.

---

## 1. Supported Providers

### Provider A: routerd (Default Local Hardware Routing Daemon)
`systemd-sentry` connects to `routerd` by default on TCP port 32768 for hardware-arbitrated, cost-optimized local and hybrid model routing.
```toml
# /etc/systemd-sentry/config.toml
[provider]
provider_type = "openai"
endpoint = "http://127.0.0.1:32768/v1"
model = "fast"
timeout_secs = 10
```

### Provider B: llama.cpp Server
Suitable for embedded or resource-constrained nodes running a lightweight `llama-server` binary:
```toml
[provider]
provider_type = "llamacpp"
endpoint = "http://127.0.0.1:8080"
model = "default"
timeout_secs = 10
```

### Provider C: OpenAI-Compatible APIs (Cloud / vLLM / LiteLLM)
For high-capacity cloud inference:
```toml
[provider]
provider_type = "openai"
endpoint = "https://api.openai.com"
model = "gpt-4o-mini"
timeout_secs = 15
# When using systemd-creds, omit api_key here; it is discovered via $CREDENTIALS_DIRECTORY
api_key = "sk-..."
```

### Provider E: Zero-Network Deterministic Fallback
When no AI provider is configured, or during total network partitions, `systemd-sentry` uses its internal deterministic expert system. It evaluates:
* Process exit status (e.g. `137` = OOM, `139` = Segfault, `143` = SIGTERM)
* `systemd-coredump` signal tags and top stack frames
* cgroup v2 memory counters and PSI spikes
* Journal error signatures

---

## 2. Resource Bounding & Security Guards

1. **Strict Response Limits**: HTTP response parsing is capped at **512 KiB** (`bounded_body.rs`). Any runaway or hallucinated payload is severed at the byte boundary to prevent heap exhaustion.
2. **Resilient JSON Sanitization**: Models occasionally return markdown fences (````json ... ````) or malformed syntax. Sentry's parsing pipeline extracts and validates structured JSON against strict domain schemas.
3. **Interactive Setup Wizard**: Run `systemd-sentry --setup` to automatically detect local inference daemons, ping endpoints, test model latency, and generate hardened configs.
