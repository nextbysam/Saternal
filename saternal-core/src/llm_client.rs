/// LLM client for natural language to command translation
/// Uses Anannas AI with Claude for command generation

use anyhow::{Context, Result};
use lru::LruCache;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

const ANANNAS_BASE_URL: &str = "https://api.anannas.ai/v1/chat/completions";
const DEFAULT_MODEL: &str = "openai/gpt-4o-mini";
const DEFAULT_TIMEOUT: u64 = 10;
const CACHE_SIZE: usize = 100;

/// LLM client for command generation
pub struct LLMClient {
    client: reqwest::Client,
    api_key: String,
    model: String,
    cache: Arc<Mutex<LruCache<String, Vec<String>>>>,
}

/// Context information for command generation
#[derive(Debug, Clone)]
pub struct CommandContext {
    pub shell: String,
    pub current_dir: String,
    pub os: String,
}

#[derive(Debug, Serialize)]
struct LLMRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct LLMResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

impl LLMClient {
    /// Create a new LLM client with Anannas AI
    pub fn new(api_key: String) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT))
            .pool_max_idle_per_host(2)
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        let cache = Arc::new(Mutex::new(
            LruCache::new(NonZeroUsize::new(CACHE_SIZE).unwrap())
        ));

        Ok(Self {
            client,
            api_key,
            model: DEFAULT_MODEL.to_string(),
            cache,
        })
    }

    /// Set a custom model (default: anthropic/claude-3.5-sonnet)
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }

    /// Generate shell command from natural language
    pub async fn generate_command(
        &self,
        natural_language: &str,
        context: &CommandContext,
    ) -> Result<Vec<String>> {
        // Check cache first
        let cache_key = format!("{}:{}:{}", context.shell, context.os, natural_language);
        
        if let Some(cached) = self.cache.lock().get(&cache_key) {
            log::info!("Cache hit for NL command");
            return Ok(cached.clone());
        }

        // Build prompt with context
        let prompt = Self::build_prompt(natural_language, context);
        
        // Query LLM
        let response = self.query_llm(&prompt).await?;
        
        // Parse commands
        let commands = Self::parse_commands(&response)?;
        
        // Cache result
        self.cache.lock().put(cache_key, commands.clone());
        
        Ok(commands)
    }

    /// Build prompt with context
    fn build_prompt(nl: &str, context: &CommandContext) -> String {
        format!(
            r#"You are a shell command generator. Convert natural language requests into executable shell commands.

CONTEXT:
- Shell: {}
- Current Directory: {}
- OS: {}

USER REQUEST:
{}

INSTRUCTIONS:
1. Generate ONLY valid shell commands for the user's shell
2. Output one command per line
3. No explanations, no markdown, no comments
4. If multiple steps needed, output them in order
5. Prefer safe, non-destructive commands when possible
6. Use standard Unix/shell tools

COMMANDS:"#,
            context.shell,
            context.current_dir,
            context.os,
            nl
        )
    }

    /// Query the LLM API
    async fn query_llm(&self, prompt: &str) -> Result<String> {
        let request = LLMRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: 512,
            temperature: 0.2, // Low temperature for deterministic commands
        };

        let response = self
            .client
            .post(ANANNAS_BASE_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Anannas AI")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Anannas AI request failed with status {}: {}", status, body);
        }

        let llm_response: LLMResponse = response
            .json()
            .await
            .context("Failed to parse Anannas AI response")?;

        if llm_response.choices.is_empty() {
            anyhow::bail!("Anannas AI returned no choices");
        }

        Ok(llm_response.choices[0].message.content.clone())
    }

    /// Parse commands from LLM response
    fn parse_commands(response: &str) -> Result<Vec<String>> {
        let commands: Vec<String> = response
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .filter(|line| !line.starts_with('#'))      // Filter comments
            .filter(|line| !line.starts_with("```"))    // Filter markdown
            .filter(|line| !line.starts_with("COMMANDS:")) // Filter our prompt echo
            .map(|line| {
                // Remove leading numbers like "1. " or "1) "
                if let Some(stripped) = line.strip_prefix(|c: char| c.is_ascii_digit()) {
                    if let Some(stripped) = stripped.strip_prefix(". ").or_else(|| stripped.strip_prefix(") ")) {
                        return stripped.to_string();
                    }
                }
                line.to_string()
            })
            .collect();

        if commands.is_empty() {
            anyhow::bail!("LLM returned no valid commands");
        }

        Ok(commands)
    }

    /// Clear the cache
    pub fn clear_cache(&self) {
        self.cache.lock().clear();
    }

    /// Get cache hit rate statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.lock();
        (cache.len(), cache.cap().get())
    }
}

impl CommandContext {
    /// Gather current context from environment
    pub fn gather() -> Self {
        Self {
            shell: std::env::var("SHELL")
                .unwrap_or_else(|_| "/bin/bash".to_string())
                .split('/')
                .last()
                .unwrap_or("bash")
                .to_string(),
            current_dir: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "~".to_string()),
            os: std::env::consts::OS.to_string(),
        }
    }

    /// Create context with specific values (for testing)
    pub fn new(shell: String, current_dir: String, os: String) -> Self {
        Self {
            shell,
            current_dir,
            os,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_commands() {
        let response = "find . -name '*.rs'\nwc -l";
        let commands = LLMClient::parse_commands(response).unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], "find . -name '*.rs'");
        assert_eq!(commands[1], "wc -l");
    }

    #[test]
    fn test_parse_commands_with_markdown() {
        let response = r#"```bash
find . -name '*.rs'
wc -l
```"#;
        let commands = LLMClient::parse_commands(response).unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], "find . -name '*.rs'");
        assert_eq!(commands[1], "wc -l");
    }

    #[test]
    fn test_parse_commands_with_numbers() {
        let response = "1. git add .\n2. git commit -m 'Update'\n3. git push";
        let commands = LLMClient::parse_commands(response).unwrap();
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0], "git add .");
        assert_eq!(commands[1], "git commit -m 'Update'");
        assert_eq!(commands[2], "git push");
    }

    #[test]
    fn test_parse_commands_with_comments() {
        let response = "# List files\nls -la\n# Count lines\nwc -l";
        let commands = LLMClient::parse_commands(response).unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], "ls -la");
        assert_eq!(commands[1], "wc -l");
    }

    #[test]
    fn test_build_prompt() {
        let context = CommandContext::new(
            "zsh".to_string(),
            "/Users/sam/project".to_string(),
            "macos".to_string(),
        );

        let prompt = LLMClient::build_prompt("list all files", &context);
        assert!(prompt.contains("zsh"));
        assert!(prompt.contains("/Users/sam/project"));
        assert!(prompt.contains("macos"));
        assert!(prompt.contains("list all files"));
    }

    #[test]
    fn test_context_gather() {
        let context = CommandContext::gather();
        assert!(!context.shell.is_empty());
        assert!(!context.current_dir.is_empty());
        assert!(!context.os.is_empty());
    }
}
