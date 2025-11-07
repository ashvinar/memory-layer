use chrono::Utc;
use memory_layer_ingestion::{Database, MemoryExtractor, MemoryOrganizer};
use memory_layer_schemas::{
    generate_thread_id, generate_turn_id, SourceApp, Turn, TurnSource,
};
use tempfile::TempDir;

/// Test that memories automatically create hierarchy on insert
#[test]
fn test_auto_organization_creates_hierarchy() {
    // Setup temporary database
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path).unwrap();

    let extractor = MemoryExtractor::new();
    let organizer = MemoryOrganizer::new();

    // Create a turn with VSCode source
    let turn = Turn {
        id: generate_turn_id(),
        thread_id: generate_thread_id(),
        ts_user: Utc::now().to_rfc3339(),
        user_text: "I decided to use Rust for the memory-layer project.".to_string(),
        ts_ai: None,
        ai_text: None,
        source: TurnSource {
            app: SourceApp::VSCode,
            url: None,
            path: Some("/Users/me/code/memory-layer/src/main.rs".to_string()),
        },
    };

    // Extract memories
    let memories = extractor.extract(&turn).unwrap();
    assert!(!memories.is_empty(), "Should extract at least one memory");

    // Insert turn
    db.insert_turn(&turn).unwrap();

    // Organize and insert each memory
    for memory in &memories {
        let organized_memory = organizer.organize(&db, memory, &turn).unwrap();

        // Verify topic_id is set
        assert!(
            organized_memory.topic_id.is_some(),
            "Memory should have topic_id after organization"
        );

        // Insert the organized memory
        db.insert_memory(&organized_memory).unwrap();
    }

    // Verify hierarchy was created
    let workspaces = db.get_all_workspaces().unwrap();
    assert!(
        !workspaces.is_empty(),
        "Should have created at least one workspace"
    );
    assert!(
        workspaces.iter().any(|(_, name)| name == "VSCode"),
        "Should have created VSCode workspace"
    );

    let projects = db.get_all_projects().unwrap();
    assert!(
        !projects.is_empty(),
        "Should have created at least one project"
    );
    assert!(
        projects
            .iter()
            .any(|(_, name, _)| name == "memory-layer"),
        "Should have created memory-layer project"
    );

    let areas = db.get_all_areas().unwrap();
    assert!(!areas.is_empty(), "Should have created at least one area");
    assert!(
        areas.iter().any(|(_, name, _)| name == "Decisions"),
        "Should have created Decisions area"
    );

    let topics = db.get_all_topics().unwrap();
    assert!(!topics.is_empty(), "Should have created at least one topic");
}

/// Test that same app creates same workspace
#[test]
fn test_same_app_same_workspace() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path).unwrap();

    let extractor = MemoryExtractor::new();
    let organizer = MemoryOrganizer::new();

    // Create two turns with the same app (Claude)
    let turn1 = Turn {
        id: generate_turn_id(),
        thread_id: generate_thread_id(),
        ts_user: Utc::now().to_rfc3339(),
        user_text: "I decided to use neural networks for AI modeling.".to_string(),
        ts_ai: None,
        ai_text: None,
        source: TurnSource {
            app: SourceApp::Claude,
            url: None,
            path: None,
        },
    };

    let turn2 = Turn {
        id: generate_turn_id(),
        thread_id: generate_thread_id(),
        ts_user: Utc::now().to_rfc3339(),
        user_text: "I decided to implement gradient descent for ML optimization.".to_string(),
        ts_ai: None,
        ai_text: None,
        source: TurnSource {
            app: SourceApp::Claude,
            url: None,
            path: None,
        },
    };

    // Extract and organize memories from both turns
    for turn in &[turn1, turn2] {
        db.insert_turn(turn).unwrap();
        let memories = extractor.extract(turn).unwrap();

        for memory in &memories {
            let organized_memory = organizer.organize(&db, memory, turn).unwrap();
            db.insert_memory(&organized_memory).unwrap();
        }
    }

    // Should only have one workspace for Claude
    let workspaces = db.get_all_workspaces().unwrap();
    let claude_workspaces: Vec<_> = workspaces
        .iter()
        .filter(|(_, name)| name == "Claude")
        .collect();

    assert_eq!(
        claude_workspaces.len(),
        1,
        "Should have exactly one Claude workspace"
    );
}

/// Test that same file path prefix creates same project
#[test]
fn test_same_path_prefix_same_project() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path).unwrap();

    let extractor = MemoryExtractor::new();
    let organizer = MemoryOrganizer::new();

    // Create two turns with the same project path
    let turn1 = Turn {
        id: generate_turn_id(),
        thread_id: generate_thread_id(),
        ts_user: Utc::now().to_rfc3339(),
        user_text: "I decided to use JWT tokens for authentication module.".to_string(),
        ts_ai: None,
        ai_text: None,
        source: TurnSource {
            app: SourceApp::VSCode,
            url: None,
            path: Some("/Users/me/code/my-app/src/auth.rs".to_string()),
        },
    };

    let turn2 = Turn {
        id: generate_turn_id(),
        thread_id: generate_thread_id(),
        ts_user: Utc::now().to_rfc3339(),
        user_text: "I decided to use PostgreSQL for the database module.".to_string(),
        ts_ai: None,
        ai_text: None,
        source: TurnSource {
            app: SourceApp::VSCode,
            url: None,
            path: Some("/Users/me/code/my-app/src/db.rs".to_string()),
        },
    };

    // Extract and organize memories from both turns
    for turn in &[turn1, turn2] {
        db.insert_turn(turn).unwrap();
        let memories = extractor.extract(turn).unwrap();

        for memory in &memories {
            let organized_memory = organizer.organize(&db, memory, turn).unwrap();
            db.insert_memory(&organized_memory).unwrap();
        }
    }

    // Should only have one project for my-app
    let projects = db.get_all_projects().unwrap();
    let myapp_projects: Vec<_> = projects
        .iter()
        .filter(|(_, name, _)| name == "my-app")
        .collect();

    assert_eq!(
        myapp_projects.len(),
        1,
        "Should have exactly one my-app project"
    );
}

/// Test that hierarchy tables are populated with real data
#[test]
fn test_hierarchy_tables_populated() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path).unwrap();

    let extractor = MemoryExtractor::new();
    let organizer = MemoryOrganizer::new();

    // Create a comprehensive turn
    let turn = Turn {
        id: generate_turn_id(),
        thread_id: generate_thread_id(),
        ts_user: Utc::now().to_rfc3339(),
        user_text: "I decided to use PostgreSQL for the backend. TODO: Set up database schema."
            .to_string(),
        ts_ai: None,
        ai_text: None,
        source: TurnSource {
            app: SourceApp::VSCode,
            url: None,
            path: Some("/Users/me/code/backend-api/src/db/schema.rs".to_string()),
        },
    };

    db.insert_turn(&turn).unwrap();
    let memories = extractor.extract(&turn).unwrap();

    // Should extract multiple memories (decision + task)
    assert!(
        memories.len() >= 2,
        "Should extract at least decision and task"
    );

    for memory in &memories {
        let organized_memory = organizer.organize(&db, memory, &turn).unwrap();
        db.insert_memory(&organized_memory).unwrap();
    }

    // Verify all levels of hierarchy are populated
    let workspaces = db.get_all_workspaces().unwrap();
    assert!(!workspaces.is_empty(), "Workspaces table should be populated");

    let projects = db.get_all_projects().unwrap();
    assert!(!projects.is_empty(), "Projects table should be populated");

    let areas = db.get_all_areas().unwrap();
    assert!(!areas.is_empty(), "Areas table should be populated");

    let topics = db.get_all_topics().unwrap();
    assert!(!topics.is_empty(), "Topics table should be populated");

    // Verify we have both Decisions and Tasks areas
    let area_names: Vec<String> = areas.iter().map(|(_, name, _)| name.clone()).collect();
    assert!(
        area_names.contains(&"Decisions".to_string()),
        "Should have Decisions area"
    );
    assert!(
        area_names.contains(&"Tasks".to_string()),
        "Should have Tasks area"
    );
}

/// Test different memory kinds create different areas
#[test]
fn test_memory_kinds_create_different_areas() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path).unwrap();

    let extractor = MemoryExtractor::new();
    let organizer = MemoryOrganizer::new();

    // Create a turn that should extract multiple memory kinds
    let turn = Turn {
        id: generate_turn_id(),
        thread_id: generate_thread_id(),
        ts_user: Utc::now().to_rfc3339(),
        user_text: "I decided to use Rust. PostgreSQL is a relational database. TODO: Set up CI/CD. Here's the code:\n```rust\nfn main() {}\n```".to_string(),
        ts_ai: None,
        ai_text: None,
        source: TurnSource {
            app: SourceApp::Claude,
            url: None,
            path: None,
        },
    };

    db.insert_turn(&turn).unwrap();
    let memories = extractor.extract(&turn).unwrap();

    for memory in &memories {
        let organized_memory = organizer.organize(&db, memory, &turn).unwrap();
        db.insert_memory(&organized_memory).unwrap();
    }

    // Check that different areas were created
    let areas = db.get_all_areas().unwrap();
    let area_names: Vec<String> = areas.iter().map(|(_, name, _)| name.clone()).collect();

    // We should have at least 3 different areas (Decisions, Facts, Tasks, Code)
    assert!(
        area_names.len() >= 3,
        "Should have created at least 3 different areas"
    );
}
