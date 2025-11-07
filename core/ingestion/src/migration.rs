use anyhow::{Context, Result};
use memory_layer_schemas::ProjectStatus;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::database::Database;

/// Suggested hierarchy from LLM for a flat topic string
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchySuggestion {
    #[serde(default = "default_workspace")]
    pub workspace: String,
    #[serde(default = "default_project")]
    pub project: String,
    #[serde(default = "default_area")]
    pub area: String,
    #[serde(default = "default_topic")]
    pub topic: String,
    pub rationale: Option<String>,
}

fn default_workspace() -> String {
    "General".to_string()
}

fn default_project() -> String {
    "Miscellaneous".to_string()
}

fn default_area() -> String {
    "General".to_string()
}

fn default_topic() -> String {
    "Uncategorized".to_string()
}

/// Migration statistics
#[derive(Debug, Clone, Default)]
pub struct MigrationStats {
    pub total_topics: usize,
    pub total_memories: usize,
    pub workspaces_created: usize,
    pub projects_created: usize,
    pub areas_created: usize,
    pub topics_created: usize,
    pub memories_updated: usize,
}

/// Migrate flat topic-based memories to hierarchical structure
pub async fn migrate_flat_to_hierarchical(
    db: &Database,
    use_llm: bool,
) -> Result<MigrationStats> {
    let mut stats = MigrationStats::default();

    info!("Starting migration from flat topics to hierarchical structure");

    // 1. Get all distinct topics from existing memories
    let topics = db.get_all_distinct_topics()?;
    stats.total_topics = topics.len();

    info!("Found {} distinct topics to migrate", stats.total_topics);

    // 2. For each topic, determine hierarchy and migrate memories
    for (topic, memory_count) in topics {
        info!("Migrating topic '{}' ({} memories)", topic, memory_count);

        stats.total_memories += memory_count;

        // 3. Suggest hierarchy (using LLM if available, otherwise heuristic)
        let suggestion = if use_llm {
            suggest_hierarchy_llm(&topic).await?
        } else {
            suggest_hierarchy_heuristic(&topic)
        };

        info!(
            "  → Workspace: '{}', Project: '{}', Area: '{}', Topic: '{}'",
            suggestion.workspace, suggestion.project, suggestion.area, suggestion.topic
        );

        // 4. Create or get hierarchy IDs
        let workspace_id = db.get_or_create_workspace(&suggestion.workspace, None)?;
        if workspace_id.0.starts_with("ws_") {
            stats.workspaces_created += 1;
        }

        let project_id = db.get_or_create_project(
            &workspace_id,
            &suggestion.project,
            None,
            ProjectStatus::Active,
        )?;
        if project_id.0.starts_with("proj_") {
            stats.projects_created += 1;
        }

        let area_id = db.get_or_create_area(&project_id, &suggestion.area, None)?;
        if area_id.0.starts_with("area_") {
            stats.areas_created += 1;
        }

        let topic_id = db.get_or_create_topic(&area_id, &suggestion.topic, None, false)?;
        if topic_id.0.starts_with("topic_") {
            stats.topics_created += 1;
        }

        // 5. Update all memories with this old topic string to point to new topic_id
        let updated = db.update_memories_topic(&topic, &topic_id)?;
        stats.memories_updated += updated;

        info!("  ✓ Migrated {} memories to new hierarchy", updated);
    }

    // 6. Generate index notes for all topics
    info!("Generating index notes for all topics...");
    let all_topics = db.get_all_topics()?;
    for (topic_id, topic_name, area_name) in all_topics {
        let memory_count = db.count_memories_by_topic_id(&topic_id)?;
        if memory_count > 0 {
            db.generate_or_update_index_note(&topic_id, &topic_name, &area_name, memory_count)?;
        }
    }

    info!("Migration complete: {:?}", stats);
    Ok(stats)
}

/// Suggest hierarchy using LLM (Ollama)
async fn suggest_hierarchy_llm(flat_topic: &str) -> Result<HierarchySuggestion> {
    // Build prompt for LLM
    let prompt = format!(
        r#"You are organizing a personal knowledge management system with a 4-level hierarchy:
Workspace → Project → Area → Topic

Given this flat topic string: "{}"

Suggest a hierarchical organization:
- Workspace: High-level category (e.g., "Work", "Personal", "Learning")
- Project: Specific initiative or effort (e.g., "Memory Layer", "WaterBuddy App")
- Area: Domain or component (e.g., "Backend", "iOS Development", "Health Tracking")
- Topic: Specific concept (e.g., "Database Schema", "SwiftUI Views", "Hydration Model")

Respond with ONLY a JSON object in this exact format:
{{
  "workspace": "...",
  "project": "...",
  "area": "...",
  "topic": "...",
  "rationale": "..."
}}
"#,
        flat_topic
    );

    // Try to call Ollama API
    let ollama_host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2:3b".to_string());

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/generate", ollama_host))
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
            "format": "json"
        }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await;

    match response {
        Ok(resp) => {
            // Try to parse the response, fall back to heuristic on any error
            match parse_ollama_response(resp, flat_topic).await {
                Ok(suggestion) => Ok(suggestion),
                Err(e) => {
                    warn!("Failed to parse LLM response: {}, falling back to heuristic", e);
                    Ok(suggest_hierarchy_heuristic(flat_topic))
                }
            }
        }
        Err(e) => {
            warn!("LLM call failed: {}, falling back to heuristic", e);
            Ok(suggest_hierarchy_heuristic(flat_topic))
        }
    }
}

async fn parse_ollama_response(
    resp: reqwest::Response,
    flat_topic: &str,
) -> Result<HierarchySuggestion> {
    let json: serde_json::Value = resp.json().await?;
    let response_text = json["response"]
        .as_str()
        .context("No response field in Ollama response")?;

    // Try to parse as JSON
    match serde_json::from_str::<HierarchySuggestion>(response_text) {
        Ok(suggestion) => Ok(suggestion),
        Err(_) => {
            // If JSON parsing fails, try to manually extract fields with defaults
            if let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(response_text) {
                // Replace null values with defaults
                if json_value["workspace"].is_null() {
                    json_value["workspace"] = serde_json::Value::String("General".to_string());
                }
                if json_value["project"].is_null() {
                    json_value["project"] = serde_json::Value::String("Miscellaneous".to_string());
                }
                if json_value["area"].is_null() {
                    json_value["area"] = serde_json::Value::String("General".to_string());
                }
                if json_value["topic"].is_null() {
                    json_value["topic"] = serde_json::Value::String(flat_topic.to_string());
                }

                // Try to parse again with fixed values
                serde_json::from_value(json_value)
                    .context("Failed to parse even after fixing null values")
            } else {
                anyhow::bail!("Failed to parse response as JSON")
            }
        }
    }
}

/// Fallback heuristic-based hierarchy suggestion
fn suggest_hierarchy_heuristic(flat_topic: &str) -> HierarchySuggestion {
    let topic_lower = flat_topic.to_lowercase();

    // Heuristics based on common patterns
    let workspace = if topic_lower.contains("work")
        || topic_lower.contains("project")
        || topic_lower.contains("code")
        || topic_lower.contains("development")
    {
        "Work"
    } else if topic_lower.contains("learn")
        || topic_lower.contains("study")
        || topic_lower.contains("research")
    {
        "Learning"
    } else if topic_lower.contains("personal")
        || topic_lower.contains("health")
        || topic_lower.contains("home")
    {
        "Personal"
    } else {
        "General"
    };

    // Extract project hints from topic
    let project = if topic_lower.contains("memory") || topic_lower.contains("amem") {
        "Memory Layer"
    } else if topic_lower.contains("water") || topic_lower.contains("hydration") {
        "WaterBuddy"
    } else if topic_lower.contains("linkedin") || topic_lower.contains("radar") {
        "Social Radar"
    } else {
        "Miscellaneous"
    };

    // Extract area hints
    let area = if topic_lower.contains("database")
        || topic_lower.contains("schema")
        || topic_lower.contains("sql")
    {
        "Database"
    } else if topic_lower.contains("api") || topic_lower.contains("endpoint") {
        "API"
    } else if topic_lower.contains("ui") || topic_lower.contains("interface") {
        "User Interface"
    } else if topic_lower.contains("swift") || topic_lower.contains("ios") {
        "iOS Development"
    } else {
        "General"
    };

    // Clean up topic name (remove workspace/project/area words)
    let topic = flat_topic
        .replace("Work:", "")
        .replace("Personal:", "")
        .replace("Learning:", "")
        .trim()
        .to_string();

    HierarchySuggestion {
        workspace: workspace.to_string(),
        project: project.to_string(),
        area: area.to_string(),
        topic: if topic.is_empty() {
            flat_topic.to_string()
        } else {
            topic
        },
        rationale: Some("Heuristic-based classification".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_work_topic() {
        let suggestion = suggest_hierarchy_heuristic("Work: Memory Layer Database Schema");
        assert_eq!(suggestion.workspace, "Work");
        assert_eq!(suggestion.project, "Memory Layer");
        assert_eq!(suggestion.area, "Database");
    }

    #[test]
    fn test_heuristic_personal_topic() {
        let suggestion = suggest_hierarchy_heuristic("Personal: WaterBuddy Hydration Model");
        assert_eq!(suggestion.workspace, "Personal");
        assert_eq!(suggestion.project, "WaterBuddy");
    }

    #[test]
    fn test_heuristic_learning_topic() {
        let suggestion = suggest_hierarchy_heuristic("Learning: Rust async programming");
        assert_eq!(suggestion.workspace, "Learning");
    }

    #[test]
    fn test_heuristic_general_topic() {
        let suggestion = suggest_hierarchy_heuristic("Random notes about coffee");
        assert_eq!(suggestion.workspace, "General");
        assert_eq!(suggestion.project, "Miscellaneous");
    }
}
