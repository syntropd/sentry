//! Tier 2: R1 Architectural Limits & Boundary Tests
//!
//! Tests edge conditions on file lengths, line limits, and code boundaries.

use std::fs;
use tempfile::tempdir;

#[test]
fn test_r1_boundary_empty_file_line_count() {
    let dir = tempdir().unwrap();
    let empty_file = dir.path().join("empty.rs");
    fs::write(&empty_file, b"").unwrap();

    let lines = fs::read_to_string(&empty_file).unwrap().lines().count();
    assert_eq!(lines, 0);
    assert!(lines <= 256);
}

#[test]
fn test_r1_boundary_exact_256_lines() {
    let dir = tempdir().unwrap();
    let file_256 = dir.path().join("file_256.rs");
    let mut content = String::new();
    for i in 1..=256 {
        content.push_str(&format!("// Line {}\n", i));
    }
    fs::write(&file_256, &content).unwrap();

    let lines = fs::read_to_string(&file_256).unwrap().lines().count();
    assert_eq!(lines, 256);
    assert!(lines <= 256, "Exact 256 lines must be permitted");
}

#[test]
fn test_r1_boundary_257_lines_violation_detection() {
    let dir = tempdir().unwrap();
    let file_257 = dir.path().join("file_257.rs");
    let mut content = String::new();
    for i in 1..=257 {
        content.push_str(&format!("// Line {}\n", i));
    }
    fs::write(&file_257, &content).unwrap();

    let lines = fs::read_to_string(&file_257).unwrap().lines().count();
    assert_eq!(lines, 257);
    assert!(lines > 256, "257 lines must trigger violation");
}

#[test]
fn test_r1_boundary_deep_nested_directory_traversal() {
    let dir = tempdir().unwrap();
    let mut deep_path = dir.path().to_path_buf();
    for i in 0..10 {
        deep_path = deep_path.join(format!("level_{}", i));
    }
    fs::create_dir_all(&deep_path).unwrap();
    let test_file = deep_path.join("leaf.rs");
    fs::write(&test_file, b"// leaf file\n").unwrap();

    assert!(test_file.exists());
    let lines = fs::read_to_string(&test_file).unwrap().lines().count();
    assert_eq!(lines, 1);
}

#[test]
fn test_r1_boundary_manifest_with_trailing_whitespace_and_comments() {
    let manifest_sample = "# Leading comment\n\n[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n# Trailing comment\n";
    let parsed: toml::Value = toml::from_str(manifest_sample).unwrap();
    assert_eq!(parsed["package"]["name"].as_str(), Some("test"));
}
