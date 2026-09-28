//! Unit tests for MCP tools registry.

use sentry_mcp::tools::list_tools;

#[test]
fn test_list_tools_contains_all_required_tools() {
    let tools = list_tools();
    let arr = tools.as_array().expect("Tools must be an array");

    let tool_names: Vec<&str> = arr
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    assert!(tool_names.contains(&"get_incident"));
    assert!(tool_names.contains(&"list_incidents"));
    assert!(tool_names.contains(&"get_unit_telemetry"));
    assert!(tool_names.contains(&"explain_incident"));
    assert_eq!(tool_names.len(), 4);
}

#[test]
fn test_tool_schemas_are_valid_objects() {
    let tools = list_tools();
    for tool in tools.as_array().unwrap() {
        assert!(tool["name"].is_string());
        assert!(tool["description"].is_string());
        assert_eq!(tool["inputSchema"]["type"], "object");
    }
}
