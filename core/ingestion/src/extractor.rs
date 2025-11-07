use anyhow::Result;
use chrono::Utc;
use memory_layer_schemas::{generate_memory_id, Memory, MemoryKind, Snippet, Turn};
use tracing::debug;

/// Memory extractor that identifies and extracts structured knowledge from turns
pub struct MemoryExtractor;

impl MemoryExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract memories from a turn
    pub fn extract(&self, turn: &Turn) -> Result<Vec<Memory>> {
        let mut memories = Vec::new();

        // Extract decisions (keywords: "decided", "will", "going to", "plan to")
        if let Some(decision) = self.extract_decision(turn) {
            memories.push(decision);
        }

        // Extract facts (declarative statements, definitions)
        if let Some(fact) = self.extract_fact(turn) {
            memories.push(fact);
        }

        // Extract tasks (keywords: "TODO", "need to", "should", "must")
        if let Some(task) = self.extract_task(turn) {
            memories.push(task);
        }

        // Extract code snippets (detect code blocks, file paths, line numbers)
        memories.extend(self.extract_snippets(turn)?);

        debug!(
            "Extracted {} memories from turn {}",
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
            }
        }

        None
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
                }
            }
        }

        // Detect file paths and line references (e.g., "src/main.rs:42-56")
        if let Some(snippet) = self.extract_file_reference(turn) {
            snippets.push(snippet);
        }

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

        None
    }

    fn extract_sentence_with(&self, text: &str, keyword: &str) -> Option<String> {
        // Find the sentence containing the keyword
        let sentences: Vec<&str> = text.split(|c| c == '.' || c == '!' || c == '?').collect();

        for sentence in sentences {
            if sentence.contains(keyword) {
                return Some(sentence.trim().to_string());
            }
        }

        None
    }

    fn infer_topic(&self, turn: &Turn) -> String {
        // Simple topic inference from source
        if let Some(ref path) = turn.source.path {
            // Extract directory or file name as topic
            if let Some(last) = path.split('/').last() {
                return last.to_string();
            }
        }

        if let Some(ref url) = turn.source.url {
            // Extract domain as topic
            if let Some(domain) = url.split("://").nth(1) {
                if let Some(base) = domain.split('/').next() {
                    return base.to_string();
                }
            }
        }

        // Default to app name
        format!("{:?}", turn.source.app)
    }

    fn extract_entities(&self, turn: &Turn) -> Vec<String> {
        let mut entities = Vec::new();
        let text = &turn.user_text;

        // Extract capitalized words (potential names)
        for word in text.split_whitespace() {
            if word.len() > 2 && word.chars().next().unwrap().is_uppercase() {
                let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
                if !clean.is_empty() {
                    entities.push(clean.to_string());
                }
            }
        }

        // Add source-specific entities
        if let Some(ref path) = turn.source.path {
            entities.push(path.clone());
        }

        entities.sort();
        entities.dedup();
        entities
    }

    fn detect_language(filename: &str) -> Option<String> {
        let ext = filename.split('.').last()?;
        let lang = match ext {
            "rs" => "rust",
            "py" => "python",
            "js" => "javascript",
            "ts" => "typescript",
            "go" => "go",
            "java" => "java",
            "swift" => "swift",
            "cpp" | "cc" | "cxx" => "cpp",
            "c" => "c",
            "h" => "c",
            "hpp" => "cpp",
            "sh" => "bash",
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            "md" => "markdown",
            _ => return None,
        };
        Some(lang.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_layer_schemas::{generate_thread_id, generate_turn_id, SourceApp, TurnSource};

    #[test]
    fn test_decision_extraction() {
        let extractor = MemoryExtractor::new();
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
    fn test_task_extraction() {
        let extractor = MemoryExtractor::new();
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "TODO: need to fix the bug in authentication".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::VSCode,
                url: None,
                path: Some("src/auth.rs".to_string()),
            },
        };

        let memories = extractor.extract(&turn).unwrap();
        assert!(memories.iter().any(|m| matches!(m.kind, MemoryKind::Task)));
    }

    #[test]
    fn test_snippet_extraction() {
        let extractor = MemoryExtractor::new();
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "Here's the code:\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```"
                .to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::Claude,
                url: None,
                path: None,
            },
        };

        let memories = extractor.extract(&turn).unwrap();
        assert!(memories
            .iter()
            .any(|m| matches!(m.kind, MemoryKind::Snippet)));
    }

    #[test]
    fn test_file_reference_extraction() {
        let extractor = MemoryExtractor::new();
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "Check src/main.rs:42-56 for the implementation".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::VSCode,
                url: None,
                path: None,
            },
        };

        let memories = extractor.extract(&turn).unwrap();
        assert!(memories.iter().any(|m| {
            matches!(m.kind, MemoryKind::Snippet)
                && m.snippet.as_ref().and_then(|s| s.loc.as_ref()).is_some()
        }));
    }
}
