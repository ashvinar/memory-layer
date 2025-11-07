use anyhow::Result;
use memory_layer_schemas::{Memory, SourceApp, Turn};
use tracing::{debug, info};

use crate::database::Database;

/// Auto-organizer for memories into hierarchical structure
/// Workspace → Project → Area → Topic
pub struct MemoryOrganizer;

impl MemoryOrganizer {
    pub fn new() -> Self {
        Self
    }

    /// Organize a memory into the hierarchy based on its turn source
    /// Returns the memory with topic_id populated
    pub fn organize(&self, db: &Database, memory: &Memory, turn: &Turn) -> Result<Memory> {
        // Step 1: Workspace - cluster by app bundle ID
        let workspace_name = Self::infer_workspace_name(&turn.source.app);
        let workspace = db.get_or_create_workspace(&workspace_name, None)?;

        debug!(
            "Auto-organized memory {} to workspace: {}",
            memory.id, workspace_name
        );

        // Step 2: Project - infer from file path or URL
        let project_name = Self::infer_project_name(turn);
        let project = db.get_or_create_project(
            &workspace,
            &project_name,
            None,
            memory_layer_schemas::ProjectStatus::Active,
        )?;

        debug!(
            "Auto-organized memory {} to project: {}",
            memory.id, project_name
        );

        // Step 3: Area - cluster by memory kind
        let area_name = Self::infer_area_name(&memory.kind);
        let area = db.get_or_create_area(&project, &area_name, None)?;

        debug!(
            "Auto-organized memory {} to area: {}",
            memory.id, area_name
        );

        // Step 4: Topic - use extracted topic string
        let topic_name = if memory.topic.trim().is_empty() {
            "General".to_string()
        } else {
            memory.topic.clone()
        };
        let topic = db.get_or_create_topic(&area, &topic_name, None, false)?;

        debug!(
            "Auto-organized memory {} to topic: {}",
            memory.id, topic_name
        );

        info!(
            "Auto-organized memory {} into hierarchy: {} > {} > {} > {}",
            memory.id, workspace_name, project_name, area_name, topic_name
        );

        // Update memory with topic_id
        let mut organized_memory = memory.clone();
        organized_memory.topic_id = Some(topic);

        Ok(organized_memory)
    }

    /// Infer workspace name from app source
    fn infer_workspace_name(app: &SourceApp) -> String {
        match app {
            SourceApp::Claude => "Claude".to_string(),
            SourceApp::ChatGPT => "ChatGPT".to_string(),
            SourceApp::VSCode => "VSCode".to_string(),
            SourceApp::Mail => "Mail".to_string(),
            SourceApp::Notes => "Notes".to_string(),
            SourceApp::Terminal => "Terminal".to_string(),
            SourceApp::Other => "Other".to_string(),
        }
    }

    /// Infer project name from file path or URL
    fn infer_project_name(turn: &Turn) -> String {
        // Try to extract from file path first
        if let Some(ref path) = turn.source.path {
            // Look for common project root patterns
            // e.g., /Users/me/code/PROJECT_NAME/...
            let parts: Vec<&str> = path.split('/').collect();

            // Common paths: /Users/*/code/project, /home/*/code/project, ~/code/project
            for (i, part) in parts.iter().enumerate() {
                if *part == "code" || *part == "projects" || *part == "workspace" || *part == "work" {
                    if i + 1 < parts.len() && !parts[i + 1].is_empty() {
                        return parts[i + 1].to_string();
                    }
                }
            }

            // Fallback: use the first non-home directory
            if parts.len() >= 4 {
                // Skip /Users/username or /home/username
                return parts[3].to_string();
            }
        }

        // Try to extract from URL
        if let Some(ref url) = turn.source.url {
            // Extract domain or path component
            if let Some(domain_path) = url.split("://").nth(1) {
                let path_parts: Vec<&str> = domain_path.split('/').collect();
                if path_parts.len() > 1 && !path_parts[1].is_empty() {
                    // Use first path segment after domain
                    return path_parts[1].to_string();
                }
                // Fallback to domain
                if let Some(domain) = path_parts.first() {
                    if let Some(base) = domain.split('.').next() {
                        return base.to_string();
                    }
                }
            }
        }

        // Default fallback
        "Default".to_string()
    }

    /// Infer area name from memory kind
    fn infer_area_name(kind: &memory_layer_schemas::MemoryKind) -> String {
        match kind {
            memory_layer_schemas::MemoryKind::Decision => "Decisions".to_string(),
            memory_layer_schemas::MemoryKind::Fact => "Facts".to_string(),
            memory_layer_schemas::MemoryKind::Snippet => "Code".to_string(),
            memory_layer_schemas::MemoryKind::Task => "Tasks".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_layer_schemas::{
        generate_thread_id, generate_turn_id, MemoryKind, SourceApp, TurnSource,
    };

    #[test]
    fn test_workspace_inference() {
        assert_eq!(
            MemoryOrganizer::infer_workspace_name(&SourceApp::Claude),
            "Claude"
        );
        assert_eq!(
            MemoryOrganizer::infer_workspace_name(&SourceApp::VSCode),
            "VSCode"
        );
        assert_eq!(
            MemoryOrganizer::infer_workspace_name(&SourceApp::ChatGPT),
            "ChatGPT"
        );
    }

    #[test]
    fn test_project_inference_from_path() {
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: "2025-11-07T00:00:00Z".to_string(),
            user_text: "test".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::VSCode,
                url: None,
                path: Some("/Users/me/code/my-project/src/main.rs".to_string()),
            },
        };

        let project = MemoryOrganizer::infer_project_name(&turn);
        assert_eq!(project, "my-project");
    }

    #[test]
    fn test_project_inference_from_url() {
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: "2025-11-07T00:00:00Z".to_string(),
            user_text: "test".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::Claude,
                url: Some("https://github.com/user/repo".to_string()),
                path: None,
            },
        };

        let project = MemoryOrganizer::infer_project_name(&turn);
        assert_eq!(project, "user");
    }

    #[test]
    fn test_area_inference() {
        assert_eq!(
            MemoryOrganizer::infer_area_name(&MemoryKind::Decision),
            "Decisions"
        );
        assert_eq!(
            MemoryOrganizer::infer_area_name(&MemoryKind::Fact),
            "Facts"
        );
        assert_eq!(
            MemoryOrganizer::infer_area_name(&MemoryKind::Snippet),
            "Code"
        );
        assert_eq!(
            MemoryOrganizer::infer_area_name(&MemoryKind::Task),
            "Tasks"
        );
    }
}
