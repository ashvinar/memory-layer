use anyhow::Result;
use memory_layer_schemas::{
    generate_capsule_id, ContextCapsule, ContextRequest, ContextStyle, Memory, Message, MessageRole,
    ProvenanceItem, ProvenanceType,
};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::templates::TemplateRenderer;

/// Context Capsule composer
pub struct Composer {
    renderer: TemplateRenderer,
    cache: HashMap<String, ContextCapsule>,
    client: reqwest::Client,
    ingestion_url: String,
}

impl Composer {
    pub fn new() -> Self {
        let ingestion_url = std::env::var("INGESTION_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:21954".to_string());

        Self {
            renderer: TemplateRenderer::new(),
            cache: HashMap::new(),
            client: reqwest::Client::new(),
            ingestion_url,
        }
    }

    /// Compose a Context Capsule from a request
    pub async fn compose(&mut self, request: &ContextRequest) -> Result<ContextCapsule> {
        info!("Composing context capsule");

        // Determine topic
        let topic = request
            .topic_hint
            .clone()
            .unwrap_or_else(|| "General".to_string());

        // Get style
        let style = if request.budget_tokens < 100 {
            ContextStyle::Short
        } else if request.budget_tokens < 300 {
            ContextStyle::Standard
        } else {
            ContextStyle::Detailed
        };

        debug!(
            "Using style: {:?}, budget: {}",
            style, request.budget_tokens
        );

        // Fetch high-priority memories using hierarchy-aware importance filtering
        let memories = self.fetch_context_memories(&topic, request.budget_tokens).await
            .unwrap_or_else(|e| {
                warn!("Failed to fetch memories: {}, using empty context", e);
                Vec::new()
            });

        // Render preamble
        let preamble_text = self
            .renderer
            .render(&style, &topic, &memories, request.budget_tokens);

        // Estimate tokens (rough: ~4 chars per token)
        let estimated_tokens = preamble_text.len() as u64 / 4;

        // Create system message for Pull/Merge lanes
        let system_message = Message {
            role: MessageRole::System,
            content: preamble_text.clone(),
        };

        // Build provenance
        let provenance = vec![ProvenanceItem {
            r#type: ProvenanceType::Memory,
            r#ref: format!("{} memories", memories.len()),
            when: None,
        }];

        // Check if this is a delta request
        let delta_of = request.last_capsule_id.clone();

        let capsule = ContextCapsule {
            capsule_id: generate_capsule_id(),
            preamble_text,
            messages: vec![system_message],
            provenance,
            delta_of,
            ttl_sec: 600, // 10 minutes
            token_count: Some(estimated_tokens),
            style: Some(style),
        };

        // Cache it
        if let Some(ref thread_key) = request.thread_key {
            self.cache.insert(thread_key.clone(), capsule.clone());
        }

        info!(
            "Composed capsule: {} ({} tokens)",
            capsule.capsule_id, estimated_tokens
        );
        Ok(capsule)
    }

    /// Fetch relevant memories for context using hierarchy-aware APIs
    async fn fetch_context_memories(&self, _topic: &str, budget: u64) -> Result<Vec<Memory>> {
        // Calculate how many memories to fetch based on budget
        // Rough estimate: 100 tokens = ~2-3 memories
        let limit = ((budget / 40).max(5).min(50)) as usize;

        info!("Fetching up to {} high-priority memories for context", limit);

        // Fetch high-priority memories using importance-based filtering
        let url = format!("{}/importance/high-priority?limit={}", self.ingestion_url, limit);

        let response = self.client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to fetch memories: HTTP {}", response.status());
        }

        #[derive(serde::Deserialize)]
        struct MemoriesResponse {
            memories: Vec<Memory>,
        }

        let body: MemoriesResponse = response.json().await?;

        info!("Retrieved {} high-priority memories", body.memories.len());
        Ok(body.memories)
    }

    /// Compute delta between two capsules
    pub fn compute_delta(&self, prev: &ContextCapsule, current: &ContextCapsule) -> DeltaResult {
        // Simple comparison for MVP
        if prev.preamble_text == current.preamble_text {
            DeltaResult::NoChange
        } else {
            let similarity = self.text_similarity(&prev.preamble_text, &current.preamble_text);
            if similarity > 0.9 {
                DeltaResult::Small
            } else {
                DeltaResult::Changed
            }
        }
    }

    fn text_similarity(&self, a: &str, b: &str) -> f32 {
        // Simple word-based similarity
        let words_a: std::collections::HashSet<&str> = a.split_whitespace().collect();
        let words_b: std::collections::HashSet<&str> = b.split_whitespace().collect();

        let intersection = words_a.intersection(&words_b).count();
        let union = words_a.union(&words_b).count();

        if union == 0 {
            return 1.0;
        }

        intersection as f32 / union as f32
    }

    /// Get cached capsule for a thread
    pub fn get_cached(&self, thread_key: &str) -> Option<&ContextCapsule> {
        self.cache.get(thread_key)
    }

    /// Clear cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

#[derive(Debug, PartialEq)]
pub enum DeltaResult {
    NoChange,
    Small,
    Changed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_compose_short() {
        let mut composer = Composer::new();
        let request = ContextRequest {
            topic_hint: Some("Test".to_string()),
            intent: None,
            budget_tokens: 80,
            scopes: vec![],
            thread_key: None,
            last_capsule_id: None,
        };

        let capsule = composer.compose(&request).await.unwrap();
        assert!(capsule.token_count.unwrap() < 150);
        assert!(matches!(capsule.style, Some(ContextStyle::Short)));
    }

    #[tokio::test]
    async fn test_compose_standard() {
        let mut composer = Composer::new();
        let request = ContextRequest {
            topic_hint: Some("Test".to_string()),
            intent: None,
            budget_tokens: 220,
            scopes: vec![],
            thread_key: None,
            last_capsule_id: None,
        };

        let capsule = composer.compose(&request).await.unwrap();
        assert!(capsule.token_count.is_some());
        assert!(matches!(capsule.style, Some(ContextStyle::Standard)));
    }

    #[tokio::test]
    async fn test_compose_caching() {
        let mut composer = Composer::new();
        let request = ContextRequest {
            topic_hint: Some("Test".to_string()),
            intent: None,
            budget_tokens: 220,
            scopes: vec![],
            thread_key: Some("test_thread".to_string()),
            last_capsule_id: None,
        };

        let capsule = composer.compose(&request).await.unwrap();
        let cached = composer.get_cached("test_thread");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().capsule_id, capsule.capsule_id);
    }
}
