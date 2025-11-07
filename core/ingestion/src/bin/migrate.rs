/// Migration Binary - Converts flat topic-based memories to hierarchical structure
///
/// Usage:
///   cargo run --bin migrate [--db-path <path>] [--use-llm]
///
/// Options:
///   --db-path: Path to SQLite database (defaults to ~/Library/Application Support/MemoryLayer/memory.db)
///   --use-llm: Use Ollama LLM for intelligent hierarchy suggestions (requires OLLAMA_HOST and OLLAMA_MODEL env vars)

use anyhow::Result;
use clap::Parser;
use memory_layer_ingestion::{migrate_flat_to_hierarchical, Database};
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber;

#[derive(Parser, Debug)]
#[command(name = "migrate")]
#[command(about = "Migrate flat topic-based memories to hierarchical structure")]
struct Args {
    /// Path to SQLite database file
    #[arg(long, short)]
    db_path: Option<PathBuf>,

    /// Use LLM (Ollama) for intelligent hierarchy suggestions
    #[arg(long)]
    use_llm: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    info!("Memory Layer - Hierarchy Migration Tool v0.1.0");

    // Parse arguments
    let args = Args::parse();

    // Determine database path
    let db_path = args.db_path.unwrap_or_else(|| {
        let home = std::env::var("HOME").expect("HOME environment variable not set");
        PathBuf::from(format!(
            "{}/Library/Application Support/MemoryLayer/memory.db",
            home
        ))
    });

    info!("Database: {}", db_path.display());

    if args.use_llm {
        info!("Using LLM for intelligent hierarchy suggestions");
        let ollama_host =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
        let ollama_model =
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2:3b".to_string());
        info!("  Ollama Host: {}", ollama_host);
        info!("  Ollama Model: {}", ollama_model);
    } else {
        info!("Using heuristic-based hierarchy suggestions (faster but less accurate)");
    }

    // Open database
    info!("Opening database...");
    let db = Database::new(&db_path)?;

    // Check if database has memories to migrate
    let memory_count = db.count_memories()?;
    info!("Found {} total memories in database", memory_count);

    if memory_count == 0 {
        info!("No memories to migrate. Exiting.");
        return Ok(());
    }

    // Run migration
    info!("Starting migration...");
    info!("─────────────────────────────────────────────────");

    let stats = migrate_flat_to_hierarchical(&db, args.use_llm).await?;

    info!("─────────────────────────────────────────────────");
    info!("Migration complete!");
    info!("");
    info!("Statistics:");
    info!("  Total topics:        {}", stats.total_topics);
    info!("  Total memories:      {}", stats.total_memories);
    info!("  Workspaces created:  {}", stats.workspaces_created);
    info!("  Projects created:    {}", stats.projects_created);
    info!("  Areas created:       {}", stats.areas_created);
    info!("  Topics created:      {}", stats.topics_created);
    info!("  Memories updated:    {}", stats.memories_updated);
    info!("");

    if stats.memories_updated == stats.total_memories {
        info!("✓ All memories successfully migrated to hierarchical structure!");
    } else {
        info!(
            "⚠ Warning: {} memories were not updated",
            stats.total_memories - stats.memories_updated
        );
    }

    Ok(())
}
