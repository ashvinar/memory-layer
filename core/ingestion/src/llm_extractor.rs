use anyhow::{Context, Result};
use chrono::Utc;
use memory_layer_schemas::{generate_memory_id, Memory, MemoryKind, Turn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, warn};

use crate::heuristic::{Confidence, ExtractedMemory};

/// Configuration for LLM-based extraction
#[derive(Debug, Clone)]
pub struct LLMConfig {
    pub provider: LLMProvider,
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LLMProvider {
    Ollama,
    OpenAI,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            provider: LLMProvider::Ollama,
            api_key: None,
            base_url: "http://localhost:11434".to_string(),
            model: "llama3.2:3b".to_string(),
            timeout_secs: 30,
        }
    }
}

impl LLMConfig {
    /// Create config from environment variables
    pub fn from_env() -> Result<Self> {
        let provider = std::env::var("LLM_PROVIDER")
            .unwrap_or_else(|_| "ollama".to_string())
            .to_lowercase();

        let provider = match provider.as_str() {
            "openai" => LLMProvider::OpenAI,
            _ => LLMProvider::Ollama,
        };

        let base_url = match provider {
            LLMProvider::Ollama => {
                std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string())
            }
            LLMProvider::OpenAI => {
                std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com".to_string())
            }
        };

        let model = match provider {
            LLMProvider::Ollama => {
                std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2:3b".to_string())
            }
            LLMProvider::OpenAI => {
                std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string())
            }
        };

        let api_key = if provider == LLMProvider::OpenAI {
            Some(std::env::var("OPENAI_API_KEY")
                .context("OPENAI_API_KEY required for OpenAI provider")?)
        } else {
            None
        };

        Ok(Self {
            provider,
            api_key,
            base_url,
            model,
            timeout_secs: 30,
        })
    }
}

/// LLM response for extraction
#[derive(Debug, Deserialize, Serialize)]
struct ExtractionResponse {
    memories: Vec<ExtractedMemoryData>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ExtractedMemoryData {
    kind: String,
    text: String,
    topic: Option<String>,
    entities: Vec<String>,
    confidence: f32,
    reasoning: Option<String>,
}

/// LLM-based extractor for complex text analysis
pub struct LLMExtractor {
    config: LLMConfig,
    client: Client,
}

impl LLMExtractor {
    pub fn new(config: LLMConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap();

        Self { config, client }
    }

    /// Try to create from environment, returns None if LLM extraction is disabled
    pub fn from_env_optional() -> Option<Self> {
        let use_llm = std::env::var("USE_LLM_EXTRACTION")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase();

        if use_llm == "true" || use_llm == "1" {
            match LLMConfig::from_env() {
                Ok(config) => Some(Self::new(config)),
                Err(e) => {
                    warn!("Failed to initialize LLM extractor: {}", e);
                    None
                }
            }
        } else {
            None
        }
    }

    /// Extract memories using LLM
    pub async fn extract(&self, turn: &Turn) -> Result<Vec<ExtractedMemory>> {
        let prompt = self.build_extraction_prompt(turn);

        let response = match self.config.provider {
            LLMProvider::Ollama => self.call_ollama(&prompt).await?,
            LLMProvider::OpenAI => self.call_openai(&prompt).await?,
        };

        self.parse_llm_response(&response, turn)
    }

    /// Build extraction prompt for LLM
    fn build_extraction_prompt(&self, turn: &Turn) -> String {
        format!(
            r#"Extract structured memories from the following text. Identify:
1. DECISIONS - commitments or choices made (with reasoning)
2. TASKS - actionable items or TODOs (with priority context)
3. FACTS - important information, definitions, or key-value pairs
4. CODE REFERENCES - mentions of functions, classes, files, or code

Text:
{}

Source context:
- App: {:?}
- Path: {}
- URL: {}

Return a JSON object with this structure:
{{
  "memories": [
    {{
      "kind": "decision|task|fact|snippet",
      "text": "extracted text with context",
      "topic": "inferred topic",
      "entities": ["entity1", "entity2"],
      "confidence": 0.0-1.0,
      "reasoning": "why this was extracted"
    }}
  ]
}}

Only extract clear, actionable memories. Include context and reasoning for each extraction."#,
            turn.user_text,
            turn.source.app,
            turn.source.path.as_deref().unwrap_or("N/A"),
            turn.source.url.as_deref().unwrap_or("N/A")
        )
    }

    /// Call Ollama API
    async fn call_ollama(&self, prompt: &str) -> Result<String> {
        let url = format!("{}/api/generate", self.config.base_url);

        let request_body = json!({
            "model": self.config.model,
            "prompt": prompt,
            "stream": false,
            "format": "json",
            "options": {
                "temperature": 0.3,
                "num_predict": 1024,
            }
        });

        debug!("Calling Ollama at {}", url);

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .context("Failed to call Ollama API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error {}: {}", status, error_text);
        }

        #[derive(Deserialize)]
        struct OllamaResponse {
            response: String,
        }

        let ollama_response: OllamaResponse = response
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        Ok(ollama_response.response)
    }

    /// Call OpenAI API
    async fn call_openai(&self, prompt: &str) -> Result<String> {
        let url = format!("{}/v1/chat/completions", self.config.base_url);

        let request_body = json!({
            "model": self.config.model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are a memory extraction assistant. Extract structured information from user text and return valid JSON."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": 0.3,
            "max_tokens": 1024,
            "response_format": { "type": "json_object" }
        });

        debug!("Calling OpenAI at {}", url);

        let mut request = self.client.post(&url).json(&request_body);

        if let Some(ref api_key) = self.config.api_key {
            request = request.bearer_auth(api_key);
        }

        let response = request
            .send()
            .await
            .context("Failed to call OpenAI API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API error {}: {}", status, error_text);
        }

        #[derive(Deserialize)]
        struct OpenAIResponse {
            choices: Vec<OpenAIChoice>,
        }

        #[derive(Deserialize)]
        struct OpenAIChoice {
            message: OpenAIMessage,
        }

        #[derive(Deserialize)]
        struct OpenAIMessage {
            content: String,
        }

        let openai_response: OpenAIResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        openai_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("No response from OpenAI"))
    }

    /// Parse LLM response into memories
    fn parse_llm_response(&self, response: &str, turn: &Turn) -> Result<Vec<ExtractedMemory>> {
        let extraction: ExtractionResponse = serde_json::from_str(response)
            .context("Failed to parse LLM extraction response")?;

        let mut memories = Vec::new();

        for data in extraction.memories {
            let kind = match data.kind.to_lowercase().as_str() {
                "decision" => MemoryKind::Decision,
                "task" => MemoryKind::Task,
                "fact" => MemoryKind::Fact,
                "snippet" => MemoryKind::Snippet,
                _ => {
                    warn!("Unknown memory kind: {}, defaulting to Fact", data.kind);
                    MemoryKind::Fact
                }
            };

            // Set TTL for tasks
            let ttl = if matches!(kind, MemoryKind::Task) {
                Some(86400 * 7) // 7 days default for LLM-extracted tasks
            } else {
                None
            };

            let topic = data.topic.unwrap_or_else(|| {
                turn.source
                    .path
                    .as_ref()
                    .and_then(|p| p.split('/').last())
                    .unwrap_or("general")
                    .to_string()
            });

            memories.push(ExtractedMemory {
                memory: Memory {
                    id: generate_memory_id(),
                    kind,
                    topic,
                    text: data.text,
                    snippet: None, // LLM doesn't extract code snippets directly
                    entities: data.entities,
                    provenance: vec![turn.id.clone()],
                    created_at: Utc::now().to_rfc3339(),
                    ttl,
                },
                confidence: Confidence::new(data.confidence),
            });
        }

        debug!("LLM extracted {} memories", memories.len());
        Ok(memories)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_layer_schemas::{generate_thread_id, generate_turn_id, SourceApp, TurnSource};

    #[test]
    fn test_config_from_env() {
        std::env::set_var("LLM_PROVIDER", "ollama");
        std::env::set_var("OLLAMA_URL", "http://localhost:11434");
        std::env::set_var("OLLAMA_MODEL", "llama3.2:3b");

        let config = LLMConfig::from_env().unwrap();
        assert_eq!(config.provider, LLMProvider::Ollama);
        assert_eq!(config.base_url, "http://localhost:11434");
        assert_eq!(config.model, "llama3.2:3b");
    }

    #[test]
    fn test_extraction_prompt() {
        let config = LLMConfig::default();
        let extractor = LLMExtractor::new(config);

        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "I decided to use Rust for performance. TODO: migrate the API.".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::Claude,
                url: None,
                path: Some("src/main.rs".to_string()),
            },
        };

        let prompt = extractor.build_extraction_prompt(&turn);
        assert!(prompt.contains("DECISIONS"));
        assert!(prompt.contains("TASKS"));
        assert!(prompt.contains("FACTS"));
        assert!(prompt.contains("I decided to use Rust"));
    }

    #[test]
    fn test_parse_llm_response() {
        let config = LLMConfig::default();
        let extractor = LLMExtractor::new(config);

        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "Test".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::Claude,
                url: None,
                path: None,
            },
        };

        let llm_response = r#"{
            "memories": [
                {
                    "kind": "decision",
                    "text": "Decided to use Rust for the project",
                    "topic": "programming",
                    "entities": ["Rust"],
                    "confidence": 0.9,
                    "reasoning": "Clear commitment to a technology choice"
                },
                {
                    "kind": "task",
                    "text": "Migrate API to new framework",
                    "topic": "api",
                    "entities": ["API"],
                    "confidence": 0.85,
                    "reasoning": "Action item with clear intent"
                }
            ]
        }"#;

        let memories = extractor.parse_llm_response(llm_response, &turn).unwrap();
        assert_eq!(memories.len(), 2);
        assert!(matches!(memories[0].memory.kind, MemoryKind::Decision));
        assert!(matches!(memories[1].memory.kind, MemoryKind::Task));
        assert!(memories[0].confidence.is_confident());
    }

    #[test]
    fn test_optional_llm_extractor() {
        std::env::set_var("USE_LLM_EXTRACTION", "false");
        let extractor = LLMExtractor::from_env_optional();
        assert!(extractor.is_none());

        std::env::set_var("USE_LLM_EXTRACTION", "true");
        std::env::set_var("LLM_PROVIDER", "ollama");
        let extractor = LLMExtractor::from_env_optional();
        assert!(extractor.is_some());
    }
}
