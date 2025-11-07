/// Test Real Database - Analyze and test with actual data
///
/// This script:
/// 1. Backs up your database
/// 2. Analyzes current data
/// 3. Optionally runs migration
/// 4. Tests new features with real memories

use anyhow::Result;
use memory_layer_ingestion::Database;
use std::path::PathBuf;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    info!("═══════════════════════════════════════════════════════════");
    info!("Memory Layer - Real Database Test");
    info!("═══════════════════════════════════════════════════════════");
    info!("");

    // Database path
    let home = std::env::var("HOME").expect("HOME not set");
    let db_path = PathBuf::from(format!(
        "{}/Library/Application Support/MemoryLayer/memory.db",
        home
    ));

    if !db_path.exists() {
        anyhow::bail!("Database not found at {}", db_path.display());
    }

    info!("Database: {}", db_path.display());

    // Create backup
    let backup_path = PathBuf::from(format!(
        "{}/Library/Application Support/MemoryLayer/memory.db.backup-{}",
        home,
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    ));

    info!("Creating backup at {}", backup_path.display());
    std::fs::copy(&db_path, &backup_path)?;
    info!("✓ Backup created successfully");
    info!("");

    // Open database
    let db = Database::new(&db_path)?;

    // Analysis Phase
    info!("ANALYSIS: Current Database State");
    info!("─────────────────────────────────────────────────────────");

    let total_memories = db.count_memories()?;
    let total_turns = db.count_turns()?;
    info!("  Total memories: {}", total_memories);
    info!("  Total turns: {}", total_turns);
    info!("");

    info!("");

    // Get topic distribution
    info!("  Analyzing topics...");
    match db.topic_summaries(10) {
        Ok(topics) => {
            info!("  Top 10 topics:");
            for (i, topic) in topics.iter().enumerate() {
                info!("    {}. {} ({} memories)",
                    i + 1, topic.topic, topic.memory_count);
            }
        }
        Err(e) => {
            info!("  Could not retrieve topics: {}", e);
        }
    }
    info!("");

    // Get recent memories
    info!("  Recent memories (last 5):");
    match db.get_recent_memories(5) {
        Ok(memories) => {
            for (i, mem) in memories.iter().enumerate() {
                // Use char-aware truncation to handle Unicode correctly
                let text_preview: String = mem.text.chars().take(60).collect();
                let text_preview = if mem.text.chars().count() > 60 {
                    format!("{}...", text_preview)
                } else {
                    text_preview
                };
                info!("    {}. [{:?}] {}",
                    i + 1, mem.kind, text_preview);
                info!("       Topic: {}", mem.topic);
            }
        }
        Err(e) => {
            info!("  Could not retrieve memories: {}", e);
        }
    }
    info!("");

    // Get distinct topics for migration preview
    info!("  Getting distinct topics for migration preview...");
    match db.get_all_distinct_topics() {
        Ok(topics) => {
            info!("  Found {} distinct topics", topics.len());
            info!("  Sample topics (first 10):");
            for (i, (topic, count)) in topics.iter().take(10).enumerate() {
                info!("    {}. {} ({} memories)", i + 1, topic, count);
            }
            info!("");

            if topics.len() > 0 {
                info!("  💡 You can run migration with:");
                info!("     cargo run --bin migrate");
                info!("     OR");
                info!("     cargo run --bin migrate -- --use-llm");
                info!("");
            }
        }
        Err(e) => {
            info!("  Could not retrieve topics: {}", e);
            info!("  This might mean the database schema needs updating.");
        }
    }

    // Test relationship queries if relationships exist
    info!("TEST: Checking for existing relationships...");
    info!("─────────────────────────────────────────────────────────");

    // Try to find any memories with relationships
    match db.get_recent_memories(1) {
        Ok(memories) if !memories.is_empty() => {
            let memory = &memories[0];
            info!("  Testing with memory: {}", memory.id.0);

            match db.get_all_memory_relations(&memory.id) {
                Ok(relations) => {
                    if relations.is_empty() {
                        info!("  No relationships found yet (this is normal for unmigrated data)");
                        info!("  You can create relationships after migration!");
                    } else {
                        info!("  Found {} relationships for this memory", relations.len());
                        for rel in &relations {
                            info!("    - {:?}: {} → {}",
                                rel.relation_type, rel.source_id.0, rel.target_id.0);
                        }
                    }
                }
                Err(e) => {
                    info!("  Could not query relationships: {}", e);
                }
            }

            // Check for versions
            match db.get_memory_versions(&memory.id) {
                Ok(versions) => {
                    if versions.is_empty() {
                        info!("  No version history yet (this is normal)");
                    } else {
                        info!("  Found {} versions", versions.len());
                    }
                }
                Err(e) => {
                    info!("  Could not query versions: {}", e);
                }
            }
        }
        _ => {
            info!("  No memories found to test with");
        }
    }
    info!("");

    // Summary and recommendations
    info!("═══════════════════════════════════════════════════════════");
    info!("SUMMARY");
    info!("═══════════════════════════════════════════════════════════");
    info!("✓ Database opened successfully");
    info!("✓ Backup created: {}", backup_path.display());
    info!("✓ Analysis complete");
    info!("");
    info!("NEXT STEPS:");
    info!("1. Review the topic distribution above");
    info!("2. Run migration to organize into hierarchy:");
    info!("   cargo run --bin migrate");
    info!("3. After migration, you can:");
    info!("   - Create relationships between memories");
    info!("   - Track version history");
    info!("   - Build decision chains");
    info!("   - Query evolution trails");
    info!("");
    info!("Your original database is backed up at:");
    info!("{}", backup_path.display());
    info!("═══════════════════════════════════════════════════════════");

    Ok(())
}
