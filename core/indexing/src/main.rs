use anyhow::Result;
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use memory_layer_indexing::{AgenticMemoryBase, EmbeddingEngine, SearchEngine};
use memory_layer_schemas::MemoryId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, Level};
use tracing_subscriber;

#[derive(Clone)]
struct AppState {
    search_engine: Arc<Mutex<SearchEngine>>,
    embedding_engine: Arc<Mutex<EmbeddingEngine>>,
    agentic_base: Arc<Mutex<AgenticMemoryBase>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Memory Layer Indexing Service v0.1.0");

    // Initialize search engine
    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap();
        format!("{}/Library/Application Support/MemoryLayer/memory.db", home)
    });

    let search_engine = SearchEngine::new(&db_path)?;
    info!("Search engine initialized");

    let embedding_engine = EmbeddingEngine::new();
    info!("Embedding engine initialized (stub mode)");

    let agentic_base = AgenticMemoryBase::new(&db_path)?;
    info!("Agentic memory base ready");

    let state = AppState {
        search_engine: Arc::new(Mutex::new(search_engine)),
        embedding_engine: Arc::new(Mutex::new(embedding_engine)),
        agentic_base: Arc::new(Mutex::new(agentic_base)),
    };

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/search", get(search))
        .route("/topics", get(get_topics))
        .route("/embed", post(generate_embedding))
        .route("/agentic/recent", get(agentic_recent))
        .route("/agentic/search", get(agentic_search))
        .route("/agentic/:id", get(agentic_get))
        .route("/agentic/graph", get(agentic_graph))
        .with_state(state);

    // Start server
    let addr = "127.0.0.1:21954";
    info!("Starting HTTP server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "service": "indexing",
        "status": "healthy",
        "version": "0.1.0"
    }))
}

#[derive(Debug, Deserialize)]
struct SearchParams {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_recency_weight")]
    recency_weight: f32,
}

fn default_limit() -> usize {
    10
}

fn default_recency_weight() -> f32 {
    0.3
}

async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("Searching: {} (limit: {})", params.q, params.limit);

    let search_engine = state.search_engine.lock().await;

    let results = search_engine
        .search(&params.q, params.limit, params.recency_weight)
        .map_err(|e| {
            error!("Search failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!("Found {} results", results.len());

    Ok(Json(results))
}

async fn get_topics(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let search_engine = state.search_engine.lock().await;

    let topics = search_engine.get_topics().map_err(|e| {
        error!("Failed to get topics: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "topics": topics })))
}

#[derive(Debug, Deserialize)]
struct EmbedRequest {
    text: String,
}

#[derive(Debug, Serialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
    dimensions: usize,
}

async fn generate_embedding(
    State(state): State<AppState>,
    Json(request): Json<EmbedRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut embedding_engine = state.embedding_engine.lock().await;

    let embedding = embedding_engine.embed(&request.text).map_err(|e| {
        error!("Failed to generate embedding: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(EmbedResponse {
        dimensions: embedding.len(),
        embedding,
    }))
}

fn default_agentic_limit() -> usize {
    12
}

#[derive(Debug, Deserialize)]
struct AgenticListParams {
    #[serde(default = "default_agentic_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize)]
struct AgenticSearchParams {
    q: String,
    #[serde(default = "default_agentic_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize)]
struct AgenticGraphParams {
    #[serde(default = "default_graph_limit")]
    limit: usize,
}

fn default_graph_limit() -> usize {
    200
}

async fn agentic_recent(
    State(state): State<AppState>,
    Query(params): Query<AgenticListParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let limit = params.limit.clamp(1, 50);
    let base = state.agentic_base.lock().await;

    let memories = base.list_recent(limit).map_err(|e| {
        error!("Failed to list agentic memories: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "memories": memories })))
}

async fn agentic_search(
    State(state): State<AppState>,
    Query(params): Query<AgenticSearchParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if params.q.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "query cannot be empty".into()));
    }

    let limit = params.limit.clamp(1, 50);
    let base = state.agentic_base.lock().await;

    let memories = base.search(params.q.trim(), limit).map_err(|e| {
        error!("Agentic search failed: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "memories": memories })))
}

async fn agentic_get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let memory_id = MemoryId(id);
    let memory = {
        let base = state.agentic_base.lock().await;
        base.get(&memory_id).map_err(|e| {
            error!("Failed to load agentic memory {}: {}", memory_id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
    };

    let Some(mut memory) = memory else {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Agentic memory {} not found", memory_id),
        ));
    };

    state
        .agentic_base
        .lock()
        .await
        .record_access(&memory_id)
        .map_err(|e| {
            error!("Failed to record access for {}: {}", memory_id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    memory.retrieval_count += 1;
    memory.last_accessed = Utc::now().to_rfc3339();

    Ok(Json(memory))
}

async fn agentic_graph(
    State(state): State<AppState>,
    Query(params): Query<AgenticGraphParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let limit = params.limit.clamp(1, 500);
    let base = state.agentic_base.lock().await;

    let graph = base.graph(limit).map_err(|e| {
        error!("Failed to build agentic graph: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(graph))
}
