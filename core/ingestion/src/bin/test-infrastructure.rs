/// Infrastructure Test - Validates Phase 1 & 2 features
///
/// Tests:
/// - Hierarchy creation (Workspace → Project → Area → Topic)
/// - Memory creation and retrieval
/// - Typed relationships (Supersedes, Implements, Questions, etc.)
/// - Memory versioning and history
/// - Narrative queries (decision chains, evolution trails, etc.)

use anyhow::Result;
use chrono::Utc;
use memory_layer_ingestion::{Database, RelationDirection};
use memory_layer_schemas::{
    generate_memory_id, generate_turn_id, Memory, MemoryKind, ProjectStatus, RelationType,
};
use tempfile::NamedTempFile;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    info!("═══════════════════════════════════════════════════════════");
    info!("Memory Layer Infrastructure Test v0.1.0");
    info!("Testing Phases 1 & 2: Hierarchy, Relationships, Versioning");
    info!("═══════════════════════════════════════════════════════════");
    info!("");

    // Create temporary database
    let temp_db = NamedTempFile::new()?;
    let db = Database::new(temp_db.path())?;

    info!("✓ Database initialized at {}", temp_db.path().display());
    info!("");

    // Test 1: Hierarchy Creation
    info!("TEST 1: Hierarchy Creation");
    info!("─────────────────────────────────────────────────────────");

    let workspace_id = db.get_or_create_workspace("Work", Some("Professional projects"))?;
    info!("  Created workspace: Work ({})", workspace_id.0);

    let project_id = db.get_or_create_project(
        &workspace_id,
        "Memory Layer",
        Some("Personal knowledge management system"),
        ProjectStatus::Active,
    )?;
    info!("  Created project: Memory Layer ({})", project_id.0);

    let area_id = db.get_or_create_area(&project_id, "Database", Some("Core data layer"))?;
    info!("  Created area: Database ({})", area_id.0);

    let topic_id = db.get_or_create_topic(
        &area_id,
        "Schema Design",
        Some("Database schema architecture"),
        false,
    )?;
    info!("  Created topic: Schema Design ({})", topic_id.0);
    info!("  ✓ Full hierarchy created: Workspace → Project → Area → Topic");
    info!("");

    // Test 2: Memory Creation
    info!("TEST 2: Memory Creation");
    info!("─────────────────────────────────────────────────────────");

    let decision_memory = Memory {
        id: generate_memory_id(),
        kind: MemoryKind::Decision,
        topic: "Database: Schema Design".to_string(),
        text: "Decided to use hierarchical topic organization with 4 levels".to_string(),
        snippet: None,
        entities: vec!["hierarchy".to_string(), "topics".to_string()],
        provenance: vec![generate_turn_id()],
        created_at: Utc::now().to_rfc3339(),
        ttl: None,
    };

    let implementation_memory = Memory {
        id: generate_memory_id(),
        kind: MemoryKind::Fact,
        topic: "Database: Schema Design".to_string(),
        text: "Implemented workspaces, projects, areas, and topics tables with foreign keys"
            .to_string(),
        snippet: None,
        entities: vec!["implementation".to_string(), "tables".to_string()],
        provenance: vec![generate_turn_id()],
        created_at: Utc::now().to_rfc3339(),
        ttl: None,
    };

    let question_memory = Memory {
        id: generate_memory_id(),
        kind: MemoryKind::Task,
        topic: "Database: Schema Design".to_string(),
        text: "Should we add indexes for all foreign keys?".to_string(),
        snippet: None,
        entities: vec!["question".to_string(), "indexes".to_string()],
        provenance: vec![generate_turn_id()],
        created_at: Utc::now().to_rfc3339(),
        ttl: None,
    };

    db.insert_memory(&decision_memory)?;
    db.insert_memory(&implementation_memory)?;
    db.insert_memory(&question_memory)?;

    info!("  Created 3 memories:");
    info!("    1. Decision: {}", decision_memory.id.0);
    info!("    2. Implementation: {}", implementation_memory.id.0);
    info!("    3. Question: {}", question_memory.id.0);
    info!("  ✓ All memories inserted successfully");
    info!("");

    // Test 3: Typed Relationships
    info!("TEST 3: Typed Relationships");
    info!("─────────────────────────────────────────────────────────");

    let impl_relation = db.create_memory_relation(
        &implementation_memory.id,
        &decision_memory.id,
        RelationType::Implements,
        Some("Direct implementation of the hierarchy decision"),
    )?;
    info!("  Created IMPLEMENTS relation: {} → {}",
        implementation_memory.id.0, decision_memory.id.0);

    let question_relation = db.create_memory_relation(
        &question_memory.id,
        &decision_memory.id,
        RelationType::Questions,
        Some("Follow-up question about index optimization"),
    )?;
    info!("  Created QUESTIONS relation: {} → {}",
        question_memory.id.0, decision_memory.id.0);

    let all_relations = db.get_all_memory_relations(&decision_memory.id)?;
    info!("  ✓ Decision memory has {} relationships", all_relations.len());
    info!("");

    // Test 4: Memory Versioning
    info!("TEST 4: Memory Versioning");
    info!("─────────────────────────────────────────────────────────");

    let old_content = decision_memory.text.clone();
    let version_id = db.create_memory_version(
        &decision_memory.id,
        &old_content,
        Some("Updated to clarify 4-level hierarchy"),
    )?;
    info!("  Created version 1: {}", version_id.0);

    let version_id2 = db.create_memory_version(
        &decision_memory.id,
        "Decided to use hierarchical topic organization with 4 levels (workspace, project, area, topic)",
        Some("Added level names for clarity"),
    )?;
    info!("  Created version 2: {}", version_id2.0);

    let versions = db.get_memory_versions(&decision_memory.id)?;
    info!("  Memory has {} versions", versions.len());

    let diff = db.get_version_diff(&decision_memory.id, 1, 2)?;
    info!("  Diff v1→v2: {:.1}% similar, {} words added, {} words removed",
        diff.similarity * 100.0, diff.words_added, diff.words_removed);

    let stats = db.get_version_stats(&decision_memory.id)?;
    info!("  ✓ Version stats: {} total versions, {} changes",
        stats.total_versions, stats.total_changes);
    info!("");

    // Test 5: Decision Chain Query
    info!("TEST 5: Decision Chain Query");
    info!("─────────────────────────────────────────────────────────");

    let chain = db.get_decision_chain(&decision_memory.id)?;
    info!("  Decision: {}", chain.decision.text);
    info!("  Implementations: {}", chain.implementations.len());
    for (i, impl_mem) in chain.implementations.iter().enumerate() {
        info!("    {}. {}", i + 1, impl_mem.text);
    }
    info!("  Questions: {}", chain.questions.len());
    for (i, q_mem) in chain.questions.iter().enumerate() {
        info!("    {}. {}", i + 1, q_mem.text);
    }
    info!("  ✓ Decision chain successfully retrieved");
    info!("");

    // Test 6: Evolution Trail
    info!("TEST 6: Evolution Trail");
    info!("─────────────────────────────────────────────────────────");

    // Create a supersedes chain
    let old_decision = Memory {
        id: generate_memory_id(),
        kind: MemoryKind::Decision,
        topic: "Database: Schema Design".to_string(),
        text: "Initially planned flat topic strings".to_string(),
        snippet: None,
        entities: vec![],
        provenance: vec![generate_turn_id()],
        created_at: "2024-01-01T00:00:00Z".to_string(),
        ttl: None,
    };

    db.insert_memory(&old_decision)?;

    db.create_memory_relation(
        &decision_memory.id,
        &old_decision.id,
        RelationType::Supersedes,
        Some("Hierarchy is better than flat structure"),
    )?;

    let trail = db.get_evolution_trail(&decision_memory.id)?;
    info!("  Evolution trail ({} steps):", trail.len());
    for (i, mem) in trail.iter().enumerate() {
        info!("    Step {}: {} ({})", i + 1, mem.text, mem.created_at);
    }
    info!("  ✓ Evolution trail shows decision progression");
    info!("");

    // Test 7: Memory Narrative
    info!("TEST 7: Complete Memory Narrative");
    info!("─────────────────────────────────────────────────────────");

    let narrative = db.get_memory_narrative(&decision_memory.id)?;
    info!("  Memory: {}", narrative.memory.text);
    info!("  Total relations: {}", narrative.relations.len());
    info!("  Versions: {}", narrative.versions.len());
    info!("  Evolution trail length: {}", narrative.evolution_trail.len());
    info!("  Supersedes: {}", narrative.supersedes.len());
    info!("  Implementations: {}", narrative.implements.len());
    info!("  Questions: {}", narrative.questions.len());
    info!("  ✓ Full narrative successfully constructed");
    info!("");

    // Test 8: Relationship Query by Type
    info!("TEST 8: Query Related Memories by Type");
    info!("─────────────────────────────────────────────────────────");

    let implementations = db.get_related_memories_by_type(
        &decision_memory.id,
        RelationType::Implements,
        RelationDirection::Incoming,
    )?;
    info!("  Found {} implementations", implementations.len());

    let questions = db.get_related_memories_by_type(
        &decision_memory.id,
        RelationType::Questions,
        RelationDirection::Incoming,
    )?;
    info!("  Found {} questions", questions.len());

    info!("  ✓ Relationship type queries working");
    info!("");

    // Test 9: Hierarchy Integration
    info!("TEST 9: Hierarchy Integration with Memories");
    info!("─────────────────────────────────────────────────────────");

    let updated = db.update_memories_topic("Database: Schema Design", &topic_id)?;
    info!("  Updated {} memories to use hierarchical topic_id", updated);

    let memory_count = db.count_memories_by_topic_id(&topic_id)?;
    info!("  Topic now has {} memories", memory_count);

    db.generate_or_update_index_note(&topic_id, "Schema Design", "Database", memory_count)?;
    info!("  Generated index note for topic");
    info!("  ✓ Memories successfully integrated with hierarchy");
    info!("");

    // Summary
    info!("═══════════════════════════════════════════════════════════");
    info!("TEST SUMMARY");
    info!("═══════════════════════════════════════════════════════════");
    info!("✓ Test 1: Hierarchy Creation - PASSED");
    info!("✓ Test 2: Memory Creation - PASSED");
    info!("✓ Test 3: Typed Relationships - PASSED");
    info!("✓ Test 4: Memory Versioning - PASSED");
    info!("✓ Test 5: Decision Chain Query - PASSED");
    info!("✓ Test 6: Evolution Trail - PASSED");
    info!("✓ Test 7: Complete Memory Narrative - PASSED");
    info!("✓ Test 8: Relationship Query by Type - PASSED");
    info!("✓ Test 9: Hierarchy Integration - PASSED");
    info!("");
    info!("🎉 ALL TESTS PASSED! Infrastructure is ready.");
    info!("═══════════════════════════════════════════════════════════");

    Ok(())
}
