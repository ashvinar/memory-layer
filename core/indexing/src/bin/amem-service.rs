/// Enhanced Main Entry Point with A-mem Integration
/// This file shows how to use the A-mem system with the Memory Layer

use anyhow::Result;
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Router,
};
use memory_layer_indexing::{
    AMemSystem, ClaudeProvider, InMemoryVectorStore, OllamaProvider, OpenAIProvider,
    embedding_adapter::EmbeddingAdapter,
};
use memory_layer_schemas::MemoryId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, Level};
use tracing_subscriber;

#[derive(Clone)]
struct AppState {
    amem_system: Arc<Mutex<AMemSystem>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Memory Layer A-mem Enhanced Indexing Service v0.2.0");

    // Initialize database path
    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap();
        format!("{}/Library/Application Support/MemoryLayer/memory.db", home)
    });

    // Initialize vector store (in-memory for now, can be replaced with ChromaDB)
    let vector_store = Box::new(InMemoryVectorStore::new());

    // Initialize embedding engine
    let embedding_engine = Box::new(EmbeddingAdapter::new());

    // Initialize LLM provider based on environment variables
    let llm_provider: Option<Box<dyn memory_layer_indexing::LLMProvider>> =
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            info!("Using OpenAI for memory enrichment");
            Some(Box::new(OpenAIProvider::new(api_key, None)))
        } else if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            info!("Using Claude for memory enrichment");
            Some(Box::new(ClaudeProvider::new(api_key, None)))
        } else if std::env::var("OLLAMA_HOST").is_ok() {
            info!("Using Ollama for memory enrichment");
            Some(Box::new(OllamaProvider::new(
                std::env::var("OLLAMA_MODEL").ok(),
                std::env::var("OLLAMA_HOST").ok(),
            )))
        } else {
            info!("No LLM provider configured, using fallback enrichment");
            None
        };

    // Initialize A-mem system
    let amem_system = AMemSystem::new(
        &db_path,
        vector_store,
        embedding_engine,
        llm_provider,
    )?;

    info!("A-mem system initialized");

    let state = AppState {
        amem_system: Arc::new(Mutex::new(amem_system)),
    };

    // Build router with A-mem endpoints
    let app = Router::new()
        .route("/health", get(health_check))

        // A-mem endpoints
        .route("/amem/add", post(amem_add_memory))
        .route("/amem/search", get(amem_search))
        // TODO: reflect endpoint disabled due to Send requirement with rusqlite
        // .route("/amem/reflect", post(amem_reflect))
        .route("/amem/memory/:id", get(amem_get_memory))
        .route("/amem/memory/:id", delete(amem_delete_memory))
        .route("/amem/graph", get(amem_get_graph))

        .with_state(state);

    // Start server
    let addr = "127.0.0.1:21956"; // New port for A-mem enhanced service
    info!("Starting A-mem enhanced HTTP server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "service": "amem-indexing",
        "status": "healthy",
        "version": "0.2.0",
        "features": ["memory-evolution", "semantic-linking", "llm-enrichment"]
    }))
}

#[derive(Debug, Deserialize)]
struct AddMemoryRequest {
    content: String,
    context: Option<String>,
    tags: Option<Vec<String>>,
    category: Option<String>,
}

#[derive(Debug, Serialize)]
struct AddMemoryResponse {
    memory_id: String,
    message: String,
}

async fn amem_add_memory(
    State(state): State<AppState>,
    Json(request): Json<AddMemoryRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("Adding memory with A-mem evolution");

    let mut amem = state.amem_system.lock().await;

    let memory_id = amem
        .add_memory(
            request.content,
            request.context,
            request.tags,
            request.category,
        )
        .await
        .map_err(|e| {
            error!("Failed to add memory: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    Ok(Json(AddMemoryResponse {
        memory_id: memory_id.0,
        message: "Memory added successfully with automatic evolution and linking".to_string(),
    }))
}

#[derive(Debug, Deserialize)]
struct SearchParams {
    q: String,
    #[serde(default = "default_k")]
    k: usize,
}

fn default_k() -> usize {
    10
}

async fn amem_search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("A-mem semantic search: {}", params.q);

    let amem = state.amem_system.lock().await;

    let memories = amem
        .search_agentic(&params.q, params.k)
        .map_err(|e| {
            error!("A-mem search failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    Ok(Json(serde_json::json!({
        "query": params.q,
        "count": memories.len(),
        "memories": memories
    })))
}

#[derive(Debug, Deserialize)]
struct ReflectRequest {
    query: String,
    #[serde(default = "default_k")]
    k: usize,
}

#[derive(Debug, Serialize)]
struct ReflectResponse {
    query: String,
    reflection: String,
    memory_count: usize,
}

async fn amem_reflect(
    State(state): State<AppState>,
    Json(request): Json<ReflectRequest>,
) -> Result<Json<ReflectResponse>, (StatusCode, String)> {
    info!("A-mem reflection: {}", request.query);

    let amem = state.amem_system.lock().await;

    let reflection = amem
        .reflect(&request.query, request.k)
        .await
        .map_err(|e| {
            error!("A-mem reflection failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    Ok(Json(ReflectResponse {
        query: request.query,
        reflection,
        memory_count: request.k,
    }))
}

async fn amem_get_memory(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let memory_id = MemoryId(id.clone());
    let amem = state.amem_system.lock().await;

    let memory = amem
        .get_memory(&memory_id)
        .map_err(|e| {
            error!("Failed to get memory {}: {}", id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    match memory {
        Some(m) => Ok(Json(m)),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("Memory {} not found", id),
        )),
    }
}

async fn amem_delete_memory(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let memory_id = MemoryId(id.clone());
    let mut amem = state.amem_system.lock().await;

    amem.delete_memory(&memory_id)
        .map_err(|e| {
            error!("Failed to delete memory {}: {}", id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    Ok(Json(serde_json::json!({
        "message": format!("Memory {} deleted", id)
    })))
}

#[derive(Debug, Deserialize)]
struct GraphParams {
    #[serde(default = "default_graph_limit")]
    limit: usize,
}

fn default_graph_limit() -> usize {
    100
}

async fn amem_get_graph(
    State(state): State<AppState>,
    Query(params): Query<GraphParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("Generating A-mem knowledge graph");

    let amem = state.amem_system.lock().await;

    let graph = amem
        .get_memory_graph(params.limit)
        .map_err(|e| {
            error!("Failed to generate graph: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    Ok(Json(graph))
}

