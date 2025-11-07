/// LLM Providers for A-mem Memory Enrichment
/// Supports OpenAI and Ollama for local/remote LLM processing

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use memory_layer_schemas::AgenticMemory;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

use crate::amem::{LLMProvider, MemoryEnrichment, SuggestedLink};

/// OpenAI GPT Provider for memory enrichment
pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| "gpt-4o-mini".to_string()),
        }
    }

    async fn call_openai(&self, prompt: String, system: Option<String>) -> Result<String> {
        let mut messages = vec![];

        if let Some(sys) = system {
            messages.push(json!({
                "role": "system",
                "content": sys
            }));
        }

        messages.push(json!({
            "role": "user",
            "content": prompt
        }));

        let request_body = json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.7,
            "max_tokens": 1000
        });

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("OpenAI API error: {}", error_text));
        }

        let response_json: OpenAIResponse = response.json().await?;

        response_json.choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| anyhow!("Empty response from OpenAI"))
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn enrich_memory(&self, content: &str, context: &str) -> Result<MemoryEnrichment> {
        let prompt = format!(
            r#"Analyze the following memory and extract structured information.

Memory Content: {}
Context: {}

Please provide a JSON response with the following structure:
{{
    "tags": ["tag1", "tag2", ...], // 3-5 relevant tags
    "keywords": ["keyword1", "keyword2", ...], // 5-10 important keywords
    "category": "category_name", // One of: task, decision, fact, code, conversation, document, reference
    "summary": "brief summary", // 1-2 sentence summary
    "context_description": "enhanced context" // More descriptive context
}}

Focus on extracting actionable and searchable metadata."#,
            content, context
        );

        let response = self.call_openai(
            prompt,
            Some("You are an expert memory analyst following Zettelkasten principles.".to_string())
        ).await?;

        // Parse JSON response
        let enrichment: MemoryEnrichment = serde_json::from_str(&response)
            .map_err(|e| anyhow!("Failed to parse enrichment: {}", e))?;

        Ok(enrichment)
    }

    async fn reflect(&self, memories: Vec<&AgenticMemory>, query: &str) -> Result<String> {
        let memory_descriptions: Vec<String> = memories.iter()
            .enumerate()
            .map(|(i, m)| {
                format!(
                    "Memory {}: [{}] {} (Keywords: {}, Tags: {})",
                    i + 1,
                    m.context,
                    m.content.chars().take(200).collect::<String>(),
                    m.keywords.join(", "),
                    m.tags.join(", ")
                )
            })
            .collect();

        let prompt = format!(
            r#"Based on these memories, provide insights for the query: "{}"

Memories:
{}

Please provide:
1. A synthesis of relevant information
2. Key patterns or connections
3. Actionable insights or recommendations
4. Any gaps in knowledge

Keep the response concise and focused."#,
            query,
            memory_descriptions.join("\n\n")
        );

        self.call_openai(
            prompt,
            Some("You are an intelligent assistant that synthesizes memories to provide insights.".to_string())
        ).await
    }

    async fn extract_keywords(&self, content: &str) -> Result<Vec<String>> {
        let prompt = format!(
            r#"Extract 5-10 important keywords from this text. Return only a JSON array of strings.

Text: {}

Example response: ["keyword1", "keyword2", "keyword3"]"#,
            content
        );

        let response = self.call_openai(prompt, None).await?;
        let keywords: Vec<String> = serde_json::from_str(&response)?;

        Ok(keywords)
    }

    async fn suggest_links(&self, source: &AgenticMemory, candidates: Vec<&AgenticMemory>) -> Result<Vec<SuggestedLink>> {
        let candidate_descriptions: Vec<String> = candidates.iter()
            .map(|m| {
                format!(
                    "ID: {} | Context: {} | Summary: {}",
                    m.id.0,
                    m.context,
                    m.content.chars().take(100).collect::<String>()
                )
            })
            .collect();

        let prompt = format!(
            r#"Analyze the source memory and suggest which candidate memories it should be linked to.

Source Memory:
- Content: {}
- Context: {}
- Keywords: {}
- Tags: {}

Candidate Memories:
{}

Return a JSON array of suggested links (max 5):
[
    {{
        "target_id": "memory_id",
        "strength": 0.0-1.0,
        "rationale": "reason for the link"
    }}
]

Only suggest links with strength > 0.65."#,
            source.content.chars().take(500).collect::<String>(),
            source.context,
            source.keywords.join(", "),
            source.tags.join(", "),
            candidate_descriptions.join("\n")
        );

        let response = self.call_openai(prompt, None).await?;
        let links: Vec<SuggestedLink> = serde_json::from_str(&response)?;

        Ok(links)
    }
}

/// Ollama Provider for local LLM inference
pub struct OllamaProvider {
    client: Client,
    base_url: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(model: Option<String>, base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
            model: model.unwrap_or_else(|| "llama3.2".to_string()),
        }
    }

    async fn call_ollama(&self, prompt: String) -> Result<String> {
        let request_body = json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false
        });

        let response = self.client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Ollama API error: {}", error_text));
        }

        let response_json: OllamaResponse = response.json().await?;
        Ok(response_json.response)
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn enrich_memory(&self, content: &str, context: &str) -> Result<MemoryEnrichment> {
        let prompt = format!(
            r#"Analyze this memory and extract metadata. Return ONLY valid JSON.

Memory: {}
Context: {}

Required JSON format:
{{
    "tags": ["tag1", "tag2"],
    "keywords": ["keyword1", "keyword2"],
    "category": "category",
    "summary": "summary",
    "context_description": "context"
}}"#,
            content, context
        );

        let response = self.call_ollama(prompt).await?;

        // Try to extract JSON from response
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                &response
            }
        } else {
            &response
        };

        let enrichment: MemoryEnrichment = serde_json::from_str(json_str)
            .unwrap_or_else(|_| {
                // Fallback enrichment
                MemoryEnrichment {
                    tags: vec!["untagged".to_string()],
                    keywords: extract_keywords_simple(content),
                    category: Some("general".to_string()),
                    summary: content.chars().take(200).collect(),
                    context_description: context.to_string(),
                }
            });

        Ok(enrichment)
    }

    async fn reflect(&self, memories: Vec<&AgenticMemory>, query: &str) -> Result<String> {
        let memory_text = memories.iter()
            .map(|m| format!("- {}: {}", m.context, m.content.chars().take(150).collect::<String>()))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            r#"Based on these memories, answer: {}

Memories:
{}

Provide a concise synthesis with key insights."#,
            query, memory_text
        );

        self.call_ollama(prompt).await
    }

    async fn extract_keywords(&self, content: &str) -> Result<Vec<String>> {
        let prompt = format!(
            r#"Extract 5-10 keywords from: {}

Return ONLY a JSON array: ["keyword1", "keyword2", ...]"#,
            content
        );

        let response = self.call_ollama(prompt).await?;

        // Try to parse JSON array
        if let Ok(keywords) = serde_json::from_str::<Vec<String>>(&response) {
            Ok(keywords)
        } else {
            // Fallback to simple extraction
            Ok(extract_keywords_simple(content))
        }
    }

    async fn suggest_links(&self, source: &AgenticMemory, candidates: Vec<&AgenticMemory>) -> Result<Vec<SuggestedLink>> {
        // For local models, use simpler similarity-based linking
        let mut links = Vec::new();

        for candidate in candidates {
            // Calculate simple keyword overlap
            let source_keywords: std::collections::HashSet<_> = source.keywords.iter().collect();
            let candidate_keywords: std::collections::HashSet<_> = candidate.keywords.iter().collect();

            let overlap = source_keywords.intersection(&candidate_keywords).count();
            let total = source_keywords.len().max(1);

            let strength = (overlap as f32 / total as f32).min(1.0);

            if strength > 0.65 {
                links.push(SuggestedLink {
                    target_id: candidate.id.clone(),
                    strength,
                    rationale: format!("Keyword overlap: {} common keywords", overlap),
                });
            }
        }

        links.sort_by(|a, b| b.strength.partial_cmp(&a.strength).unwrap());
        links.truncate(5);

        Ok(links)
    }
}

/// Claude Provider for Anthropic's Claude API
pub struct ClaudeProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl ClaudeProvider {
    pub fn new(api_key: String, model: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string()),
        }
    }

    async fn call_claude(&self, prompt: String, system: Option<String>) -> Result<String> {
        let mut request_body = json!({
            "model": self.model,
            "max_tokens": 1000,
            "messages": [{
                "role": "user",
                "content": prompt
            }]
        });

        if let Some(sys) = system {
            request_body["system"] = json!(sys);
        }

        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Claude API error: {}", error_text));
        }

        let response_json: ClaudeResponse = response.json().await?;

        response_json.content
            .first()
            .and_then(|c| c.text.clone())
            .ok_or_else(|| anyhow!("Empty response from Claude"))
    }
}

#[async_trait]
impl LLMProvider for ClaudeProvider {
    async fn enrich_memory(&self, content: &str, context: &str) -> Result<MemoryEnrichment> {
        let prompt = format!(
            r#"Analyze this memory and extract structured metadata following Zettelkasten principles.

Memory Content: {}
Context: {}

Provide a JSON response:
{{
    "tags": ["tag1", "tag2", ...],
    "keywords": ["keyword1", "keyword2", ...],
    "category": "category_name",
    "summary": "brief summary",
    "context_description": "enhanced context"
}}

Categories: task, decision, fact, code, conversation, document, reference"#,
            content, context
        );

        let response = self.call_claude(
            prompt,
            Some("You are an expert at organizing memories using Zettelkasten principles.".to_string())
        ).await?;

        // Extract JSON from response
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                &response
            }
        } else {
            &response
        };

        let enrichment: MemoryEnrichment = serde_json::from_str(json_str)?;
        Ok(enrichment)
    }

    async fn reflect(&self, memories: Vec<&AgenticMemory>, query: &str) -> Result<String> {
        let memory_descriptions: Vec<String> = memories.iter()
            .enumerate()
            .map(|(i, m)| {
                format!(
                    "Memory {}: [{}] {}\nKeywords: {}\nTags: {}",
                    i + 1,
                    m.context,
                    m.content.chars().take(300).collect::<String>(),
                    m.keywords.join(", "),
                    m.tags.join(", ")
                )
            })
            .collect();

        let prompt = format!(
            r#"Synthesize these memories to answer: "{}"

{}

Provide:
1. Key information synthesis
2. Patterns and connections
3. Actionable insights
4. Knowledge gaps

Be concise and insightful."#,
            query,
            memory_descriptions.join("\n\n")
        );

        self.call_claude(
            prompt,
            Some("You synthesize memories to provide deep insights and connections.".to_string())
        ).await
    }

    async fn extract_keywords(&self, content: &str) -> Result<Vec<String>> {
        let prompt = format!(
            r#"Extract 5-10 important keywords from this text. Return ONLY a JSON array.

Text: {}

Response format: ["keyword1", "keyword2", ...]"#,
            content
        );

        let response = self.call_claude(prompt, None).await?;

        // Extract JSON array from response
        let json_str = if let Some(start) = response.find('[') {
            if let Some(end) = response.rfind(']') {
                &response[start..=end]
            } else {
                &response
            }
        } else {
            &response
        };

        let keywords: Vec<String> = serde_json::from_str(json_str)?;
        Ok(keywords)
    }

    async fn suggest_links(&self, source: &AgenticMemory, candidates: Vec<&AgenticMemory>) -> Result<Vec<SuggestedLink>> {
        let candidate_descriptions: Vec<String> = candidates.iter()
            .take(20) // Limit candidates for context window
            .map(|m| {
                format!(
                    "ID: {} | {}: {}",
                    m.id.0,
                    m.context,
                    m.content.chars().take(100).collect::<String>()
                )
            })
            .collect();

        let prompt = format!(
            r#"Suggest memory links based on semantic relationships.

Source Memory:
Content: {}
Context: {}
Keywords: {}

Candidates:
{}

Return JSON array of links (max 5, strength > 0.65):
[{{"target_id": "id", "strength": 0.0-1.0, "rationale": "reason"}}]"#,
            source.content.chars().take(500).collect::<String>(),
            source.context,
            source.keywords.join(", "),
            candidate_descriptions.join("\n")
        );

        let response = self.call_claude(prompt, None).await?;

        // Extract JSON from response
        let json_str = if let Some(start) = response.find('[') {
            if let Some(end) = response.rfind(']') {
                &response[start..=end]
            } else {
                &response
            }
        } else {
            &response
        };

        let links: Vec<SuggestedLink> = serde_json::from_str(json_str)?;
        Ok(links)
    }
}

// Helper function for simple keyword extraction
fn extract_keywords_simple(content: &str) -> Vec<String> {
    use std::collections::HashSet;

    let stop_words: HashSet<&str> = HashSet::from([
        "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
        "of", "with", "by", "from", "as", "is", "was", "are", "were", "been",
        "this", "that", "these", "those", "it", "its", "they", "their",
    ]);

    let mut word_freq: HashMap<String, usize> = HashMap::new();

    for word in content.to_lowercase().split_whitespace() {
        let cleaned = word.trim_matches(|c: char| !c.is_alphanumeric());
        if cleaned.len() > 3 && !stop_words.contains(cleaned) {
            *word_freq.entry(cleaned.to_string()).or_insert(0) += 1;
        }
    }

    let mut keywords: Vec<(String, usize)> = word_freq.into_iter().collect();
    keywords.sort_by(|a, b| b.1.cmp(&a.1));

    keywords.into_iter()
        .take(10)
        .map(|(word, _)| word)
        .collect()
}

// Response structures
#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAIMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
}

#[derive(Debug, Deserialize)]
struct ClaudeContent {
    text: Option<String>,
}