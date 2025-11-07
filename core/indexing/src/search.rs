use anyhow::Result;
use memory_layer_schemas::{Memory, MemoryId};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info};

/// Search engine with BM25, embeddings, and recency ranking
pub struct SearchEngine {
    conn: Connection,
}

impl SearchEngine {
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        info!("Search engine initialized");
        Ok(Self { conn })
    }

    /// Hybrid search combining BM25 and recency
    pub fn search(
        &self,
        query: &str,
        limit: usize,
        recency_weight: f32,
    ) -> Result<Vec<ScoredMemory>> {
        debug!("Searching for: {} (limit: {})", query, limit);

        // Use FTS5 for BM25 ranking
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.kind, m.topic, m.text, m.snippet_title, m.snippet_text,
                    m.snippet_loc, m.snippet_language, m.entities, m.provenance,
                    m.created_at, m.ttl, rank
             FROM memories m
             JOIN memories_fts fts ON m.rowid = fts.rowid
             WHERE memories_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let results: Vec<(Memory, f64)> = stmt
            .query_map(params![query, limit * 2], |row| {
                let entities_json: String = row.get(8)?;
                let provenance_json: String = row.get(9)?;

                let entities: Vec<String> =
                    serde_json::from_str(&entities_json).unwrap_or_default();
                let provenance: Vec<memory_layer_schemas::TurnId> =
                    serde_json::from_str(&provenance_json).unwrap_or_default();

                let snippet_title: Option<String> = row.get(4)?;
                let snippet = if let Some(title) = snippet_title {
                    Some(memory_layer_schemas::Snippet {
                        title,
                        text: row.get(5)?,
                        loc: row.get(6)?,
                        language: row.get(7)?,
                    })
                } else {
                    None
                };

                let created_at: String = row.get(10)?;
                let bm25_score: f64 = row.get(12)?;

                Ok((
                    Memory {
                        id: MemoryId(row.get(0)?),
                        kind: memory_layer_schemas::MemoryKind::Fact, // Parse properly
                        topic: row.get(2)?,
                        text: row.get(3)?,
                        snippet,
                        entities,
                        provenance,
                        created_at,
                        ttl: row.get::<_, Option<i64>>(11)?.map(|t| t as u64),
                    },
                    bm25_score,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Apply recency boost and convert to ScoredMemory
        let now = chrono::Utc::now().timestamp();
        let mut scored_results: Vec<ScoredMemory> = results
            .into_iter()
            .map(|(memory, bm25_score)| {
                let created_timestamp = chrono::DateTime::parse_from_rfc3339(&memory.created_at)
                    .map(|dt| dt.timestamp())
                    .unwrap_or(0);

                // Exponential decay: newer items get higher scores
                let age_seconds = (now - created_timestamp) as f32;
                let age_days = age_seconds / 86400.0;
                let recency_score = (-age_days / 30.0).exp(); // Decay over ~30 days

                let final_score =
                    (1.0 - recency_weight) * (bm25_score as f32) + recency_weight * recency_score;

                ScoredMemory {
                    memory,
                    score: final_score as f64,
                }
            })
            .collect();

        // Re-sort by combined score
        scored_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Limit results
        scored_results.truncate(limit);

        debug!("Found {} results", scored_results.len());
        Ok(scored_results)
    }

    /// Search by topic with recency ranking
    pub fn search_by_topic(&self, topic: &str, limit: usize) -> Result<Vec<Memory>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, topic, text, snippet_title, snippet_text,
                    snippet_loc, snippet_language, entities, provenance,
                    created_at, ttl
             FROM memories
             WHERE topic = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;

        let memories = stmt
            .query_map(params![topic, limit], |row| {
                let entities_json: String = row.get(8)?;
                let provenance_json: String = row.get(9)?;

                let entities: Vec<String> =
                    serde_json::from_str(&entities_json).unwrap_or_default();
                let provenance: Vec<memory_layer_schemas::TurnId> =
                    serde_json::from_str(&provenance_json).unwrap_or_default();

                let snippet_title: Option<String> = row.get(4)?;
                let snippet = if let Some(title) = snippet_title {
                    Some(memory_layer_schemas::Snippet {
                        title,
                        text: row.get(5)?,
                        loc: row.get(6)?,
                        language: row.get(7)?,
                    })
                } else {
                    None
                };

                Ok(Memory {
                    id: MemoryId(row.get(0)?),
                    kind: memory_layer_schemas::MemoryKind::Fact,
                    topic: row.get(2)?,
                    text: row.get(3)?,
                    snippet,
                    entities,
                    provenance,
                    created_at: row.get(10)?,
                    ttl: row.get::<_, Option<i64>>(11)?.map(|t| t as u64),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(memories)
    }

    /// Get all unique topics
    pub fn get_topics(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT topic FROM memories ORDER BY topic")?;

        let topics = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(topics)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScoredMemory {
    pub memory: Memory,
    pub score: f64,
}

/// Embedding stub for future integration
pub struct EmbeddingEngine {
    cache: HashMap<String, Vec<f32>>,
}

impl EmbeddingEngine {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Generate embedding for text (stub - will use sentence-transformers via PyO3)
    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        // For MVP, return a simple hash-based embedding
        // In production, this will call Python's sentence-transformers
        if let Some(cached) = self.cache.get(text) {
            return Ok(cached.clone());
        }

        let embedding = self.simple_embed(text);
        self.cache.insert(text.to_string(), embedding.clone());

        Ok(embedding)
    }

    fn simple_embed(&self, text: &str) -> Vec<f32> {
        // Simple character-based embedding for testing
        // Production will use all-MiniLM-L6-v2 or similar
        let mut embedding = vec![0.0; 384]; // Standard size for MiniLM

        for (i, ch) in text.chars().take(384).enumerate() {
            embedding[i] = (ch as u32 % 256) as f32 / 256.0;
        }

        embedding
    }

    /// Calculate cosine similarity between two embeddings
    pub fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_generation() {
        let mut engine = EmbeddingEngine::new();
        let embedding = engine.embed("test text").unwrap();
        assert_eq!(embedding.len(), 384);
    }

    #[test]
    fn test_cosine_similarity() {
        let engine = EmbeddingEngine::new();
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];

        assert!((engine.cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
        assert!((engine.cosine_similarity(&a, &c) - 0.0).abs() < 0.001);
    }
}
