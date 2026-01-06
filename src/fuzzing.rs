//! Terminal Fuzzing Engine

#![allow(dead_code)]

use crate::determinism::SeededRng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum FuzzingStrategy {
    KeySequence(KeySequenceConfig),
    ResizeStorm(ResizeStormConfig),
    SignalInjection(SignalInjectionConfig),
    InputFlood(InputFloodConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeySequenceConfig {
    pub min_length: usize,
    pub max_length: usize,
    pub include_modifiers: bool,
    pub include_control: bool,
    pub include_unicode: bool,
    pub target_keys: Vec<String>,
}

impl Default for KeySequenceConfig {
    fn default() -> Self {
        Self {
            min_length: 1,
            max_length: 10,
            include_modifiers: true,
            include_control: true,
            include_unicode: false,
            target_keys: vec![
                "enter".to_string(),
                "tab".to_string(),
                "escape".to_string(),
                "backspace".to_string(),
                "arrow_up".to_string(),
                "arrow_down".to_string(),
                "ctrl_c".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResizeStormConfig {
    pub min_size: (u16, u16),
    pub max_size: (u16, u16),
    pub step_size: u16,
    pub burst_count: usize,
    pub include_extreme: bool,
}

impl Default for ResizeStormConfig {
    fn default() -> Self {
        Self {
            min_size: (40, 10),
            max_size: (200, 60),
            step_size: 10,
            burst_count: 5,
            include_extreme: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignalInjectionConfig {
    pub signals: Vec<String>,
    pub max_per_scenario: usize,
    pub allow_restart: bool,
    pub timing: SignalTiming,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SignalTiming {
    Random,
    AfterInput,
    BeforeExit,
    Midway,
}

impl Default for SignalInjectionConfig {
    fn default() -> Self {
        Self {
            signals: vec!["SIGINT".to_string(), "SIGTERM".to_string()],
            max_per_scenario: 3,
            allow_restart: false,
            timing: SignalTiming::Random,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputFloodConfig {
    pub min_events: usize,
    pub max_events: usize,
    pub event_types: Vec<String>,
    pub inter_event_delay: u64,
}

impl Default for InputFloodConfig {
    fn default() -> Self {
        Self {
            min_events: 10,
            max_events: 100,
            event_types: vec!["key".to_string(), "resize".to_string()],
            inter_event_delay: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FuzzingConfig {
    pub enabled: bool,
    pub strategies: Vec<FuzzingStrategy>,
    pub intensity: FuzzingIntensity,
    pub seed: Option<u64>,
    pub max_mutations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FuzzingIntensity {
    Low,
    Medium,
    High,
    Extreme,
}

impl Default for FuzzingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            strategies: vec![
                FuzzingStrategy::KeySequence(KeySequenceConfig::default()),
                FuzzingStrategy::ResizeStorm(ResizeStormConfig::default()),
            ],
            intensity: FuzzingIntensity::Medium,
            seed: None,
            max_mutations: 1000,
        }
    }
}

pub struct FuzzingEngine {
    config: FuzzingConfig,
    rng: SeededRng,
}

impl FuzzingEngine {
    pub fn new(config: FuzzingConfig) -> Self {
        let seed = config.seed.unwrap_or_else(|| fastrand::u64(..));
        Self {
            config,
            rng: SeededRng::new(seed),
        }
    }

    pub fn generate_key_sequence(&mut self, config: &KeySequenceConfig) -> Vec<FuzzKeyEvent> {
        let length = self
            .rng
            .usize_range(config.min_length, config.max_length + 1);
        let mut events = Vec::with_capacity(length);

        for _ in 0..length {
            let key = self.select_random_key(config);
            let modifiers = if config.include_modifiers {
                self.generate_modifiers()
            } else {
                vec![]
            };
            events.push(FuzzKeyEvent { key, modifiers });
        }

        events
    }

    fn select_random_key(&mut self, config: &KeySequenceConfig) -> String {
        let idx = self.rng.usize(config.target_keys.len());
        config.target_keys[idx].clone()
    }

    fn generate_modifiers(&mut self) -> Vec<String> {
        let modifiers = ["ctrl", "alt", "shift"];
        let mut selected = Vec::new();

        for m in &modifiers {
            if self.rng.next_bool(0.3) {
                selected.push((*m).to_string());
            }
        }

        selected
    }

    pub fn generate_resize_sequence(&mut self, config: &ResizeStormConfig) -> Vec<FuzzResizeEvent> {
        let mut events = Vec::with_capacity(config.burst_count);

        let widths: Vec<u16> = (config.min_size.0..=config.max_size.0)
            .step_by(config.step_size as usize)
            .chain(if config.include_extreme {
                Some(1)
            } else {
                None
            })
            .collect();

        let heights: Vec<u16> = (config.min_size.1..=config.max_size.1)
            .step_by(config.step_size as usize)
            .chain(if config.include_extreme {
                Some(1)
            } else {
                None
            })
            .collect();

        for _ in 0..config.burst_count {
            let cols = self.select_from_usize_slice(&widths);
            let rows = self.select_from_usize_slice(&heights);
            events.push(FuzzResizeEvent {
                cols: cols as u16,
                rows: rows as u16,
            });
        }

        events
    }

    fn select_from_usize_slice(&mut self, values: &[u16]) -> usize {
        let idx = self.rng.usize(values.len());
        values[idx] as usize
    }

    pub fn generate_signal_sequence(
        &mut self,
        config: &SignalInjectionConfig,
    ) -> Vec<FuzzSignalEvent> {
        let count = self.rng.usize_range(1, config.max_per_scenario.min(10) + 1);
        let mut events = Vec::with_capacity(count);

        for _ in 0..count {
            let sig_idx = self.rng.usize(config.signals.len());
            let tick = match config.timing {
                SignalTiming::Random => self.rng.next_u64() % 1000,
                SignalTiming::AfterInput => 0,
                SignalTiming::BeforeExit => 900,
                SignalTiming::Midway => 500,
            };
            events.push(FuzzSignalEvent {
                signal: config.signals[sig_idx].clone(),
                tick,
            });
        }

        events
    }

    pub fn generate_input_flood(&mut self, config: &InputFloodConfig) -> Vec<FuzzInputEvent> {
        let count = self
            .rng
            .usize_range(config.min_events, config.max_events + 1);
        let mut events = Vec::with_capacity(count);

        for i in 0..count {
            let event_type_idx = self.rng.usize(config.event_types.len());
            let tick = i as u64 * (config.inter_event_delay + 1);

            let event = match config.event_types[event_type_idx].as_str() {
                "key" => FuzzInputEvent::Key(FuzzKeyEvent {
                    key: "a".to_string(),
                    modifiers: vec![],
                }),
                "resize" => FuzzInputEvent::Resize(FuzzResizeEvent { cols: 80, rows: 24 }),
                "signal" => FuzzInputEvent::Signal(FuzzSignalEvent {
                    signal: "SIGINT".to_string(),
                    tick,
                }),
                _ => FuzzInputEvent::Key(FuzzKeyEvent {
                    key: "enter".to_string(),
                    modifiers: vec![],
                }),
            };

            events.push(event);
        }

        events
    }

    pub fn run_fuzzing_strategy(&mut self, strategy: &FuzzingStrategy) -> Vec<FuzzInputEvent> {
        match strategy {
            FuzzingStrategy::KeySequence(config) => {
                let keys = self.generate_key_sequence(config);
                keys.into_iter().map(FuzzInputEvent::Key).collect()
            }
            FuzzingStrategy::ResizeStorm(config) => {
                let resizes = self.generate_resize_sequence(config);
                resizes.into_iter().map(FuzzInputEvent::Resize).collect()
            }
            FuzzingStrategy::SignalInjection(config) => {
                let signals = self.generate_signal_sequence(config);
                signals.into_iter().map(FuzzInputEvent::Signal).collect()
            }
            FuzzingStrategy::InputFlood(config) => self.generate_input_flood(config),
        }
    }

    pub fn generate_mutation_sequence(&self, base: &[FuzzInputEvent]) -> Vec<FuzzInputEvent> {
        let mut mutated = base.to_vec();
        let mut rng = SeededRng::new(self.rng.state());
        let mutations = rng.usize_range(0, 6);

        for _ in 0..mutations {
            let op = rng.usize(4);
            match op {
                0 => {
                    mutated.push(FuzzInputEvent::Key(FuzzKeyEvent {
                        key: "x".to_string(),
                        modifiers: vec![],
                    }));
                }
                1 if !mutated.is_empty() => {
                    let idx = rng.usize(mutated.len());
                    mutated.remove(idx);
                }
                2 if mutated.len() >= 2 => {
                    let idx = rng.usize(mutated.len() - 1);
                    mutated.swap(idx, idx + 1);
                }
                3 => {
                    let idx = rng.usize(mutated.len());
                    mutated[idx] = FuzzInputEvent::Key(FuzzKeyEvent {
                        key: "y".to_string(),
                        modifiers: vec![],
                    });
                }
                _ => {}
            }
        }

        mutated
    }

    pub fn seed(&self) -> u64 {
        self.rng.state()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FuzzInputEvent {
    Key(FuzzKeyEvent),
    Resize(FuzzResizeEvent),
    Signal(FuzzSignalEvent),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FuzzKeyEvent {
    pub key: String,
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FuzzResizeEvent {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FuzzSignalEvent {
    pub signal: String,
    pub tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzing_config_default() {
        let config = FuzzingConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.strategies.len(), 2);
    }

    #[test]
    fn key_sequence_config_default() {
        let config = KeySequenceConfig::default();
        assert_eq!(config.min_length, 1);
        assert_eq!(config.max_length, 10);
        assert!(config.include_modifiers);
    }

    #[test]
    fn resize_storm_config_default() {
        let config = ResizeStormConfig::default();
        assert_eq!(config.min_size, (40, 10));
        assert_eq!(config.max_size, (200, 60));
        assert_eq!(config.burst_count, 5);
    }

    #[test]
    fn signal_injection_config_default() {
        let config = SignalInjectionConfig::default();
        assert!(config.signals.contains(&"SIGINT".to_string()));
        assert_eq!(config.max_per_scenario, 3);
    }

    #[test]
    fn input_flood_config_default() {
        let config = InputFloodConfig::default();
        assert_eq!(config.min_events, 10);
        assert_eq!(config.max_events, 100);
    }

    #[test]
    fn fuzzing_engine_new() {
        let config = FuzzingConfig::default();
        let engine = FuzzingEngine::new(config);
        assert!(engine.seed() > 0);
    }

    #[test]
    fn generate_key_sequence() {
        let config = KeySequenceConfig {
            min_length: 2,
            max_length: 4,
            include_modifiers: true,
            include_control: true,
            include_unicode: false,
            target_keys: vec!["enter".to_string(), "tab".to_string()],
        };
        let config_with_seed = FuzzingConfig {
            enabled: true,
            strategies: vec![FuzzingStrategy::KeySequence(config.clone())],
            intensity: FuzzingIntensity::Medium,
            seed: Some(42),
            max_mutations: 100,
        };
        let mut engine = FuzzingEngine::new(config_with_seed);
        let sequence = engine.generate_key_sequence(&config);

        assert!(sequence.len() >= 2 && sequence.len() <= 4);
        for event in &sequence {
            assert!(config.target_keys.contains(&event.key));
        }
    }

    #[test]
    fn generate_resize_sequence() {
        let config = ResizeStormConfig::default();
        let config_with_seed = FuzzingConfig {
            enabled: true,
            strategies: vec![FuzzingStrategy::ResizeStorm(config.clone())],
            intensity: FuzzingIntensity::Medium,
            seed: Some(42),
            max_mutations: 100,
        };
        let mut engine = FuzzingEngine::new(config_with_seed);
        let sequence = engine.generate_resize_sequence(&config);

        assert_eq!(sequence.len(), config.burst_count);
        for resize in &sequence {
            assert!(resize.cols >= 1 && resize.cols <= config.max_size.0);
            assert!(resize.rows >= 1 && resize.rows <= config.max_size.1);
        }
    }

    #[test]
    fn generate_signal_sequence() {
        let config = SignalInjectionConfig::default();
        let config_with_seed = FuzzingConfig {
            enabled: true,
            strategies: vec![FuzzingStrategy::SignalInjection(config.clone())],
            intensity: FuzzingIntensity::Medium,
            seed: Some(42),
            max_mutations: 100,
        };
        let mut engine = FuzzingEngine::new(config_with_seed);
        let sequence = engine.generate_signal_sequence(&config);

        assert!(!sequence.is_empty());
        assert!(sequence.len() <= config.max_per_scenario);
        for signal in &sequence {
            assert!(config.signals.contains(&signal.signal));
        }
    }

    #[test]
    fn generate_input_flood() {
        let config = InputFloodConfig::default();
        let config_with_seed = FuzzingConfig {
            enabled: true,
            strategies: vec![FuzzingStrategy::InputFlood(config.clone())],
            intensity: FuzzingIntensity::Medium,
            seed: Some(42),
            max_mutations: 100,
        };
        let mut engine = FuzzingEngine::new(config_with_seed);
        let sequence = engine.generate_input_flood(&config);

        assert!(sequence.len() >= config.min_events && sequence.len() <= config.max_events);
    }

    #[test]
    fn run_fuzzing_strategy_key_sequence() {
        let config = KeySequenceConfig::default();
        let config_with_seed = FuzzingConfig {
            enabled: true,
            strategies: vec![FuzzingStrategy::KeySequence(config.clone())],
            intensity: FuzzingIntensity::Medium,
            seed: Some(42),
            max_mutations: 100,
        };
        let mut engine = FuzzingEngine::new(config_with_seed);
        let events = engine.run_fuzzing_strategy(&FuzzingStrategy::KeySequence(config));

        assert!(!events.is_empty());
        assert!(events.iter().all(|e| matches!(e, FuzzInputEvent::Key(_))));
    }

    #[test]
    fn run_fuzzing_strategy_resize_storm() {
        let config = ResizeStormConfig::default();
        let config_with_seed = FuzzingConfig {
            enabled: true,
            strategies: vec![FuzzingStrategy::ResizeStorm(config.clone())],
            intensity: FuzzingIntensity::Medium,
            seed: Some(42),
            max_mutations: 100,
        };
        let mut engine = FuzzingEngine::new(config_with_seed);
        let events = engine.run_fuzzing_strategy(&FuzzingStrategy::ResizeStorm(config));

        assert!(!events.is_empty());
        assert!(events
            .iter()
            .all(|e| matches!(e, FuzzInputEvent::Resize(_))));
    }

    #[test]
    fn generate_mutation_sequence() {
        let base = vec![
            FuzzInputEvent::Key(FuzzKeyEvent {
                key: "enter".to_string(),
                modifiers: vec![],
            }),
            FuzzInputEvent::Resize(FuzzResizeEvent { cols: 80, rows: 24 }),
        ];
        let config_with_seed = FuzzingConfig {
            enabled: true,
            strategies: vec![],
            intensity: FuzzingIntensity::Medium,
            seed: Some(42),
            max_mutations: 100,
        };
        let engine = FuzzingEngine::new(config_with_seed);
        let mutated = engine.generate_mutation_sequence(&base);

        assert!(mutated.len() >= 1);
    }

    #[test]
    fn fuzzing_intensity_variants() {
        assert_eq!(format!("{:?}", FuzzingIntensity::Low), "Low");
        assert_eq!(format!("{:?}", FuzzingIntensity::Medium), "Medium");
        assert_eq!(format!("{:?}", FuzzingIntensity::High), "High");
        assert_eq!(format!("{:?}", FuzzingIntensity::Extreme), "Extreme");
    }

    #[test]
    fn signal_timing_variants() {
        assert_eq!(format!("{:?}", SignalTiming::Random), "Random");
        assert_eq!(format!("{:?}", SignalTiming::AfterInput), "AfterInput");
        assert_eq!(format!("{:?}", SignalTiming::BeforeExit), "BeforeExit");
        assert_eq!(format!("{:?}", SignalTiming::Midway), "Midway");
    }
}
