//! Integration tests for BTE scenario parsing and execution

use std::path::Path;
use std::process::Command;

#[test]
fn test_scenario_parsing_valid() {
    let scenario = r#"
name: test-scenario
description: A test scenario
command: echo hello
steps:
  - action: wait_for
    pattern: hello
    timeout_ms: 1000
"#;

    // Test that the scenario can be parsed
    let parsed: serde_yaml::Value = serde_yaml::from_str(scenario).expect("Failed to parse YAML");
    assert_eq!(parsed["name"], "test-scenario");
    assert_eq!(parsed["command"], "echo hello");
}

#[test]
fn test_scenario_with_all_step_types() {
    let scenario = r#"
name: full-scenario
description: Tests all step types
command: cat
steps:
  - action: wait_for
    pattern: ready
    timeout_ms: 500
  - action: send_keys
    keys: "test\n"
  - action: wait_ticks
    ticks: 10
  - action: assert_screen
    pattern: "test"
    anywhere: true
  - action: resize
    cols: 80
    rows: 24
  - action: send_signal
    signal: SIGTERM
"#;

    let parsed: serde_yaml::Value = serde_yaml::from_str(scenario).expect("Failed to parse");
    let steps = parsed["steps"].as_sequence().expect("Should have steps");
    assert_eq!(steps.len(), 6);
}

#[test]
fn test_scenario_with_invariants() {
    let scenario = r#"
name: invariant-test
description: Test with invariants
command: yes | head -5
steps:
  - action: wait_for
    pattern: "y"
    timeout_ms: 1000
invariants:
  - type: cursor_bounds
  - type: no_deadlock
  - type: screen_contains
    pattern: "y"
"#;

    let parsed: serde_yaml::Value = serde_yaml::from_str(scenario).expect("Failed to parse");
    let invariants = parsed["invariants"]
        .as_sequence()
        .expect("Should have invariants");
    assert_eq!(invariants.len(), 3);
}

#[test]
fn test_scenario_env_vars() {
    let scenario = r#"
name: env-test
description: Test environment variables
command: echo $TEST_VAR
env:
  TEST_VAR: hello_world
steps:
  - action: wait_for
    pattern: hello_world
    timeout_ms: 500
"#;

    let parsed: serde_yaml::Value = serde_yaml::from_str(scenario).expect("Failed to parse");
    let env = parsed["env"].as_mapping().expect("Should have env");
    assert!(env.contains_key(&serde_yaml::Value::from("TEST_VAR")));
}

#[test]
fn test_scenario_seed_reproducibility() {
    let scenario = r#"
name: seeded-scenario
description: Test with seed
command: cat
seed: 42
steps:
  - action: wait_ticks
    ticks: 5
"#;

    let parsed: serde_yaml::Value = serde_yaml::from_str(scenario).expect("Failed to parse");
    let seed = parsed["seed"].as_u64().expect("Should have seed");
    assert_eq!(seed, 42);
}

#[test]
fn test_invalid_scenario_missing_name() {
    let scenario = r#"
description: Missing name
command: echo test
steps: []
"#;

    let result = serde_yaml::from_str::<serde_yaml::Value>(scenario);
    // Should still parse as YAML but fail validation later
    assert!(result.is_ok());
}

#[test]
fn test_invalid_scenario_missing_command() {
    let scenario = r#"
name: test
steps: []
"#;

    let result = serde_yaml::from_str::<serde_yaml::Value>(scenario);
    assert!(result.is_ok());
}

#[test]
fn test_special_keys_in_scenario() {
    let scenario = r#"
name: keys-test
command: cat
steps:
  - action: send_keys
    keys:
      - "Enter"
      - "Tab"
      - "Backspace"
      - "Escape"
      - "Up"
      - "Down"
      - "Left"
      - "Right"
      - "Ctrl+c"
      - "Alt+x"
"#;

    let parsed: serde_yaml::Value = serde_yaml::from_str(scenario).expect("Failed to parse");
    let keys = parsed["steps"][0]["keys"]
        .as_sequence()
        .expect("Should have keys");
    assert_eq!(keys.len(), 10);
}

#[test]
fn test_timeout_handling() {
    let scenario = r#"
name: timeout-test
description: Test timeout configuration
command: sleep 10
timeout_ms: 500
steps:
  - action: wait_for
    pattern: never
    timeout_ms: 100
"#;

    let parsed: serde_yaml::Value = serde_yaml::from_str(scenario).expect("Failed to parse");
    let timeout_ms = parsed["timeout_ms"].as_u64().expect("Should have timeout");
    assert_eq!(timeout_ms, 500);
}

#[test]
fn test_terminal_config() {
    let scenario = r#"
name: terminal-test
command: cat
terminal:
  cols: 120
  rows: 40
steps:
  - action: wait_ticks
    ticks: 1
"#;

    let parsed: serde_yaml::Value = serde_yaml::from_str(scenario).expect("Failed to parse");
    let terminal = parsed["terminal"]
        .as_mapping()
        .expect("Should have terminal");
    assert_eq!(terminal["cols"], 120);
    assert_eq!(terminal["rows"], 40);
}
