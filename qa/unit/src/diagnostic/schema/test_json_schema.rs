//! Unit tests for DiagnosticPayload JSON schema generator.

use sentry_diagnostic::schema::{diagnostic_payload_json_schema, openai_response_format};

#[test]
fn test_diagnostic_payload_json_schema_structure() {
    let schema = diagnostic_payload_json_schema();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["incident_id"].is_object());
    assert!(schema["properties"]["timestamp"].is_object());
    assert!(schema["properties"]["unit_name"].is_object());
    assert!(schema["properties"]["root_cause"].is_object());
    assert!(schema["properties"]["evidence"].is_object());
    assert!(schema["properties"]["severity"].is_object());
    assert!(schema["properties"]["proposed_remediation"].is_object());

    let required = schema["required"].as_array().expect("required array");
    assert!(required.iter().any(|v| v == "incident_id"));
    assert!(required.iter().any(|v| v == "proposed_remediation"));
}

#[test]
fn test_openai_response_format_structure() {
    let format = openai_response_format();
    assert_eq!(format["type"], "json_schema");
    assert_eq!(format["json_schema"]["name"], "systemd_incident_diagnostic");
    assert_eq!(format["json_schema"]["strict"], true);
    assert!(format["json_schema"]["schema"].is_object());
}
