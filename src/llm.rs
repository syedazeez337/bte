use std::process::{Command, Stdio};

pub struct LlmAssistant {
    model: String,
    api_endpoint: Option<String>,
    api_key: Option<String>,
}

impl LlmAssistant {
    pub fn new(model: String) -> Self {
        Self {
            model,
            api_endpoint: None,
            api_key: None,
        }
    }

    pub fn with_api_endpoint(mut self, endpoint: String) -> Self {
        self.api_endpoint = Some(endpoint);
        self
    }

    pub fn with_api_key(mut self, key: String) -> Self {
        self.api_key = Some(key);
        self
    }

    pub fn generate_scenario(&self, description: &str) -> Option<String> {
        let prompt = format!(
            "Generate a BTE test scenario YAML for the following application description:\n\n{}\n\n\
            The scenario should include:\n\
            - Clear test steps with key injections\n\
            - Expected screen patterns for verification\n\
            - Appropriate invariants for the TUI framework being tested",
            description
        );

        self.call_llm(&prompt)
    }

    pub fn analyze_failure(&self, scenario: &str, output: &str) -> Option<String> {
        let prompt = format!(
            "Analyze this BTE test failure:\n\nScenario:\n{}\n\nOutput:\n{}\n\n\
            Provide:\n\
            1. Root cause analysis\n\
            2. Suggested fixes to the scenario or application",
            scenario, output
        );

        self.call_llm(&prompt)
    }

    pub fn suggest_invariants(&self, app_type: &str) -> Vec<String> {
        let prompt = format!(
            "Suggest BTE invariants for testing a {} application. \
            Return only invariant names, one per line.",
            app_type
        );

        self.call_llm(&prompt)
            .map(|output| {
                output
                    .lines()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn call_llm(&self, prompt: &str) -> Option<String> {
        self.call_cli(prompt)
    }

    fn call_cli(&self, _prompt: &str) -> Option<String> {
        let output = Command::new("ollama")
            .args(&["run", &self.model])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .output()
            .ok()?;

        String::from_utf8(output.stdout).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_assistant_creation() {
        let assistant = LlmAssistant::new("llama3".to_string());
        assert_eq!(assistant.model, "llama3");
    }

    #[test]
    fn test_llm_assistant_with_options() {
        let assistant = LlmAssistant::new("llama3".to_string())
            .with_api_endpoint("http://localhost:11434/v1".to_string())
            .with_api_key("test-key".to_string());
        assert!(assistant.api_endpoint.is_some());
        assert!(assistant.api_key.is_some());
    }
}
