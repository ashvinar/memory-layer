use anyhow::Result;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use memory_layer_composer::Composer;
use memory_layer_schemas::{ContextRequest, UndoRequest, UndoResponse};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info, Level};
use tracing_subscriber;

#[derive(Clone)]
struct AppState {
    composer: Arc<Mutex<Composer>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Memory Layer Composer Service v0.1.0");

    let composer = Composer::new();

    let state = AppState {
        composer: Arc::new(Mutex::new(composer)),
    };

    // CORS layer for browser extensions
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/v1/context", post(get_context))
        .route("/v1/undo", post(undo_context))
        .layer(cors)
        .with_state(state);

    // Start HTTP server
    let http_addr = "127.0.0.1:21955";
    info!("Starting HTTP server on http://{}", http_addr);
    info!("Provider endpoint: http://{}/v1/context", http_addr);

    let listener = tokio::net::TcpListener::bind(http_addr).await?;

    // TODO: Add Unix socket listener as well
    // let sock_path = format!("{}/Library/Application Support/MemoryLayer/context.sock",
    //                         std::env::var("HOME")?);
    // info!("Unix socket endpoint: {}", sock_path);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "service": "composer",
        "status": "healthy",
        "version": "0.1.0"
    }))
}

async fn get_context(
    State(state): State<AppState>,
    Json(request): Json<ContextRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(
        "Context request: topic={:?}, budget={}",
        request.topic_hint, request.budget_tokens
    );

    let mut composer = state.composer.lock().await;

    let capsule = composer.compose(&request).await.map_err(|e| {
        error!("Failed to compose context: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    info!(
        "Returning capsule: {} ({} tokens)",
        capsule.capsule_id,
        capsule.token_count.unwrap_or(0)
    );

    Ok(Json(capsule))
}

async fn undo_context(
    State(_state): State<AppState>,
    Json(request): Json<UndoRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(
        "Undo request: capsule={}, thread={}",
        request.capsule_id, request.thread_key
    );

    // For MVP, just acknowledge the undo
    // In production, this would remove the capsule from active contexts
    // and potentially notify connected clients

    Ok(Json(UndoResponse {
        success: true,
        message: Some("Context undone".to_string()),
    }))
}
