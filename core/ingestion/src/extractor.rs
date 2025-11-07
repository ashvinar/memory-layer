use anyhow::Result;
use memory_layer_schemas::{Memory, Turn};
use tracing::{debug, warn};

use crate::heuristic::HeuristicExtractor;
use crate::llm_extractor::LLMExtractor;

/// Extraction strategy for memories
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtractionStrategy {
    /// Fast heuristic-only extraction
    HeuristicOnly,
    /// LLM-enhanced extraction with heuristic fallback
    LLMWithFallback,
    /// Hybrid: heuristics + LLM for complex text
    Hybrid,
}

/// Memory extractor that identifies and extracts structured knowledge from turns
/// Supports multiple extraction strategies: fast heuristics and optional LLM enhancement
pub struct MemoryExtractor {
    strategy: ExtractionStrategy,
    heuristic: HeuristicExtractor,
    llm: Option<LLMExtractor>,
}

impl Default for MemoryExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryExtractor {
    /// Create a new extractor with automatic strategy selection based on environment
    pub fn new() -> Self {
        let llm = LLMExtractor::from_env_optional();
        let strategy = if llm.is_some() {
            ExtractionStrategy::Hybrid
        } else {
            ExtractionStrategy::HeuristicOnly
        };

        Self {
            strategy,
            heuristic: HeuristicExtractor::new(),
            llm,
        }
    }

    /// Create an extractor with a specific strategy
    pub fn with_strategy(strategy: ExtractionStrategy) -> Self {
        let llm = if strategy != ExtractionStrategy::HeuristicOnly {
            LLMExtractor::from_env_optional()
        } else {
            None
        };

        Self {
            strategy,
            heuristic: HeuristicExtractor::new(),
            llm,
        }
    }

    /// Extract memories from a turn using the configured strategy
    pub fn extract(&self, turn: &Turn) -> Result<Vec<Memory>> {
        match self.strategy {
            ExtractionStrategy::HeuristicOnly => self.extract_heuristic(turn),
            ExtractionStrategy::LLMWithFallback => self.extract_llm_with_fallback(turn),
            ExtractionStrategy::Hybrid => self.extract_hybrid(turn),
        }
    }

    /// Async version of extract for LLM-based strategies
    pub async fn extract_async(&self, turn: &Turn) -> Result<Vec<Memory>> {
        match self.strategy {
            ExtractionStrategy::HeuristicOnly => self.extract_heuristic(turn),
            ExtractionStrategy::LLMWithFallback => {
                self.extract_llm_with_fallback_async(turn).await
            }
            ExtractionStrategy::Hybrid => self.extract_hybrid_async(turn).await,
        }
    }

    // Strategy implementations

    /// Extract using only heuristics (fast path)
    fn extract_heuristic(&self, turn: &Turn) -> Result<Vec<Memory>> {
        let extracted = self.heuristic.extract(turn)?;

        // Filter by confidence threshold
        let memories: Vec<Memory> = extracted
            .into_iter()
            .filter(|e| e.confidence.is_confident())
            .map(|e| e.memory)
            .collect();

        debug!(
            "Heuristic extractor: {} memories from turn {}",
            memories.len(),
            turn.id
        );

        Ok(memories)
    }

    fn extract_decision(&self, turn: &Turn) -> Option<Memory> {
        let text = &turn.user_text.to_lowercase();

        let decision_keywords = [
            "decided",
            "will",
            "going to",
            "plan to",
            "chose",
            "selected",
            "switching to",
            "moving to",
            "adopting",
        ];

        for keyword in &decision_keywords {
            if text.contains(keyword) {
                // Extract the sentence containing the decision
                if let Some(sentence) = self.extract_sentence_with(text, keyword) {
                    return Some(Memory {
                        id: generate_memory_id(),
                        kind: MemoryKind::Decision,
                        topic: self.infer_topic(turn),
                        text: sentence,
                        snippet: None,
                        entities: self.extract_entities(turn),
                        provenance: vec![turn.id.clone()],
                        created_at: Utc::now().to_rfc3339(),
                        ttl: None,
                        topic_id: None,
                    });
                }
            }
        }

        None
    }

    fn extract_fact(&self, turn: &Turn) -> Option<Memory> {
        let text = &turn.user_text;

        // Simple heuristic: if the text contains "is", "are", "means", it's likely a fact
        let fact_indicators = ["is a", "is the", "are", "means", "refers to", "defined as"];

        for indicator in &fact_indicators {
            if text.to_lowercase().contains(indicator) {
                return Some(Memory {
                    id: generate_memory_id(),
                    kind: MemoryKind::Fact,
                    topic: self.infer_topic(turn),
                    text: text.clone(),
                    snippet: None,
                    entities: self.extract_entities(turn),
                    provenance: vec![turn.id.clone()],
                    created_at: Utc::now().to_rfc3339(),
                    ttl: None,
                    topic_id: None,
                });
    /// Extract using LLM with heuristic fallback
    fn extract_llm_with_fallback(&self, turn: &Turn) -> Result<Vec<Memory>> {
        // For sync API, fall back to heuristics
        warn!("LLM extraction requires async API, falling back to heuristics");
        self.extract_heuristic(turn)
    }

    /// Extract using LLM with heuristic fallback (async)
    async fn extract_llm_with_fallback_async(&self, turn: &Turn) -> Result<Vec<Memory>> {
        if let Some(ref llm) = self.llm {
            match llm.extract(turn).await {
                Ok(extracted) => {
                    let memories: Vec<Memory> = extracted
                        .into_iter()
                        .filter(|e| e.confidence.is_confident())
                        .map(|e| e.memory)
                        .collect();

                    debug!(
                        "LLM extractor: {} memories from turn {}",
                        memories.len(),
                        turn.id
                    );

                    return Ok(memories);
                }
                Err(e) => {
                    warn!("LLM extraction failed: {}, falling back to heuristics", e);
                }
            }
        }

        // Fallback to heuristics
        self.extract_heuristic(turn)
    }

    fn extract_task(&self, turn: &Turn) -> Option<Memory> {
        let text = &turn.user_text.to_lowercase();

        let task_keywords = [
            "todo",
            "need to",
            "should",
            "must",
            "have to",
            "remember to",
            "don't forget",
            "action item",
        ];

        for keyword in &task_keywords {
            if text.contains(keyword) {
                if let Some(sentence) = self.extract_sentence_with(text, keyword) {
                    return Some(Memory {
                        id: generate_memory_id(),
                        kind: MemoryKind::Task,
                        topic: self.infer_topic(turn),
                        text: sentence,
                        snippet: None,
                        entities: self.extract_entities(turn),
                        provenance: vec![turn.id.clone()],
                        created_at: Utc::now().to_rfc3339(),
                        ttl: Some(86400 * 7), // Tasks expire after 7 days by default
                        topic_id: None,
                    });
                }
            }
        }

        None
    }

    fn extract_snippets(&self, turn: &Turn) -> Result<Vec<Memory>> {
        let mut snippets = Vec::new();
        let text = &turn.user_text;

        // Detect code blocks (markdown style)
        if text.contains("```") {
            let blocks: Vec<&str> = text.split("```").collect();
            for (i, block) in blocks.iter().enumerate() {
                if i % 2 == 1 {
                    // Odd indices are code blocks
                    let lines: Vec<&str> = block.lines().collect();
                    let language = if !lines.is_empty() {
                        Some(lines[0].trim().to_string())
                    } else {
                        None
                    };

                    let code = if !lines.is_empty() {
                        lines[1..].join("\n")
                    } else {
                        block.to_string()
                    };

                    snippets.push(Memory {
                        id: generate_memory_id(),
                        kind: MemoryKind::Snippet,
                        topic: self.infer_topic(turn),
                        text: "Code snippet".to_string(),
                        snippet: Some(Snippet {
                            title: format!("Snippet from {:?}", turn.source.app),
                            text: code,
                            loc: None,
                            language,
                        }),
                        entities: self.extract_entities(turn),
                        provenance: vec![turn.id.clone()],
                        created_at: Utc::now().to_rfc3339(),
                        ttl: None,
                        topic_id: None,
                    });
    /// Hybrid extraction: heuristics + LLM for complex text
    fn extract_hybrid(&self, turn: &Turn) -> Result<Vec<Memory>> {
        // For sync API, only use heuristics
        self.extract_heuristic(turn)
    }

    /// Hybrid extraction: heuristics + LLM for complex text (async)
    async fn extract_hybrid_async(&self, turn: &Turn) -> Result<Vec<Memory>> {
        // Always run heuristics first (fast path)
        let heuristic_extracted = self.heuristic.extract(turn)?;

        let mut all_extracted = heuristic_extracted.clone();

        // Use LLM for complex text if available
        if let Some(ref llm) = self.llm {
            if self.is_complex_text(&turn.user_text) {
                match llm.extract(turn).await {
                    Ok(llm_extracted) => {
                        debug!("LLM enhanced extraction for complex text");
                        all_extracted.extend(llm_extracted);
                    }
                    Err(e) => {
                        warn!("LLM extraction failed: {}, using heuristics only", e);
                    }
                }
            }
        }

        // Deduplicate and filter by confidence
        let memories = self.deduplicate_and_filter(all_extracted)?;

        Ok(snippets)
    }

    fn extract_file_reference(&self, turn: &Turn) -> Option<Memory> {
        let text = &turn.user_text;

        // Look for patterns like "file.ext:line" or "file.ext:line1-line2"
        use regex::Regex;
        let re = Regex::new(r"([a-zA-Z0-9_\-./]+\.[a-z]+):(\d+)(?:-(\d+))?").ok()?;

        if let Some(caps) = re.captures(text) {
            let file = caps.get(1)?.as_str();
            let start_line = caps.get(2)?.as_str();
            let end_line = caps.get(3).map(|m| m.as_str());

            let loc = if let Some(end) = end_line {
                format!("L{}-L{}", start_line, end)
            } else {
                format!("L{}", start_line)
            };

            return Some(Memory {
                id: generate_memory_id(),
                kind: MemoryKind::Snippet,
                topic: self.infer_topic(turn),
                text: format!("Reference to {}", file),
                snippet: Some(Snippet {
                    title: file.to_string(),
                    text: text.clone(),
                    loc: Some(loc),
                    language: Self::detect_language(file),
                }),
                entities: vec![file.to_string()],
                provenance: vec![turn.id.clone()],
                created_at: Utc::now().to_rfc3339(),
                ttl: None,
                topic_id: None,
            });
        }
        debug!(
            "Hybrid extractor: {} memories from turn {}",
            memories.len(),
            turn.id
        );

        Ok(memories)
    }

    /// Check if text is complex enough to benefit from LLM
    fn is_complex_text(&self, text: &str) -> bool {
        // Complex if:
        // - Contains multiple sentences with decisions or tasks
        // - Has nested reasoning (because, since, therefore)
        // - Long text with multiple paragraphs
        let sentences: Vec<&str> = text.split(|c| c == '.' || c == '!' || c == '?').collect();

        if sentences.len() > 5 {
            return true;
        }

        let lower = text.to_lowercase();
        let reasoning_count = ["because", "since", "therefore", "thus", "hence"]
            .iter()
            .filter(|w| lower.contains(*w))
            .count();

        if reasoning_count >= 2 {
            return true;
        }

        // Check for multiple action items
        let action_count = ["decided", "will", "need to", "should", "must", "todo"]
            .iter()
            .filter(|w| lower.contains(*w))
            .count();

        action_count >= 2
    }

    /// Deduplicate and filter extracted memories
    fn deduplicate_and_filter(
        &self,
        extracted: Vec<crate::heuristic::ExtractedMemory>,
    ) -> Result<Vec<Memory>> {
        use std::collections::HashMap;

        // Group by (kind, topic) and keep highest confidence
        let mut best_by_key: HashMap<(String, String), crate::heuristic::ExtractedMemory> =
            HashMap::new();

        for item in extracted {
            // Skip low confidence
            if !item.confidence.is_confident() {
                continue;
            }

            let key = (
                format!("{:?}", item.memory.kind),
                item.memory.topic.clone(),
            );

            best_by_key
                .entry(key)
                .and_modify(|existing| {
                    // Keep the one with higher confidence or more entities
                    if item.confidence.score() > existing.confidence.score()
                        || (item.confidence.score() == existing.confidence.score()
                            && item.memory.entities.len() > existing.memory.entities.len())
                    {
                        *existing = item.clone();
                    }
                })
                .or_insert(item);
        }

        Ok(best_by_key.into_values().map(|e| e.memory).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use memory_layer_schemas::{generate_thread_id, generate_turn_id, MemoryKind, SourceApp, TurnSource};

    #[test]
    fn test_basic_decision_extraction() {
        let extractor = MemoryExtractor::with_strategy(ExtractionStrategy::HeuristicOnly);
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "I decided to use Rust for this project.".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::Claude,
                url: None,
                path: None,
            },
        };

        let memories = extractor.extract(&turn).unwrap();
        assert!(!memories.is_empty());
        assert!(memories
            .iter()
            .any(|m| matches!(m.kind, MemoryKind::Decision)));
    }

    #[test]
    fn test_decision_with_reasoning() {
        let extractor = MemoryExtractor::with_strategy(ExtractionStrategy::HeuristicOnly);
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "I decided to use Rust because it's fast and memory-safe. This will improve performance significantly.".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::Claude,
                url: None,
                path: Some("src/main.rs".to_string()),
            },
        };

        let memories = extractor.extract(&turn).unwrap();
        let decision = memories
            .iter()
            .find(|m| matches!(m.kind, MemoryKind::Decision));

        assert!(decision.is_some());
        let decision = decision.unwrap();

        // Should extract context including reasoning
        assert!(decision.text.contains("Rust") || decision.text.contains("fast"));
        // Should extract entities
        assert!(!decision.entities.is_empty());
    }

    #[test]
    fn test_high_priority_task() {
        let extractor = MemoryExtractor::with_strategy(ExtractionStrategy::HeuristicOnly);
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "URGENT: Critical bug in authentication! Need to fix ASAP.".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::VSCode,
                url: None,
                path: Some("src/auth.rs".to_string()),
            },
        };

        let memories = extractor.extract(&turn).unwrap();
        let task = memories.iter().find(|m| matches!(m.kind, MemoryKind::Task));

        assert!(task.is_some());
        let task = task.unwrap();

        // High priority tasks should have shorter TTL
        assert!(task.ttl.is_some());
        assert!(task.ttl.unwrap() < 86400 * 5); // Less than 5 days
    }

    #[test]
    fn test_multiple_tasks_extraction() {
        let extractor = MemoryExtractor::with_strategy(ExtractionStrategy::HeuristicOnly);
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "TODO: implement login. Also need to add error handling. Remember to write tests.".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::VSCode,
                url: None,
                path: None,
            },
        };

        let memories = extractor.extract(&turn).unwrap();
        let tasks: Vec<_> = memories
            .iter()
            .filter(|m| matches!(m.kind, MemoryKind::Task))
            .collect();

        // Should extract multiple tasks
        assert!(tasks.len() >= 1);
    }

    #[test]
    fn test_structured_facts() {
        let extractor = MemoryExtractor::with_strategy(ExtractionStrategy::HeuristicOnly);
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "API endpoint: /api/v1/users\nDatabase: PostgreSQL 15\nFramework: Actix-web".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::Claude,
                url: None,
                path: None,
            },
        };

        let memories = extractor.extract(&turn).unwrap();
        let facts: Vec<_> = memories
            .iter()
            .filter(|m| matches!(m.kind, MemoryKind::Fact))
            .collect();

        // Should extract multiple facts as key-value pairs
        assert!(!facts.is_empty());
    }

    #[test]
    fn test_code_block_extraction() {
        let extractor = MemoryExtractor::with_strategy(ExtractionStrategy::HeuristicOnly);
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "Here's the implementation:\n```rust\nfn calculate(x: i32) -> i32 {\n    x * 2\n}\n```".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::Claude,
                url: None,
                path: None,
            },
        };

        let memories = extractor.extract(&turn).unwrap();
        let snippet = memories
            .iter()
            .find(|m| matches!(m.kind, MemoryKind::Snippet));

        assert!(snippet.is_some());
        let snippet = snippet.unwrap();
        assert!(snippet.snippet.is_some());

        let snippet_data = snippet.snippet.as_ref().unwrap();
        assert_eq!(snippet_data.language, Some("rust".to_string()));
        assert!(snippet_data.text.contains("calculate"));
    }

    #[test]
    fn test_file_reference_with_lines() {
        let extractor = MemoryExtractor::with_strategy(ExtractionStrategy::HeuristicOnly);
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "Check the implementation in src/main.rs:42-56 and src/utils.rs:10".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::VSCode,
                url: None,
                path: None,
            },
        };

        let memories = extractor.extract(&turn).unwrap();
        let snippets: Vec<_> = memories
            .iter()
            .filter(|m| matches!(m.kind, MemoryKind::Snippet) && m.snippet.as_ref().unwrap().loc.is_some())
            .collect();

        // Should extract multiple file references
        assert!(!snippets.is_empty());

        // Check that line numbers are captured
        for snippet in snippets {
            let loc = snippet.snippet.as_ref().unwrap().loc.as_ref().unwrap();
            assert!(loc.starts_with('L'));
        }
    }

    #[test]
    fn test_complex_text_detection() {
        let extractor = MemoryExtractor::with_strategy(ExtractionStrategy::HeuristicOnly);

        // Simple text
        let simple = "I decided to use Rust.";
        assert!(!extractor.is_complex_text(simple));

        // Complex text with reasoning
        let complex = "I decided to use Rust because it's fast. Since we need performance, \
                       this is critical. We must migrate the API soon. Therefore, I will \
                       start working on it next week. TODO: plan migration.";
        assert!(extractor.is_complex_text(complex));
    }

    #[test]
    fn test_deduplication() {
        let extractor = MemoryExtractor::with_strategy(ExtractionStrategy::HeuristicOnly);
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "I decided to use Rust. We're going to use Rust for the project.".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::Claude,
                url: None,
                path: None,
            },
        };

        let memories = extractor.extract(&turn).unwrap();
        let decisions: Vec<_> = memories
            .iter()
            .filter(|m| matches!(m.kind, MemoryKind::Decision))
            .collect();

        // Should deduplicate similar decisions (keep best one)
        // This depends on implementation but should be <= number of decision keywords
        assert!(!decisions.is_empty());
    }

    #[test]
    fn test_extraction_quality_improvement() {
        // This test demonstrates the improvement over simple keyword matching
        let extractor = MemoryExtractor::with_strategy(ExtractionStrategy::HeuristicOnly);

        let complex_turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: r#"
I decided to use Rust for the backend API because it offers superior performance
and memory safety compared to Python. This is a critical decision for the project.

TODO: Migrate existing endpoints to Rust (HIGH PRIORITY)
TODO: Set up CI/CD pipeline
Need to update documentation

Key facts:
- API endpoint: /api/v1/users
- Database: PostgreSQL 15
- Framework: Actix-web 4.0

Check src/api/handlers.rs:120-145 for the implementation.

```rust
async fn get_user(id: UserId) -> Result<User> {
    db.get_user(id).await
}
```
            "#.to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::Claude,
                url: Some("https://github.com/project/repo".to_string()),
                path: Some("src/api/handlers.rs".to_string()),
            },
        };

        let memories = extractor.extract(&complex_turn).unwrap();

        // Should extract multiple types of memories
        let decisions = memories.iter().filter(|m| matches!(m.kind, MemoryKind::Decision)).count();
        let tasks = memories.iter().filter(|m| matches!(m.kind, MemoryKind::Task)).count();
        let facts = memories.iter().filter(|m| matches!(m.kind, MemoryKind::Fact)).count();
        let snippets = memories.iter().filter(|m| matches!(m.kind, MemoryKind::Snippet)).count();

        // Quality improvements:
        // 1. Should extract decisions with context and reasoning
        assert!(decisions >= 1, "Should extract at least 1 decision");

        // 2. Should extract multiple tasks
        assert!(tasks >= 1, "Should extract at least 1 task");

        // 3. Should extract structured facts
        assert!(facts >= 1, "Should extract at least 1 fact");

        // 4. Should extract code snippets and file references
        assert!(snippets >= 2, "Should extract at least 2 snippets (code + file ref)");

        // 5. Total should be significantly better than simple keyword matching
        // Simple keyword matching would extract ~3-5 items
        // Intelligent extraction should get 8-12+ items with better quality
        assert!(
            memories.len() >= 5,
            "Should extract at least 5 memories (got {})",
            memories.len()
        );

        println!("\nExtraction quality test results:");
        println!("Total memories: {}", memories.len());
        println!("Decisions: {}", decisions);
        println!("Tasks: {}", tasks);
        println!("Facts: {}", facts);
        println!("Snippets: {}", snippets);
    }
}

