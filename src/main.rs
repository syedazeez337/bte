//! Behavioral Testing Engine for CLI/TUI applications
//!
//! This crate provides deterministic behavioral testing for terminal applications.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod ansi;
mod determinism;
mod invariants;
mod io_loop;
mod keys;
mod process;
mod pty;
mod replay;
mod runner;
mod scenario;
mod screen;
mod termination;
mod timing;
mod trace;
mod vtparse;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(name = "bte")]
#[command(author = "BTE Team")]
#[command(version = VERSION)]
#[command(about = "Behavioral Testing Engine for CLI/TUI applications", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,

    #[arg(short, long)]
    seed: Option<u64>,

    #[arg(long, default_value = "10000")]
    max_ticks: u64,

    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    #[command(name = "run")]
    Run {
        #[arg(value_name = "FILE")]
        scenario: PathBuf,

        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
    },

    #[command(name = "replay")]
    Replay {
        #[arg(value_name = "FILE")]
        trace: PathBuf,

        #[arg(long)]
        halt_on_divergence: bool,
    },

    #[command(name = "validate")]
    Validate {
        #[arg(value_name = "FILE")]
        scenario: PathBuf,
    },

    #[command(name = "info")]
    Info {
        #[arg(value_name = "FILE")]
        trace: PathBuf,
    },
}

use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(code) => {
            if code >= 0 && code <= 255 {
                ExitCode::from(code as u8)
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("Error: {:?}", e);
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<i32> {
    let args = Args::parse();

    let config = runner::RunnerConfig {
        seed: args.seed,
        max_ticks: args.max_ticks,
        verbose: args.verbose,
        ..runner::RunnerConfig::default()
    };

    match args.command {
        Command::Run { scenario, output } => cmd_run(scenario, output, &config),
        Command::Replay {
            trace,
            halt_on_divergence,
        } => cmd_replay(trace, halt_on_divergence).map(|_| 0),
        Command::Validate { scenario } => cmd_validate(scenario).map(|_| 0),
        Command::Info { trace } => cmd_info(trace).map(|_| 0),
    }
}

fn cmd_run(
    scenario_path: PathBuf,
    output_path: Option<PathBuf>,
    config: &runner::RunnerConfig,
) -> Result<i32> {
    if config.verbose {
        eprintln!("Loading scenario: {}", scenario_path.display());
    }

    let scenario_content = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("Failed to read scenario: {}", scenario_path.display()))?;

    let scenario: scenario::Scenario = if scenario_path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        == Some("json".to_string())
    {
        serde_json::from_str(&scenario_content)
            .with_context(|| "Failed to parse scenario as JSON")?
    } else {
        serde_yaml::from_str(&scenario_content)
            .with_context(|| "Failed to parse scenario as YAML")?
    };

    if config.verbose {
        eprintln!("Scenario: {}", scenario.name);
        eprintln!("Steps: {}", scenario.steps.len());
        if let Some(seed) = config.seed.or(scenario.seed) {
            eprintln!("Seed: {}", seed);
        }
    }

    if let Err(errors) = scenario.validate() {
        let error_msg: String = errors
            .iter()
            .map(|e| format!("  - {}", e))
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::bail!("Scenario validation failed:\n{}", error_msg);
    }

    let seed = config
        .seed
        .or(scenario.seed)
        .unwrap_or_else(|| fastrand::u64(..));

    let config = runner::RunnerConfig {
        seed: Some(seed),
        trace_path: output_path.map(|p| p.to_string_lossy().to_string()),
        verbose: config.verbose,
        max_ticks: config.max_ticks,
        tick_delay_ms: config.tick_delay_ms,
    };

    if config.verbose {
        eprintln!("Running with seed: {}", seed);
    }

    let result = runner::run_scenario(&scenario, &config);

    println!("=== Run Result ===");
    println!("Exit code: {}", result.exit_code);
    println!("Steps executed: {}", result.trace.steps.len());
    println!("Ticks: {}", result.trace.total_ticks);

    match &result.trace.outcome {
        trace::TraceOutcome::Success {
            exit_code,
            total_ticks,
        } => {
            println!(
                "Status: SUCCESS (exit={}, ticks={})",
                exit_code, total_ticks
            );
        }
        trace::TraceOutcome::InvariantViolation {
            invariant_name,
            checkpoint_index,
        } => {
            println!("Status: INVARIANT VIOLATION");
            println!("Invariant: {}", invariant_name);
            println!("Checkpoint: {}", checkpoint_index);
        }
        trace::TraceOutcome::Timeout {
            max_ticks,
            elapsed_ticks,
        } => {
            println!("Status: TIMEOUT");
            println!("Max ticks: {}, Elapsed: {}", max_ticks, elapsed_ticks);
        }
        trace::TraceOutcome::Error {
            message,
            step_index,
        } => {
            println!("Status: ERROR");
            println!("Message: {}", message);
            println!("Step: {}", step_index);
        }
        trace::TraceOutcome::Signaled {
            signal,
            signal_name,
        } => {
            println!("Status: SIGNALED");
            println!("Signal: {} ({})", signal_name, signal);
        }
        trace::TraceOutcome::ReplayDivergence {
            expected,
            actual,
            context,
        } => {
            println!("Status: DIVERGENCE");
            println!("Expected: {}", expected);
            println!("Actual: {}", actual);
            println!("Context: {}", context);
        }
    }

    let violations: Vec<_> = result
        .trace
        .invariant_results
        .iter()
        .filter(|r| r.violation())
        .collect();

    if !violations.is_empty() {
        println!("\nInvariant Violations:");
        for v in &violations {
            println!("  - {}: {}", v.name, v.description);
            if let Some(details) = &v.details {
                println!("    Details: {}", details);
            }
        }
    }

    Ok(result.exit_code.max(-1))
}

fn cmd_replay(trace_path: PathBuf, halt_on_divergence: bool) -> Result<()> {
    if halt_on_divergence {
        eprintln!("Loading trace: {}", trace_path.display());
    }

    let trace = trace::load_trace(&trace_path)
        .with_context(|| format!("Failed to load trace: {}", trace_path.display()))?;

    if halt_on_divergence {
        eprintln!("Scenario: {}", trace.scenario.name);
        eprintln!("Seed: {}", trace.seed);
        eprintln!("Steps: {}", trace.steps.len());
    }

    let mut replay = trace::ReplayEngine::new(&trace);
    replay.set_halt_on_divergence(halt_on_divergence);

    println!("=== Replay Result ===");

    if replay.is_successful() {
        println!("Status: REPLAY SUCCESSFUL");
        println!("All checkpoints matched.");
        Ok(())
    } else {
        println!("Status: REPLAY DIVERGENCE DETECTED");
        println!("Divergences: {}", replay.divergences().len());

        for div in replay.divergences() {
            println!("\nDivergence:");
            println!("  Type: {:?}", div.kind);
            println!("  Expected: {}", div.expected);
            println!("  Actual: {}", div.actual);
            println!("  Context: {}", div.context);
            println!("  Step: {}", div.step_index);
            println!("  Tick: {}", div.tick);
        }

        Err(anyhow::anyhow!("Replay failed - divergences detected"))
    }
}

fn cmd_validate(scenario_path: PathBuf) -> Result<()> {
    println!("Validating scenario: {}", scenario_path.display());

    let content = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("Failed to read: {}", scenario_path.display()))?;

    let scenario: scenario::Scenario = if scenario_path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        == Some("json".to_string())
    {
        serde_json::from_str(&content).with_context(|| "Failed to parse scenario as JSON")?
    } else {
        serde_yaml::from_str(&content).with_context(|| "Failed to parse scenario as YAML")?
    };

    match scenario.validate() {
        Ok(()) => {
            println!("Scenario is valid.");
            println!("  Name: {}", scenario.name);
            println!("  Steps: {}", scenario.steps.len());
            println!("  Invariants: {}", scenario.invariants.len());
            if let Some(seed) = scenario.seed {
                println!("  Seed: {}", seed);
            }
            Ok(())
        }
        Err(errors) => {
            println!("Scenario validation FAILED:");
            for error in &errors {
                println!("  {}: {}", error.path, error.message);
            }
            Err(anyhow::anyhow!("Validation failed"))
        }
    }
}

fn cmd_info(trace_path: PathBuf) -> Result<()> {
    let trace = trace::load_trace(&trace_path)
        .with_context(|| format!("Failed to load trace: {}", trace_path.display()))?;

    trace::print_trace_summary(&trace);

    Ok(())
}
