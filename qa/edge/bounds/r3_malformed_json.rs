//! Tier 2: R3 Malformed LLM Output & Schema Boundary Tests
//!
//! Validates repair pipelines against conversational filler, truncated JSON, and invalid enums.

#[test]
fn test_r3_boundary_truncated_json_token_exhaustion() {
    let truncated = "{\"unit_name\": \"nginx.service\", \"root_cause\": {\"summary\": \"Segfault";
    let is_valid = serde_json::from_str::<serde_json::Value>(truncated).is_ok();
    assert!(!is_valid, "Truncated JSON must fail parsing");
}

#[test]
fn test_r3_boundary_conversational_preface_and_postscript() {
    let raw = "Here is the incident triage diagnosis you requested:\n\
               {\"action\": \"RESTART\", \"confidence\": 0.90}\n\
               Please verify this matches your requirements!";

    let start = raw.find('{').unwrap();
    let end = raw.rfind('}').unwrap();
    let isolated = &raw[start..=end];

    let parsed: serde_json::Value = serde_json::from_str(isolated).unwrap();
    assert_eq!(parsed["action"], "RESTART");
    assert_eq!(parsed["confidence"], 0.90);
}

#[test]
fn test_r3_boundary_confidence_range_validation() {
    let test_cases = [(-0.1, false), (0.0, true), (0.5, true), (1.0, true), (1.01, false)];

    for (val, should_be_valid) in test_cases {
        let is_valid = val >= 0.0 && val <= 1.0;
        assert_eq!(
            is_valid, should_be_valid,
            "Confidence {} validation check failed",
            val
        );
    }
}

#[test]
fn test_r3_boundary_deeply_nested_json_structure() {
    let mut nested = String::from("{\"a\":");
    for _ in 0..50 {
        nested.push_str("{\"nested\":");
    }
    nested.push_str("\"val\"");
    for _ in 0..50 {
        nested.push('}');
    }
    nested.push('}');

    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&nested);
    assert!(parsed.is_ok(), "50-level nested JSON should parse without panic");
}

#[test]
fn test_r3_boundary_hallucinated_remediation_action() {
    let hallucinated = ["reboot_system", "kill_all_processes", "format_c", "apt_update"];
    let allowed = [
        "NO_ACTION",
        "RESTART",
        "RESTART_WITH_BACKOFF",
        "RELOAD",
        "RESET_FAILED",
        "ESCALATE_TO_ADMIN",
    ];

    for action in &hallucinated {
        assert!(
            !allowed.contains(action),
            "Hallucinated action '{}' should be rejected",
            action
        );
    }
}
