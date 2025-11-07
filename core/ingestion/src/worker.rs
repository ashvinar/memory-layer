use anyhow::Result;
use memory_layer_schemas::Turn;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

use crate::{Database, MemoryExtractor};

/// Background worker that processes turns asynchronously
pub struct IngestionWorker {
    db: Arc<Mutex<Database>>,
    extractor: Arc<MemoryExtractor>,
    receiver: mpsc::UnboundedReceiver<Turn>,
}

impl IngestionWorker {
    /// Create a new ingestion worker
    pub fn new(
        db: Arc<Mutex<Database>>,
        extractor: Arc<MemoryExtractor>,
        receiver: mpsc::UnboundedReceiver<Turn>,
    ) -> Self {
        Self {
            db,
            extractor,
            receiver,
        }
    }

    /// Start the worker loop - processes turns from the channel
    /// This runs indefinitely until the channel is closed
    pub async fn run(mut self) {
        info!("Ingestion worker started");

        while let Some(turn) = self.receiver.recv().await {
            if let Err(e) = self.process_turn(turn).await {
                error!("Failed to process turn: {}", e);
                // Continue processing even on error - we don't want one failure to stop the worker
            }
        }

        warn!("Ingestion worker stopped - channel closed");
    }

    /// Process a single turn: insert turn, extract memories, and insert them into the database
    async fn process_turn(&self, turn: Turn) -> Result<()> {
        info!("Processing turn: {}", turn.id);

        // Lock the database once for all operations
        let db = self.db.lock().await;

        // First, insert the turn itself
        if let Err(e) = db.insert_turn(&turn) {
            error!("Failed to insert turn {}: {}", turn.id, e);
            return Err(e);
        }

        // Extract memories from the turn
        let memories = self.extractor.extract(&turn)?;

        info!(
            "Extracted {} memories from turn {}",
            memories.len(),
            turn.id
        );

        // Insert memories into database
        for memory in &memories {
            if let Err(e) = db.insert_memory(memory) {
                error!("Failed to insert memory {}: {}", memory.id, e);
                // Continue with other memories even if one fails
                continue;
            }

            if let Err(e) = db.upsert_agentic_memory(memory) {
                error!("Failed to capture agentic metadata for memory {}: {}", memory.id, e);
                // Non-critical error, continue
            }
        }

        info!("Successfully processed turn: {}", turn.id);
        Ok(())
    }
}
