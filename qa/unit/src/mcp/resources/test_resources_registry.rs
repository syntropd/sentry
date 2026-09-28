//! Unit tests for MCP resources catalog.

use sentry_mcp::resources::list_resources;

#[test]
fn test_list_resources_contains_required_uris() {
    let resources = list_resources();
    let arr = resources.as_array().expect("Resources is array");

    let uris: Vec<&str> = arr.iter().filter_map(|r| r["uri"].as_str()).collect();

    assert!(uris.contains(&"incident://{incident_id}"));
    assert!(uris.contains(&"telemetry://{unit_name}"));
    assert!(uris.contains(&"policy://current"));
    assert!(uris.contains(&"circuit://status"));
    assert_eq!(uris.len(), 4);
}
