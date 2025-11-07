use memory_layer_schemas::{ContextStyle, Memory, MemoryKind};

/// Template renderer for Context Capsules
pub struct TemplateRenderer;

impl TemplateRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Render context in the specified style
    pub fn render(
        &self,
        style: &ContextStyle,
        topic: &str,
        memories: &[Memory],
        token_budget: u64,
    ) -> String {
        match style {
            ContextStyle::Short => self.render_short(topic, memories, token_budget),
            ContextStyle::Standard => self.render_standard(topic, memories, token_budget),
            ContextStyle::Detailed => self.render_detailed(topic, memories, token_budget),
        }
    }

    fn render_short(&self, topic: &str, memories: &[Memory], _token_budget: u64) -> String {
        // Target: 1-2 lines, minimal tokens (~50-100 tokens)
        let mut parts = vec![format!("Context: {}", topic)];

        // Add 1-2 most recent gists
        let recent: Vec<String> = memories
            .iter()
            .take(2)
            .map(|m| self.gist_from_memory(m))
            .collect();

        if !recent.is_empty() {
            parts.push(format!("Last: {}", recent.join(", ")));
        }

        // Add one tiny snippet if available
        if let Some(snippet_memory) = memories.iter().find(|m| m.snippet.is_some()) {
            if let Some(ref snippet) = snippet_memory.snippet {
                let short_text = snippet.text.lines().next().unwrap_or("");
                if short_text.len() < 80 {
                    parts.push(format!("\"{}\"", short_text));
                }
            }
        }

        parts.join(". ")
    }

    fn render_standard(&self, topic: &str, memories: &[Memory], _token_budget: u64) -> String {
        // Target: ~220 tokens
        let mut lines = vec![
            "Context (continue without re-explaining):".to_string(),
            format!("• Project/Topic: {}", topic),
        ];

        // Add recent memories (1-2 gists with times)
        let recent: Vec<String> = memories
            .iter()
            .take(2)
            .map(|m| {
                let time = self.format_relative_time(&m.created_at);
                format!("{} ({})", self.gist_from_memory(m), time)
            })
            .collect();

        if !recent.is_empty() {
            lines.push(format!("• Recent: {}", recent.join("; ")));
        }

        // Add one snippet if available
        if let Some(snippet_memory) = memories.iter().find(|m| m.snippet.is_some()) {
            if let Some(ref snippet) = snippet_memory.snippet {
                let snippet_text = snippet.text.lines().take(3).collect::<Vec<_>>().join("\n");
                lines.push(format!("• Snip: \"{}\"", snippet_text));
                if let Some(ref loc) = snippet.loc {
                    lines.push(format!("  ({} {})", snippet.title, loc));
                }
            }
        }

        lines.push("Instructions: Use the context to answer. Don't restate it. Ask one concise follow-up if a key detail is missing.".to_string());

        lines.join("\n")
    }

    fn render_detailed(&self, topic: &str, memories: &[Memory], _token_budget: u64) -> String {
        // Target: More detailed, up to 500 tokens
        let mut lines = vec![
            "# Context Summary".to_string(),
            format!("\n## Project: {}", topic),
            "\n## Recent Activities:".to_string(),
        ];

        // Group memories by kind
        let mut decisions = Vec::new();
        let mut facts = Vec::new();
        let mut tasks = Vec::new();
        let mut snippets = Vec::new();

        for memory in memories {
            match memory.kind {
                MemoryKind::Decision => decisions.push(memory),
                MemoryKind::Fact => facts.push(memory),
                MemoryKind::Task => tasks.push(memory),
                MemoryKind::Snippet => snippets.push(memory),
            }
        }

        if !decisions.is_empty() {
            lines.push("\n### Decisions:".to_string());
            for decision in decisions.iter().take(3) {
                lines.push(format!("- {}", decision.text));
            }
        }

        if !facts.is_empty() {
            lines.push("\n### Facts:".to_string());
            for fact in facts.iter().take(5) {
                lines.push(format!("- {}", fact.text));
            }
        }

        if !tasks.is_empty() {
            lines.push("\n### Tasks:".to_string());
            for task in tasks.iter().take(3) {
                lines.push(format!("- {}", task.text));
            }
        }

        if !snippets.is_empty() {
            lines.push("\n### Code Snippets:".to_string());
            for snippet_mem in snippets.iter().take(2) {
                if let Some(ref snippet) = snippet_mem.snippet {
                    lines.push(format!("\n#### {}", snippet.title));
                    if let Some(ref loc) = snippet.loc {
                        lines.push(format!("Location: {}", loc));
                    }
                    lines.push(format!("```\n{}\n```", snippet.text));
                }
            }
        }

        lines.push("\n## Instructions:".to_string());
        lines.push("Use this context to inform your responses. Reference specific details when relevant. If critical information is missing, ask clarifying questions before proceeding.".to_string());

        lines.join("\n")
    }

    fn gist_from_memory(&self, memory: &Memory) -> String {
        // Create a concise gist from memory
        let text = &memory.text;
        if text.len() <= 50 {
            text.clone()
        } else {
            let first_sentence: String = text
                .split('.')
                .next()
                .unwrap_or(text)
                .chars()
                .take(50)
                .collect();
            format!("{}...", first_sentence.trim())
        }
    }

    fn format_relative_time(&self, rfc3339: &str) -> String {
        use chrono::{DateTime, Utc};

        if let Ok(dt) = DateTime::parse_from_rfc3339(rfc3339) {
            let now = Utc::now();
            let duration = now.signed_duration_since(dt);

            if duration.num_hours() < 1 {
                format!("{}m ago", duration.num_minutes())
            } else if duration.num_days() < 1 {
                format!("{}h ago", duration.num_hours())
            } else if duration.num_days() < 7 {
                format!("{}d ago", duration.num_days())
            } else {
                format!("{}w ago", duration.num_weeks())
            }
        } else {
            "recently".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use memory_layer_schemas::{generate_memory_id, generate_turn_id};

    #[test]
    fn test_short_template() {
        let renderer = TemplateRenderer::new();
        let memories = vec![Memory {
            id: generate_memory_id(),
            kind: MemoryKind::Fact,
            topic: "test".to_string(),
            text: "This is a test fact".to_string(),
            snippet: None,
            entities: vec![],
            provenance: vec![generate_turn_id()],
            created_at: Utc::now().to_rfc3339(),
            ttl: None,
        }];

        let result = renderer.render(&ContextStyle::Short, "TestProject", &memories, 100);
        assert!(result.contains("Context: TestProject"));
        assert!(result.len() < 200);
    }

    #[test]
    fn test_standard_template() {
        let renderer = TemplateRenderer::new();
        let memories = vec![Memory {
            id: generate_memory_id(),
            kind: MemoryKind::Decision,
            topic: "test".to_string(),
            text: "Decided to use Rust".to_string(),
            snippet: None,
            entities: vec![],
            provenance: vec![generate_turn_id()],
            created_at: Utc::now().to_rfc3339(),
            ttl: None,
        }];

        let result = renderer.render(&ContextStyle::Standard, "TestProject", &memories, 220);
        assert!(result.contains("Context (continue without re-explaining)"));
        assert!(result.contains("TestProject"));
    }

    #[test]
    fn test_detailed_template() {
        let renderer = TemplateRenderer::new();
        let memories = vec![
            Memory {
                id: generate_memory_id(),
                kind: MemoryKind::Decision,
                topic: "test".to_string(),
                text: "Decided to use Rust".to_string(),
                snippet: None,
                entities: vec![],
                provenance: vec![generate_turn_id()],
                created_at: Utc::now().to_rfc3339(),
                ttl: None,
            },
            Memory {
                id: generate_memory_id(),
                kind: MemoryKind::Task,
                topic: "test".to_string(),
                text: "Need to write tests".to_string(),
                snippet: None,
                entities: vec![],
                provenance: vec![generate_turn_id()],
                created_at: Utc::now().to_rfc3339(),
                ttl: Some(86400),
            },
        ];

        let result = renderer.render(&ContextStyle::Detailed, "TestProject", &memories, 500);
        assert!(result.contains("# Context Summary"));
        assert!(result.contains("TestProject"));
        assert!(result.contains("Decisions:"));
        assert!(result.contains("Tasks:"));
    }
}
