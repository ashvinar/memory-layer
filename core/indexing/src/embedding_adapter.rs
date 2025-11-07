/// Adapter to connect the existing EmbeddingEngine with A-mem's EmbeddingEngine trait

use anyhow::Result;
use crate::search::EmbeddingEngine as SearchEmbeddingEngine;
use std::sync::RwLock;

/// Adapter that wraps the existing EmbeddingEngine for A-mem
pub struct EmbeddingAdapter {
    inner: RwLock<SearchEmbeddingEngine>,
}

impl EmbeddingAdapter {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(SearchEmbeddingEngine::new()),
        }
    }
}

impl crate::amem::EmbeddingEngine for EmbeddingAdapter {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.inner.write().unwrap().embed(text)
    }
}