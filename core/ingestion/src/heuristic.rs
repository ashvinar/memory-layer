use anyhow::Result;
use chrono::Utc;
use memory_layer_schemas::{generate_memory_id, Memory, MemoryKind, Snippet, Turn};
use regex::Regex;
use tracing::debug;

/// Confidence score for extracted memories (0.0 to 1.0)
#[derive(Debug, Clone, Copy)]
pub struct Confidence(f32);

impl Confidence {
    pub fn new(score: f32) -> Self {
        Self(score.clamp(0.0, 1.0))
    }

    pub fn score(&self) -> f32 {
        self.0
    }

    pub fn is_confident(&self) -> bool {
        self.0 >= 0.7
    }
}

/// Extracted memory with confidence score
#[derive(Debug, Clone)]
pub struct ExtractedMemory {
    pub memory: Memory,
    pub confidence: Confidence,
}

/// Improved heuristic-based memory extractor
pub struct HeuristicExtractor {
    decision_patterns: Vec<Regex>,
    task_patterns: Vec<Regex>,
    fact_patterns: Vec<Regex>,
    code_ref_patterns: Vec<Regex>,
    priority_patterns: Vec<Regex>,
}

impl Default for HeuristicExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl HeuristicExtractor {
    pub fn new() -> Self {
        Self {
            decision_patterns: vec![
                // Strong decision indicators
                Regex::new(r"(?i)\b(decided|chose|selected|picked|opted)\s+to\b").unwrap(),
                Regex::new(r"(?i)\b(will|going to|plan to|planning to)\s+(?:use|adopt|implement|switch to|move to)\b").unwrap(),
                Regex::new(r"(?i)\b(switching|moving|migrating|adopting)\s+(?:from|to)\b").unwrap(),
                // Decision with reasoning
                Regex::new(r"(?i)\b(because|since|as|reason|rationale|why).*\b(decided|chose|using|adopted)\b").unwrap(),
            ],
            task_patterns: vec![
                // Explicit task markers
                Regex::new(r"(?i)\b(TODO|FIXME|HACK|XXX|NOTE)\s*[:\-]?\s*(.+)").unwrap(),
                Regex::new(r"(?i)\b(need to|must|should|have to|got to)\s+([a-z].+)").unwrap(),
                Regex::new(r"(?i)\b(remember to|don't forget to|action item)\s*[:\-]?\s*(.+)").unwrap(),
                // Task with context
                Regex::new(r"(?i)\b(next step|next up|upcoming).*[:\-]\s*(.+)").unwrap(),
            ],
            fact_patterns: vec![
                // Definitions
                Regex::new(r"(?i)(\w+)\s+(?:is|means|refers to|defined as)\s+(.+)").unwrap(),
                // Key-value facts
                Regex::new(r"(?i)(\w+)\s*[=:]\s*([^\n]+)").unwrap(),
                // Technical facts
                Regex::new(r"(?i)\b(uses|requires|depends on|built with)\s+(.+)").unwrap(),
            ],
            code_ref_patterns: vec![
                // File with line numbers
                Regex::new(r"([a-zA-Z0-9_\-./]+\.[a-z]+):(\d+)(?:-(\d+))?").unwrap(),
                // Function/method references
                Regex::new(r"(?:function|method|class)\s+([a-zA-Z0-9_]+)").unwrap(),
                // Code identifiers with context
                Regex::new(r"`([a-zA-Z0-9_:.<>]+(?:\([^)]*\))?)`").unwrap(),
            ],
            priority_patterns: vec![
                Regex::new(r"(?i)\b(urgent|critical|asap|high priority|immediately)\b").unwrap(),
                Regex::new(r"(?i)\b(blocking|blocker|broken|bug)\b").unwrap(),
            ],
        }
    }

    /// Extract all memories with confidence scores
    pub fn extract(&self, turn: &Turn) -> Result<Vec<ExtractedMemory>> {
        let mut extracted = Vec::new();

        // Fast path: obvious patterns (code blocks, file refs)
        extracted.extend(self.extract_code_blocks(turn)?);
        extracted.extend(self.extract_file_references(turn)?);

        // Smart heuristics: detect decisions, tasks, facts with context
        if let Some(decision) = self.extract_decision_with_context(turn)? {
            extracted.push(decision);
        }

        extracted.extend(self.extract_tasks_with_priority(turn)?);
        extracted.extend(self.extract_facts_with_structure(turn)?);

        debug!(
            "Heuristic extractor found {} memories from turn {}",
            extracted.len(),
            turn.id
        );

        Ok(extracted)
    }

    /// Extract decisions with reasoning and entities
    fn extract_decision_with_context(&self, turn: &Turn) -> Result<Option<ExtractedMemory>> {
        let text = &turn.user_text;
        let lower = text.to_lowercase();

        for pattern in &self.decision_patterns {
            if let Some(caps) = pattern.captures(&lower) {
                // Extract the full context around the decision
                let match_pos = caps.get(0).unwrap().start();
                let context = self.extract_context_around(text, match_pos, 200);

                // Calculate confidence based on multiple factors
                let mut confidence = 0.7; // Base confidence

                // Boost if reasoning is present
                if lower.contains("because")
                    || lower.contains("since")
                    || lower.contains("reason")
                {
                    confidence += 0.15;
                }

                // Boost if entities are mentioned
                let entities = self.extract_entities_smart(text);
                if !entities.is_empty() {
                    confidence += 0.1;
                }

                // Boost if technical terms present
                if self.has_technical_terms(&lower) {
                    confidence += 0.05;
                }

                return Ok(Some(ExtractedMemory {
                    memory: Memory {
                        id: generate_memory_id(),
                        kind: MemoryKind::Decision,
                        topic: self.infer_topic_smart(turn),
                        text: context,
                        snippet: None,
                        entities,
                        provenance: vec![turn.id.clone()],
                        created_at: Utc::now().to_rfc3339(),
                        ttl: None,
                    },
                    confidence: Confidence::new(confidence),
                }));
            }
        }

        Ok(None)
    }

    /// Extract tasks with priority and context
    fn extract_tasks_with_priority(&self, turn: &Turn) -> Result<Vec<ExtractedMemory>> {
        let mut tasks = Vec::new();
        let text = &turn.user_text;
        let lower = text.to_lowercase();

        for pattern in &self.task_patterns {
            for caps in pattern.captures_iter(&lower) {
                let task_text = if caps.len() > 2 {
                    caps.get(2).map(|m| m.as_str()).unwrap_or("")
                } else {
                    caps.get(0).map(|m| m.as_str()).unwrap_or("")
                };

                if task_text.trim().is_empty() {
                    continue;
                }

                // Extract full context
                let match_pos = caps.get(0).unwrap().start();
                let context = self.extract_context_around(text, match_pos, 150);

                // Determine priority and confidence
                let is_high_priority = self.is_high_priority(&context);
                let mut confidence = if caps.get(1).map(|m| m.as_str()).unwrap_or("").to_uppercase() == "TODO" {
                    0.9 // Explicit TODO markers are very confident
                } else {
                    0.75
                };

                if is_high_priority {
                    confidence += 0.1;
                }

                // Set TTL based on priority
                let ttl = if is_high_priority {
                    Some(86400 * 2) // 2 days for urgent
                } else {
                    Some(86400 * 7) // 7 days for normal
                };

                let entities = self.extract_entities_smart(&context);
                tasks.push(ExtractedMemory {
                    memory: Memory {
                        id: generate_memory_id(),
                        kind: MemoryKind::Task,
                        topic: self.infer_topic_smart(turn),
                        text: context,
                        snippet: None,
                        entities,
                        provenance: vec![turn.id.clone()],
                        created_at: Utc::now().to_rfc3339(),
                        ttl,
                    },
                    confidence: Confidence::new(confidence),
                });
            }
        }

        Ok(tasks)
    }

    /// Extract facts with structured key-value pairs
    fn extract_facts_with_structure(&self, turn: &Turn) -> Result<Vec<ExtractedMemory>> {
        let mut facts = Vec::new();
        let text = &turn.user_text;

        for pattern in &self.fact_patterns {
            for caps in pattern.captures_iter(text) {
                if caps.len() >= 2 {
                    let key = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    let value = caps.get(2).map(|m| m.as_str()).unwrap_or("");

                    if key.trim().is_empty() || value.trim().is_empty() {
                        continue;
                    }

                    // Filter out common false positives
                    if self.is_likely_false_positive(key, value) {
                        continue;
                    }

                    let fact_text = format!("{}: {}", key.trim(), value.trim());
                    let mut confidence = 0.6;

                    // Boost confidence for technical facts
                    if self.has_technical_terms(&fact_text.to_lowercase()) {
                        confidence += 0.2;
                    }

                    // Boost for explicit definitions
                    if text.contains("defined as") || text.contains("means") {
                        confidence += 0.15;
                    }

                    facts.push(ExtractedMemory {
                        memory: Memory {
                            id: generate_memory_id(),
                            kind: MemoryKind::Fact,
                            topic: self.infer_topic_smart(turn),
                            text: fact_text,
                            snippet: None,
                            entities: vec![key.trim().to_string()],
                            provenance: vec![turn.id.clone()],
                            created_at: Utc::now().to_rfc3339(),
                            ttl: None,
                        },
                        confidence: Confidence::new(confidence),
                    });
                }
            }
        }

        Ok(facts)
    }

    /// Extract code blocks with language detection
    fn extract_code_blocks(&self, turn: &Turn) -> Result<Vec<ExtractedMemory>> {
        let mut snippets = Vec::new();
        let text = &turn.user_text;

        if !text.contains("```") {
            return Ok(snippets);
        }

        let blocks: Vec<&str> = text.split("```").collect();
        for (i, block) in blocks.iter().enumerate() {
            if i % 2 == 1 {
                // Odd indices are code blocks
                let lines: Vec<&str> = block.lines().collect();
                let language = if !lines.is_empty() {
                    let first = lines[0].trim();
                    if !first.is_empty() && first.chars().all(|c| c.is_alphanumeric()) {
                        Some(first.to_string())
                    } else {
                        None
                    }
                } else {
                    None
                };

                let code = if !lines.is_empty() && language.is_some() {
                    lines[1..].join("\n")
                } else {
                    block.to_string()
                };

                snippets.push(ExtractedMemory {
                    memory: Memory {
                        id: generate_memory_id(),
                        kind: MemoryKind::Snippet,
                        topic: self.infer_topic_smart(turn),
                        text: "Code snippet".to_string(),
                        snippet: Some(Snippet {
                            title: format!("Snippet from {:?}", turn.source.app),
                            text: code,
                            loc: None,
                            language,
                        }),
                        entities: self.extract_entities_smart(text),
                        provenance: vec![turn.id.clone()],
                        created_at: Utc::now().to_rfc3339(),
                        ttl: None,
                    },
                    confidence: Confidence::new(0.95), // Code blocks are very clear
                });
            }
        }

        Ok(snippets)
    }

    /// Extract file references with line numbers
    fn extract_file_references(&self, turn: &Turn) -> Result<Vec<ExtractedMemory>> {
        let mut snippets = Vec::new();
        let text = &turn.user_text;

        // File path with line numbers
        if let Some(pattern) = self.code_ref_patterns.first() {
            for caps in pattern.captures_iter(text) {
                let file = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let start_line = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                let end_line = caps.get(3).map(|m| m.as_str());

                let loc = if let Some(end) = end_line {
                    format!("L{}-L{}", start_line, end)
                } else {
                    format!("L{}", start_line)
                };

                let context = self.extract_context_around(text, caps.get(0).unwrap().start(), 100);

                snippets.push(ExtractedMemory {
                    memory: Memory {
                        id: generate_memory_id(),
                        kind: MemoryKind::Snippet,
                        topic: self.infer_topic_smart(turn),
                        text: format!("Reference to {}", file),
                        snippet: Some(Snippet {
                            title: file.to_string(),
                            text: context,
                            loc: Some(loc),
                            language: Self::detect_language(file),
                        }),
                        entities: vec![file.to_string()],
                        provenance: vec![turn.id.clone()],
                        created_at: Utc::now().to_rfc3339(),
                        ttl: None,
                    },
                    confidence: Confidence::new(0.9),
                });
            }
        }

        Ok(snippets)
    }

    // Helper methods

    fn extract_context_around(&self, text: &str, position: usize, radius: usize) -> String {
        let start = position.saturating_sub(radius);
        let end = (position + radius).min(text.len());

        // Find sentence boundaries
        let before = &text[start..position];
        let after = &text[position..end];

        let sentence_start = before.rfind(|c| c == '.' || c == '!' || c == '?')
            .map(|p| start + p + 1)
            .unwrap_or(start);

        let sentence_end = after.find(|c| c == '.' || c == '!' || c == '?')
            .map(|p| position + p + 1)
            .unwrap_or(end);

        text[sentence_start..sentence_end].trim().to_string()
    }

    fn extract_entities_smart(&self, text: &str) -> Vec<String> {
        let mut entities = Vec::new();

        // Extract capitalized words (potential names)
        for word in text.split_whitespace() {
            if word.len() > 2 && word.chars().next().unwrap().is_uppercase() {
                let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
                if !clean.is_empty() && !self.is_common_word(clean) {
                    entities.push(clean.to_string());
                }
            }
        }

        // Extract technical identifiers in backticks
        for pattern in &self.code_ref_patterns[2..] {
            for caps in pattern.captures_iter(text) {
                if let Some(m) = caps.get(1) {
                    entities.push(m.as_str().to_string());
                }
            }
        }

        entities.sort();
        entities.dedup();
        entities
    }

    fn infer_topic_smart(&self, turn: &Turn) -> String {
        // Try path first
        if let Some(ref path) = turn.source.path {
            if let Some(last) = path.split('/').last() {
                return last.to_string();
            }
        }

        // Try URL
        if let Some(ref url) = turn.source.url {
            if let Some(domain) = url.split("://").nth(1) {
                if let Some(base) = domain.split('/').next() {
                    return base.to_string();
                }
            }
        }

        // Extract topic from text using technical terms
        let lower = turn.user_text.to_lowercase();
        let tech_terms = [
            "rust", "python", "javascript", "typescript", "go", "java",
            "api", "database", "auth", "frontend", "backend", "testing",
            "deployment", "docker", "kubernetes", "ci/cd",
        ];

        for term in &tech_terms {
            if lower.contains(term) {
                return term.to_string();
            }
        }

        format!("{:?}", turn.source.app)
    }

    fn is_high_priority(&self, text: &str) -> bool {
        let lower = text.to_lowercase();
        self.priority_patterns
            .iter()
            .any(|p| p.is_match(&lower))
    }

    fn has_technical_terms(&self, text: &str) -> bool {
        let tech_indicators = [
            "api", "function", "class", "method", "library", "framework",
            "database", "sql", "endpoint", "service", "module", "package",
            "interface", "struct", "type", "trait", "impl", "enum",
        ];

        tech_indicators.iter().any(|term| text.contains(term))
    }

    fn is_common_word(&self, word: &str) -> bool {
        let common = [
            "The", "This", "That", "These", "Those", "Here", "There",
            "When", "Where", "What", "Which", "How", "Why", "Who",
            "But", "And", "Or", "Not", "If", "Then", "So",
        ];

        common.contains(&word)
    }

    fn is_likely_false_positive(&self, key: &str, value: &str) -> bool {
        // Filter out common sentence patterns that aren't facts
        let lower_key = key.to_lowercase();
        let lower_value = value.to_lowercase();

        // Too short
        if key.len() < 2 || value.len() < 3 {
            return true;
        }

        // Common false positives
        if lower_key.starts_with("this")
            || lower_key.starts_with("that")
            || lower_key.starts_with("it")
        {
            return true;
        }

        // Questions
        if lower_value.contains('?') {
            return true;
        }

        false
    }

    fn detect_language(filename: &str) -> Option<String> {
        let ext = filename.split('.').last()?;
        let lang = match ext {
            "rs" => "rust",
            "py" => "python",
            "js" => "javascript",
            "ts" => "typescript",
            "tsx" => "typescript",
            "jsx" => "javascript",
            "go" => "go",
            "java" => "java",
            "swift" => "swift",
            "kt" => "kotlin",
            "cpp" | "cc" | "cxx" => "cpp",
            "c" => "c",
            "h" => "c",
            "hpp" => "cpp",
            "sh" => "bash",
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            "md" => "markdown",
            "html" => "html",
            "css" => "css",
            "sql" => "sql",
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
    fn test_decision_with_reasoning() {
        let extractor = HeuristicExtractor::new();
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "I decided to use Rust because it's fast and safe. We'll adopt it for the new service.".to_string(),
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
        let decision = memories.iter().find(|m| matches!(m.memory.kind, MemoryKind::Decision));
        assert!(decision.is_some());
        assert!(decision.unwrap().confidence.is_confident());
    }

    #[test]
    fn test_high_priority_task() {
        let extractor = HeuristicExtractor::new();
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "URGENT: need to fix the critical bug in auth service".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::VSCode,
                url: None,
                path: Some("src/auth.rs".to_string()),
            },
        };

        let memories = extractor.extract(&turn).unwrap();
        let task = memories.iter().find(|m| matches!(m.memory.kind, MemoryKind::Task));
        assert!(task.is_some());
        let task_mem = task.unwrap();
        assert!(task_mem.confidence.is_confident());
        // High priority tasks should have shorter TTL
        assert!(task_mem.memory.ttl.unwrap() < 86400 * 5);
    }

    #[test]
    fn test_fact_extraction() {
        let extractor = HeuristicExtractor::new();
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "API endpoint: /api/v1/users\nDatabase: PostgreSQL".to_string(),
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
            .filter(|m| matches!(m.memory.kind, MemoryKind::Fact))
            .collect();
        assert!(!facts.is_empty());
    }

    #[test]
    fn test_code_reference_with_lines() {
        let extractor = HeuristicExtractor::new();
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "Check the implementation in src/main.rs:42-56".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::VSCode,
                url: None,
                path: None,
            },
        };

        let memories = extractor.extract(&turn).unwrap();
        let snippet = memories.iter().find(|m| matches!(m.memory.kind, MemoryKind::Snippet));
        assert!(snippet.is_some());
        let snippet_mem = snippet.unwrap();
        assert!(snippet_mem.memory.snippet.as_ref().unwrap().loc.is_some());
    }

    #[test]
    fn test_confidence_scoring() {
        let extractor = HeuristicExtractor::new();

        // High confidence: explicit TODO
        let turn1 = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "TODO: implement authentication".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::VSCode,
                url: None,
                path: None,
            },
        };

        let memories1 = extractor.extract(&turn1).unwrap();
        assert!(memories1[0].confidence.score() >= 0.8);

        // Lower confidence: ambiguous statement
        let turn2 = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "This is something".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::Claude,
                url: None,
                path: None,
            },
        };

        let memories2 = extractor.extract(&turn2).unwrap();
        // Should extract very few or no memories from ambiguous text
        assert!(memories2.len() < memories1.len());
    }
}
