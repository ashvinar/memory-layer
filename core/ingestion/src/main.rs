use anyhow::Result;
use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use memory_layer_ingestion::{
    ActivityDay, Database, EntityStats, ImportanceStats, IndexNote, LifecycleStats,
    MemoryExtractor, MemoryOrganizer, ProjectSummary, TopicSummary, TrendingTopic,
};
use memory_layer_schemas::{MemoryId, ProjectId, TopicId, Turn, WorkspaceId};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, Level};
use tracing_subscriber;

#[derive(Clone)]
struct AppState {
    db: Arc<Mutex<Database>>,
    extractor: Arc<MemoryExtractor>,
    organizer: Arc<MemoryOrganizer>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Memory Layer Ingestion Service v0.1.0");

    // Initialize database
    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap();
        format!("{}/Library/Application Support/MemoryLayer/memory.db", home)
    });

    // Create directory if it doesn't exist
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db = Database::new(&db_path)?;
    info!("Database initialized at: {}", db_path);

    let extractor = MemoryExtractor::new();
    let organizer = MemoryOrganizer::new();

    let state = AppState {
        db: Arc::new(Mutex::new(db)),
        extractor: Arc::new(extractor),
        organizer: Arc::new(organizer),
    };

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/ingest/turn", post(ingest_turn))
        .route("/stats", get(get_stats))

        // Original endpoints
        .route("/memories/recent", get(list_recent_memories))
        .route("/memories/topics", get(list_topic_summaries))

        // Hierarchy navigation
        .route("/hierarchy/workspaces", get(list_workspaces))
        .route("/hierarchy/projects", get(list_projects))
        .route("/hierarchy/areas", get(list_areas))
        .route("/hierarchy/topics", get(list_topics))

        // Project-based queries
        .route("/projects/:project_id/memories", get(get_project_memories))
        .route("/projects/:project_id/summary", get(get_project_summary_endpoint))
        .route("/projects/:project_id/related", get(get_related_projects_endpoint))
        .route("/projects/:project_id/activity", get(get_project_activity_endpoint))

        // Temporal views
        .route("/temporal/this-week", get(get_this_week))
        .route("/temporal/this-month", get(get_this_month))
        .route("/temporal/this-year", get(get_this_year))
        .route("/temporal/timeline", get(get_timeline))
        .route("/temporal/trending", get(get_trending))

        // Entity navigation
        .route("/entities", get(list_all_entities))
        .route("/entities/:entity", get(get_entity_memories))
        .route("/entities/:entity/stats", get(get_entity_stats_endpoint))
        .route("/entities/:entity/evolution", get(get_entity_evolution_endpoint))
        .route("/entities/:entity/cooccurrence", get(get_entity_cooccurrence_endpoint))

        // Importance filtering
        .route("/importance/high-priority", get(get_high_priority))
        .route("/importance/stats", get(get_importance_stats_endpoint))
        .route("/importance/:memory_id/recalculate", post(recalculate_importance_endpoint))

        // Lifecycle management
        .route("/lifecycle/stats", get(get_lifecycle_stats_endpoint))
        .route("/lifecycle/:status", get(get_memories_by_status))

        // Index notes
        .route("/index-notes", get(list_index_notes))
        .route("/index-notes/:topic_id", get(get_index_note_endpoint))

        .with_state(state);

    // Start server
    let addr = "127.0.0.1:21953";
    info!("Starting HTTP server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "service": "ingestion",
        "status": "healthy",
        "version": "0.1.0"
    }))
}

async fn ingest_turn(
    State(state): State<AppState>,
    Json(turn): Json<Turn>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("Ingesting turn: {}", turn.id);

    // Extract memories from the turn
    let memories = state.extractor.extract(&turn).map_err(|e| {
        error!("Failed to extract memories: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    info!(
        "Extracted {} memories from turn {}",
        memories.len(),
        turn.id
    );

    // Store turn and memories
    let db = state.db.lock().await;

    db.insert_turn(&turn).map_err(|e| {
        error!("Failed to insert turn: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    for memory in &memories {
        // Auto-organize memory into hierarchy
        let organized_memory = state.organizer.organize(&db, memory, &turn).map_err(|e| {
            error!("Failed to organize memory: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

        db.insert_memory(&organized_memory).map_err(|e| {
            error!("Failed to insert memory: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

        db.upsert_agentic_memory(&organized_memory).map_err(|e| {
            error!("Failed to capture agentic metadata: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    }

    Ok(Json(serde_json::json!({
        "turn_id": turn.id.0,
        "memories_extracted": memories.len()
    })))
}

async fn get_stats(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;

    let turn_count = db.count_turns().map_err(|e| {
        error!("Failed to count turns: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    let memory_count = db.count_memories().map_err(|e| {
        error!("Failed to count memories: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({
        "turns": turn_count,
        "memories": memory_count
    })))
}

#[derive(Debug, Default, Deserialize)]
struct ListQuery {
    limit: Option<usize>,
}

async fn list_recent_memories(
    State(state): State<AppState>,
    query: Option<Query<ListQuery>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let params = query.map(|q| q.0).unwrap_or_default();
    let limit = params.limit.unwrap_or(100).clamp(1, 500);

    let db = state.db.lock().await;
    let memories = db.get_recent_memories(limit).map_err(|e| {
        error!("Failed to list memories: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({
        "memories": memories
    })))
}

async fn list_topic_summaries(
    State(state): State<AppState>,
    query: Option<Query<ListQuery>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let params = query.map(|q| q.0).unwrap_or_default();
    let limit = params.limit.unwrap_or(50).clamp(1, 200);

    let db = state.db.lock().await;
    let topics: Vec<TopicSummary> = db.topic_summaries(limit).map_err(|e| {
        error!("Failed to list topic summaries: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({
        "topics": topics
    })))
}

// ========== HIERARCHY NAVIGATION ==========

async fn list_workspaces(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;
    let workspaces = db.get_all_workspaces().map_err(|e| {
        error!("Failed to list workspaces: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "workspaces": workspaces })))
}

#[derive(Deserialize)]
struct WorkspaceQuery {
    workspace_id: Option<String>,
}

async fn list_projects(
    State(state): State<AppState>,
    query: Option<Query<WorkspaceQuery>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;

    if let Some(Query(WorkspaceQuery { workspace_id: Some(ws_id) })) = query {
        let workspace_id = WorkspaceId(ws_id);
        let projects = db.get_projects_by_workspace(&workspace_id).map_err(|e| {
            error!("Failed to list projects for workspace: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
        Ok(Json(serde_json::json!({ "projects": projects })))
    } else {
        let projects = db.get_all_projects().map_err(|e| {
            error!("Failed to list all projects: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
        Ok(Json(serde_json::json!({ "projects": projects })))
    }
}

#[derive(Deserialize)]
struct ProjectQuery {
    project_id: Option<String>,
}

async fn list_areas(
    State(state): State<AppState>,
    query: Option<Query<ProjectQuery>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;

    if let Some(Query(ProjectQuery { project_id: Some(proj_id) })) = query {
        let project_id = ProjectId(proj_id);
        let areas = db.get_areas_by_project(&project_id).map_err(|e| {
            error!("Failed to list areas for project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
        Ok(Json(serde_json::json!({ "areas": areas })))
    } else {
        let areas = db.get_all_areas().map_err(|e| {
            error!("Failed to list all areas: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
        Ok(Json(serde_json::json!({ "areas": areas })))
    }
}

async fn list_topics(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;
    let topics = db.get_all_topics().map_err(|e| {
        error!("Failed to list topics: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "topics": topics })))
}

// ========== PROJECT-BASED QUERIES ==========

async fn get_project_memories(
    State(state): State<AppState>,
    axum::extract::Path(project_id): axum::extract::Path<String>,
    query: Option<Query<ListQuery>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let params = query.map(|q| q.0).unwrap_or_default();
    let limit = params.limit.map(|l| l.clamp(1, 500));

    let db = state.db.lock().await;
    let project_id = ProjectId(project_id);
    let memories = db.get_project_memories(&project_id, limit).map_err(|e| {
        error!("Failed to get project memories: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "memories": memories })))
}

async fn get_project_summary_endpoint(
    State(state): State<AppState>,
    axum::extract::Path(project_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;
    let project_id = ProjectId(project_id);
    let summary: ProjectSummary = db.get_project_summary(&project_id).map_err(|e| {
        error!("Failed to get project summary: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(summary))
}

#[derive(Deserialize)]
struct RelatedQuery {
    limit: Option<usize>,
}

async fn get_related_projects_endpoint(
    State(state): State<AppState>,
    axum::extract::Path(project_id): axum::extract::Path<String>,
    query: Option<Query<RelatedQuery>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let params = query.map(|q| q.0).unwrap_or(RelatedQuery { limit: None });
    let limit = params.limit.unwrap_or(5).clamp(1, 20);

    let db = state.db.lock().await;
    let project_id = ProjectId(project_id);
    let related = db.get_related_projects(&project_id, limit).map_err(|e| {
        error!("Failed to get related projects: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "related_projects": related })))
}

#[derive(Deserialize)]
struct ActivityQuery {
    days: Option<usize>,
}

async fn get_project_activity_endpoint(
    State(state): State<AppState>,
    axum::extract::Path(project_id): axum::extract::Path<String>,
    query: Option<Query<ActivityQuery>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let params = query.map(|q| q.0).unwrap_or(ActivityQuery { days: None });
    let days = params.days.unwrap_or(7).clamp(1, 365);

    let db = state.db.lock().await;
    let project_id = ProjectId(project_id);
    let activity = db.get_project_activity(&project_id, days).map_err(|e| {
        error!("Failed to get project activity: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "activity": activity, "days": days })))
}

// ========== TEMPORAL VIEWS ==========

async fn get_this_week(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;
    let memories = db.get_this_week_memories().map_err(|e| {
        error!("Failed to get this week's memories: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "memories": memories, "period": "week" })))
}

async fn get_this_month(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;
    let memories = db.get_this_month_memories().map_err(|e| {
        error!("Failed to get this month's memories: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "memories": memories, "period": "month" })))
}

async fn get_this_year(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;
    let memories = db.get_this_year_memories().map_err(|e| {
        error!("Failed to get this year's memories: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "memories": memories, "period": "year" })))
}

#[derive(Deserialize)]
struct TimelineQuery {
    days: Option<usize>,
}

async fn get_timeline(
    State(state): State<AppState>,
    query: Option<Query<TimelineQuery>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let params = query.map(|q| q.0).unwrap_or(TimelineQuery { days: None });
    let days = params.days.unwrap_or(30).clamp(1, 365);

    let db = state.db.lock().await;
    let timeline: Vec<ActivityDay> = db.get_activity_timeline(days).map_err(|e| {
        error!("Failed to get activity timeline: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "timeline": timeline, "days": days })))
}

#[derive(Deserialize)]
struct TrendingQuery {
    days: Option<usize>,
    limit: Option<usize>,
}

async fn get_trending(
    State(state): State<AppState>,
    query: Option<Query<TrendingQuery>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let params = query.map(|q| q.0).unwrap_or(TrendingQuery { days: None, limit: None });
    let days = params.days.unwrap_or(7).clamp(1, 365);
    let limit = params.limit.unwrap_or(10).clamp(1, 50);

    let db = state.db.lock().await;
    let trending: Vec<TrendingTopic> = db.get_trending_topics(days, limit).map_err(|e| {
        error!("Failed to get trending topics: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "trending": trending })))
}

// ========== ENTITY NAVIGATION ==========

async fn list_all_entities(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;
    let entities = db.get_all_entities().map_err(|e| {
        error!("Failed to list entities: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "entities": entities, "count": entities.len() })))
}

async fn get_entity_memories(
    State(state): State<AppState>,
    axum::extract::Path(entity): axum::extract::Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;
    let memories = db.get_memories_by_entity(&entity).map_err(|e| {
        error!("Failed to get memories for entity: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "entity": entity, "memories": memories })))
}

async fn get_entity_stats_endpoint(
    State(state): State<AppState>,
    axum::extract::Path(entity): axum::extract::Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;
    let stats: EntityStats = db.get_entity_stats(&entity).map_err(|e| {
        error!("Failed to get entity stats: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(stats))
}

#[derive(Deserialize)]
struct EvolutionQuery {
    days: Option<usize>,
}

async fn get_entity_evolution_endpoint(
    State(state): State<AppState>,
    axum::extract::Path(entity): axum::extract::Path<String>,
    query: Option<Query<EvolutionQuery>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let params = query.map(|q| q.0).unwrap_or(EvolutionQuery { days: None });
    let days = params.days.unwrap_or(30).clamp(1, 365);

    let db = state.db.lock().await;
    let evolution = db.get_entity_evolution(&entity, days).map_err(|e| {
        error!("Failed to get entity evolution: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "entity": entity, "evolution": evolution, "days": days })))
}

#[derive(Deserialize)]
struct CooccurrenceQuery {
    limit: Option<usize>,
}

async fn get_entity_cooccurrence_endpoint(
    State(state): State<AppState>,
    axum::extract::Path(entity): axum::extract::Path<String>,
    query: Option<Query<CooccurrenceQuery>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let params = query.map(|q| q.0).unwrap_or(CooccurrenceQuery { limit: None });
    let limit = params.limit.unwrap_or(10).clamp(1, 50);

    let db = state.db.lock().await;
    let cooccurrence = db.get_entity_cooccurrence(&entity, limit).map_err(|e| {
        error!("Failed to get entity cooccurrence: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "entity": entity, "cooccurrence": cooccurrence })))
}

// ========== IMPORTANCE FILTERING ==========

async fn get_high_priority(
    State(state): State<AppState>,
    query: Option<Query<ListQuery>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let params = query.map(|q| q.0).unwrap_or_default();
    let limit = params.limit.unwrap_or(50).clamp(1, 200);

    let db = state.db.lock().await;
    let memories = db.get_high_priority_memories(limit).map_err(|e| {
        error!("Failed to get high priority memories: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "memories": memories })))
}

async fn get_importance_stats_endpoint(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;
    let stats: ImportanceStats = db.get_importance_stats().map_err(|e| {
        error!("Failed to get importance stats: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(stats))
}

async fn recalculate_importance_endpoint(
    State(state): State<AppState>,
    axum::extract::Path(memory_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;
    let memory_id = MemoryId(memory_id);
    let importance = db.recalculate_and_update_importance(&memory_id).map_err(|e| {
        error!("Failed to recalculate importance: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "memory_id": memory_id.0, "importance": importance })))
}

// ========== LIFECYCLE MANAGEMENT ==========

async fn get_lifecycle_stats_endpoint(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;
    let stats: LifecycleStats = db.get_lifecycle_stats().map_err(|e| {
        error!("Failed to get lifecycle stats: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(stats))
}

async fn get_memories_by_status(
    State(state): State<AppState>,
    axum::extract::Path(status): axum::extract::Path<String>,
    query: Option<Query<ListQuery>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let params = query.map(|q| q.0).unwrap_or_default();
    let limit = params.limit.map(|l| l.clamp(1, 500));

    let db = state.db.lock().await;
    let memories = db.get_memories_by_status(&status, limit).map_err(|e| {
        error!("Failed to get memories by status: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "status": status, "memories": memories })))
}

// ========== INDEX NOTES ==========

async fn list_index_notes(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;
    let notes: Vec<IndexNote> = db.get_all_index_notes().map_err(|e| {
        error!("Failed to list index notes: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "index_notes": notes, "count": notes.len() })))
}

async fn get_index_note_endpoint(
    State(state): State<AppState>,
    axum::extract::Path(topic_id_str): axum::extract::Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db.lock().await;
    let topic_id = TopicId(topic_id_str);

    // Get topic name and area name first
    let (topic_name, area_name) = db.get_topic_info(&topic_id).map_err(|e| {
        error!("Failed to get topic/area name: {}", e);
        (StatusCode::NOT_FOUND, "Topic not found".to_string())
    })?;

    let note = db.get_index_note(&topic_name, &area_name).map_err(|e| {
        error!("Failed to get index note: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    match note {
        Some(n) => Ok(Json(serde_json::json!(n))),
        None => Err((StatusCode::NOT_FOUND, "Index note not found".to_string())),
    }
}
