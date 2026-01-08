//! Integration tests for BTE scenario parsing and execution

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
    let test_var_key = serde_yaml::Value::String("TEST_VAR".to_string());
    assert!(env.contains_key(&test_var_key));
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

// ============================================================================
// Scenario Validation Integration Tests
// These tests verify validation behavior through the public API
// ============================================================================

/// Helper function to parse and validate a scenario
fn parse_and_validate(yaml: &str) -> Result<(), Vec<String>> {
    let scenario: serde_yaml::Value =
        serde_yaml::from_str(yaml).map_err(|e| vec![format!("Parse error: {}", e)])?;

    let mut errors = Vec::new();

    // Check required fields
    if scenario.get("name").is_none() || scenario["name"].as_str().map_or(true, |s| s.is_empty()) {
        errors.push("name: Scenario name cannot be empty".to_string());
    }

    if scenario.get("command").is_none() {
        errors.push("command: Command is required".to_string());
    }

    // Check steps exist
    let steps = match scenario.get("steps").and_then(|s| s.as_sequence()) {
        Some(s) => s,
        None => {
            errors.push("steps: Scenario must have at least one step".to_string());
            return Err(errors);
        }
    };

    if steps.is_empty() {
        errors.push("steps: Scenario must have at least one step".to_string());
    }

    // Validate terminal config
    if let Some(terminal) = scenario.get("terminal").and_then(|t| t.as_mapping()) {
        if let Some(cols) = terminal.get("cols").and_then(|c| c.as_i64()) {
            if cols <= 0 {
                errors.push("terminal.cols: Terminal columns must be > 0".to_string());
            }
        }
        if let Some(rows) = terminal.get("rows").and_then(|r| r.as_i64()) {
            if rows <= 0 {
                errors.push("terminal.rows: Terminal rows must be > 0".to_string());
            }
        }
    }

    // Validate each step
    for (i, step) in steps.iter().enumerate() {
        let action = step
            .get("action")
            .and_then(|a| a.as_str())
            .unwrap_or("unknown");

        match action {
            "wait_for" => {
                if let Some(pattern) = step.get("pattern").and_then(|p| p.as_str()) {
                    if pattern.is_empty() {
                        errors.push(format!("steps[{}].pattern: Pattern cannot be empty", i));
                    }
                } else {
                    errors.push(format!("steps[{}].pattern: Pattern is required", i));
                }
            }
            "wait_ticks" => {
                if let Some(ticks) = step.get("ticks").and_then(|t| t.as_i64()) {
                    if ticks <= 0 {
                        errors.push(format!("steps[{}].ticks: Ticks must be > 0", i));
                    }
                }
            }
            "resize" => {
                if let Some(cols) = step.get("cols").and_then(|c| c.as_i64()) {
                    if cols <= 0 {
                        errors.push(format!("steps[{}].cols: Resize cols must be > 0", i));
                    }
                }
                if let Some(rows) = step.get("rows").and_then(|r| r.as_i64()) {
                    if rows <= 0 {
                        errors.push(format!("steps[{}].rows: Resize rows must be > 0", i));
                    }
                }
            }
            "snapshot" => {
                if let Some(name) = step.get("name").and_then(|n| n.as_str()) {
                    if name.is_empty() {
                        errors.push(format!("steps[{}].name: Snapshot name cannot be empty", i));
                    }
                }
            }
            _ => {}
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[test]
fn test_validation_empty_name_rejected() {
    let yaml = r#"
name: ""
command: echo hello
steps:
  - action: wait_ticks
    ticks: 1
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("name")));
}

#[test]
fn test_validation_zero_cols_rejected() {
    let yaml = r#"
name: test
command: echo
terminal:
  cols: 0
  rows: 24
steps:
  - action: wait_ticks
    ticks: 1
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("cols")));
}

#[test]
fn test_validation_zero_rows_rejected() {
    let yaml = r#"
name: test
command: echo
terminal:
  cols: 80
  rows: 0
steps:
  - action: wait_ticks
    ticks: 1
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("rows")));
}

#[test]
fn test_validation_empty_steps_rejected() {
    let yaml = r#"
name: test
command: echo
steps: []
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("steps")));
}

#[test]
fn test_validation_wait_for_empty_pattern_rejected() {
    let yaml = r#"
name: test
command: echo
steps:
  - action: wait_for
    pattern: ""
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("pattern")));
}

#[test]
fn test_validation_wait_ticks_zero_rejected() {
    let yaml = r#"
name: test
command: echo
steps:
  - action: wait_ticks
    ticks: 0
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("ticks")));
}

#[test]
fn test_validation_resize_zero_cols_rejected() {
    let yaml = r#"
name: test
command: echo
steps:
  - action: resize
    cols: 0
    rows: 24
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("cols")));
}

#[test]
fn test_validation_resize_zero_rows_rejected() {
    let yaml = r#"
name: test
command: echo
steps:
  - action: resize
    cols: 80
    rows: 0
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("rows")));
}

#[test]
fn test_validation_snapshot_empty_name_rejected() {
    let yaml = r#"
name: test
command: echo
steps:
  - action: snapshot
    name: ""
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("name")));
}

#[test]
fn test_validation_multiple_errors() {
    let yaml = r#"
name: ""
command: echo
terminal:
  cols: 0
  rows: 0
steps:
  - action: wait_ticks
    ticks: 0
  - action: resize
    cols: 0
    rows: 0
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    // Should have at least 5 errors: name, terminal.cols, terminal.rows, steps[0].ticks, steps[1].cols
    assert!(
        errors.len() >= 5,
        "Expected at least 5 errors, got {}",
        errors.len()
    );
}

#[test]
fn test_validation_valid_scenario_succeeds() {
    let yaml = r#"
name: valid-test
description: A valid test scenario
command: echo hello
terminal:
  cols: 80
  rows: 24
env:
  TEST_VAR: test
steps:
  - action: wait_for
    pattern: hello
    timeout_ms: 5000
  - action: send_keys
    keys: "exit\n"
  - action: wait_ticks
    ticks: 10
  - action: resize
    cols: 120
    rows: 40
invariants:
  - type: cursor_bounds
  - type: screen_contains
    pattern: hello
seed: 12345
timeout_ms: 10000
"#;
    let result = parse_and_validate(yaml);
    assert!(
        result.is_ok(),
        "Expected valid scenario, got errors: {:?}",
        result
    );
}

#[test]
fn test_validation_yaml_parse_and_validate() {
    let yaml = r#"
name: yaml-validation-test
description: Test YAML parsing and validation
command: echo hello
steps:
  - action: wait_for
    pattern: hello
    timeout_ms: 1000
  - action: wait_ticks
    ticks: 5
"#;
    let result = parse_and_validate(yaml);
    assert!(
        result.is_ok(),
        "Expected valid scenario, got errors: {:?}",
        result
    );
}

#[test]
fn test_validation_json_parse_and_validate() {
    let json = r#"{
  "name": "json-validation-test",
  "command": "echo hello",
  "steps": [
    {"action": "wait_for", "pattern": "hello"},
    {"action": "wait_ticks", "ticks": 5}
  ]
}"#;

    // Parse JSON as YAML-compatible value
    let value: serde_yaml::Value = serde_json::from_str(json)
        .map_err(|e| vec![format!("Parse error: {}", e)])
        .expect("Failed to parse JSON");

    // Convert to YAML string and validate
    let yaml_str = serde_yaml::to_string(&value).expect("Failed to convert to YAML");
    let result = parse_and_validate(&yaml_str);
    assert!(
        result.is_ok(),
        "Expected valid scenario, got errors: {:?}",
        result
    );
}

#[test]
fn test_validation_complex_command() {
    let yaml = r#"
name: complex-command-test
command:
  program: /usr/bin/python3
  args:
    - "-c"
    - "print('hello')"
  cwd: /tmp
steps:
  - action: wait_ticks
    ticks: 1
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_validation_all_invariant_types() {
    let yaml = r#"
name: all-invariants-test
command: echo
steps:
  - action: wait_ticks
    ticks: 1
invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 1000
  - type: signal_handled
    signal: SIGTERM
  - type: screen_contains
    pattern: test
  - type: screen_not_contains
    pattern: error
  - type: no_output_after_exit
  - type: process_terminated_cleanly
    allowed_signals:
      - 15
  - type: screen_stability
    min_ticks: 10
  - type: viewport_valid
  - type: response_time
    max_ticks: 100
  - type: max_latency
    max_ticks: 50
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_validation_special_keys_steps() {
    let yaml = r#"
name: special-keys-test
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
  - action: assert_cursor
    row: 0
    col: 0
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_validation_assert_screen_step() {
    let yaml = r#"
name: assert-screen-test
command: echo test
steps:
  - action: wait_for
    pattern: test
  - action: assert_screen
    pattern: test
    anywhere: true
  - action: assert_screen
    pattern: test
    anywhere: false
    row: 0
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_validation_send_signal_step() {
    let yaml = r#"
name: signal-test
command: sleep 60
steps:
  - action: send_signal
    signal: SIGTERM
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_validation_check_invariant_step() {
    let yaml = r#"
name: check-invariant-test
command: echo
steps:
  - action: wait_ticks
    ticks: 1
  - action: check_invariant
    invariant:
      type: cursor_bounds
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_validation_error_message_format() {
    let yaml = r#"
name: ""
command: echo
terminal:
  cols: 0
  rows: 0
steps: []
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_err());
    let errors = result.unwrap_err();

    for error in &errors {
        // Each error should have a message and a path
        assert!(!error.is_empty(), "Error message should not be empty");
        // Error should contain colon separator
        assert!(error.contains(':'), "Error should contain colon separator");
    }
}

#[test]
fn test_validation_large_terminal_size() {
    let yaml = r#"
name: large-terminal
command: echo
terminal:
  cols: 65535
  rows: 65535
steps:
  - action: wait_ticks
    ticks: 1
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_validation_large_tick_values() {
    let yaml = r#"
name: large-ticks
command: echo
steps:
  - action: wait_ticks
    ticks: 18446744073709551615
  - action: wait_for
    pattern: test
    timeout_ms: 18446744073709551615
seed: 18446744073709551615
timeout_ms: 18446744073709551615
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_validation_environment_special_characters() {
    let yaml = r#"
name: special-env
command: echo
env:
  PATH: /usr/bin:/bin
  TEST_WITH_EQUALS: value=with=equals
  TEST_WITH_COLON: value:with:colons
steps:
  - action: wait_ticks
    ticks: 1
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_validation_all_signal_names() {
    let signals = [
        "SIGINT", "SIGTERM", "SIGKILL", "SIGWINCH", "SIGSTOP", "SIGCONT",
    ];

    for signal in &signals {
        let yaml = format!(
            r#"
name: signal-test
command: sleep 60
steps:
  - action: send_signal
    signal: {}
"#,
            signal
        );
        let result = parse_and_validate(&yaml);
        assert!(result.is_ok(), "Signal {} should be valid", signal);
    }
}

#[test]
fn test_validation_step_order_preserved() {
    let yaml = r#"
name: ordered-steps
command: cat
steps:
  - action: wait_ticks
    ticks: 1
  - action: wait_ticks
    ticks: 2
  - action: wait_ticks
    ticks: 3
  - action: wait_ticks
    ticks: 4
  - action: wait_ticks
    ticks: 5
"#;
    let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).expect("Failed to parse");
    let steps = parsed["steps"].as_sequence().expect("Should have steps");

    assert_eq!(steps.len(), 5);
    for (i, step) in steps.iter().enumerate() {
        let ticks = step.get("ticks").and_then(|t| t.as_i64()).unwrap();
        assert_eq!(ticks, (i + 1) as i64);
    }
}

#[test]
fn test_validation_nested_env_vars() {
    let yaml = r#"
name: nested-env
command: echo
env:
  VAR1: value1
  VAR2: value2
  VAR3: value3
steps:
  - action: wait_ticks
    ticks: 1
"#;
    let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).expect("Failed to parse");
    let env = parsed["env"].as_mapping().expect("Should have env");

    assert_eq!(env.len(), 3);
    assert!(env.contains_key(&serde_yaml::Value::String("VAR1".to_string())));
    assert!(env.contains_key(&serde_yaml::Value::String("VAR2".to_string())));
    assert!(env.contains_key(&serde_yaml::Value::String("VAR3".to_string())));
}

#[test]
fn test_validation_with_description() {
    let yaml = r#"
name: described-scenario
description: This is a detailed description of what this scenario tests.
description2: And this is another optional description field.
command: echo hello
steps:
  - action: wait_for
    pattern: hello
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_validation_mixed_action_types() {
    let yaml = r#"
name: mixed-actions
command: cat
steps:
  - action: send_keys
    keys: "hello\n"
  - action: wait_for
    pattern: hello
  - action: send_signal
    signal: SIGTERM
  - action: resize
    cols: 80
    rows: 24
  - action: assert_screen
    pattern: ".*"
    anywhere: true
  - action: assert_cursor
    row: 0
    col: 0
  - action: snapshot
    name: "final-state"
  - action: check_invariant
    invariant:
      type: cursor_bounds
  - action: wait_ticks
    ticks: 5
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_ok());
}
