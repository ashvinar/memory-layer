/// A-mem: Advanced Memory Management System
/// Based on https://github.com/agiresearch/A-mem
///
/// This module implements A-mem's key features:
/// 1. Memory Evolution - Automatic refinement when similar memories are added
/// 2. Vector Embeddings - Semantic search using embeddings
/// 3. Automatic Linking - Create connections between related memories
/// 4. LLM-powered Reflection - Enrich memories with context
/// 5. Zettelkasten Organization - Better tagging and categorization

use anyhow::Result;
use chrono::Utc;
use memory_layer_schemas::{AgenticEvolution, AgenticLink, AgenticMemory, MemoryId, generate_memory_id};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::{debug, info};
use async_trait::async_trait;

/// ChromaDB-like vector store interface (simplified for local use)
pub trait VectorStore: Send + Sync {
    fn add(&mut self, id: &str, embedding: Vec<f32>, metadata: HashMap<String, String>) -> Result<()>;
    fn search(&self, query_embedding: Vec<f32>, k: usize) -> Result<Vec<(String, f32, HashMap<String, String>)>>;
    fn update(&mut self, id: &str, embedding: Vec<f32>, metadata: HashMap<String, String>) -> Result<()>;
    fn delete(&mut self, id: &str) -> Result<()>;
}

/// LLM interface for memory enrichment and reflection
#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn enrich_memory(&self, content: &str, context: &str) -> Result<MemoryEnrichment>;
    async fn reflect(&self, memories: Vec<&AgenticMemory>, query: &str) -> Result<String>;
    async fn extract_keywords(&self, content: &str) -> Result<Vec<String>>;
    async fn suggest_links(&self, source: &AgenticMemory, candidates: Vec<&AgenticMemory>) -> Result<Vec<SuggestedLink>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEnrichment {
    pub tags: Vec<String>,
    pub keywords: Vec<String>,
    pub category: Option<String>,
    pub summary: String,
    pub context_description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedLink {
    pub target_id: MemoryId,
    pub strength: f32,
    pub rationale: String,
}

/// A-mem compatible memory system
pub struct AMemSystem {
    conn: Connection,
    vector_store: Box<dyn VectorStore>,
    llm_provider: Option<Box<dyn LLMProvider>>,
    embedding_engine: Box<dyn EmbeddingEngine>,
    evolution_threshold: f32,
    link_threshold: f32,
}

impl AMemSystem {
    pub fn new<P: AsRef<Path>>(
        db_path: P,
        vector_store: Box<dyn VectorStore>,
        embedding_engine: Box<dyn EmbeddingEngine>,
        llm_provider: Option<Box<dyn LLMProvider>>,
    ) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // Ensure tables exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS amem_memories (
                memory_id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                context TEXT NOT NULL,
                summary TEXT,
                keywords TEXT NOT NULL DEFAULT '[]',
                tags TEXT NOT NULL DEFAULT '[]',
                category TEXT,
                links TEXT NOT NULL DEFAULT '[]',
                retrieval_count INTEGER NOT NULL DEFAULT 0,
                last_accessed TEXT NOT NULL,
                created_at TEXT NOT NULL,
                evolution_history TEXT NOT NULL DEFAULT '[]',
                embedding_version INTEGER DEFAULT 1,
                metadata TEXT DEFAULT '{}'
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS amem_evolution_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                memory_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                trigger_memory_id TEXT,
                change_type TEXT NOT NULL,
                old_value TEXT,
                new_value TEXT,
                rationale TEXT,
                FOREIGN KEY(memory_id) REFERENCES amem_memories(memory_id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_amem_last_accessed
             ON amem_memories(last_accessed DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_amem_category
             ON amem_memories(category)",
            [],
        )?;

        Ok(Self {
            conn,
            vector_store,
            llm_provider,
            embedding_engine,
            evolution_threshold: 0.75, // Similarity threshold for triggering evolution
            link_threshold: 0.65,      // Similarity threshold for creating links
        })
    }

    /// Add a new memory with automatic evolution and linking
    pub async fn add_memory(
        &mut self,
        content: String,
        context: Option<String>,
        tags: Option<Vec<String>>,
        category: Option<String>,
    ) -> Result<MemoryId> {
        let memory_id = generate_memory_id();
        let now = Utc::now().to_rfc3339();
        let context = context.unwrap_or_else(|| "General context".to_string());

        info!("Adding new memory: {}", memory_id);

        // Generate embedding for the content
        let embedding = self.embedding_engine.embed(&content)?;

        // Search for similar existing memories
        let similar_memories = self.vector_store.search(embedding.clone(), 10)?;

        // Enrich memory with LLM if available
        let enrichment = if let Some(ref llm) = self.llm_provider {
            llm.enrich_memory(&content, &context).await?
        } else {
            // Fallback to simple extraction
            MemoryEnrichment {
                tags: tags.clone().unwrap_or_default(),
                keywords: self.extract_keywords_simple(&content),
                category: category.clone(),
                summary: content.chars().take(200).collect(),
                context_description: context.clone(),
            }
        };

        // Create initial memory
        let mut memory = AgenticMemory {
            id: memory_id.clone(),
            content: content.clone(),
            context: enrichment.context_description.clone(),
            keywords: enrichment.keywords.clone(),
            tags: enrichment.tags.clone(),
            category: enrichment.category.clone(),
            links: Vec::new(),
            retrieval_count: 0,
            last_accessed: now.clone(),
            created_at: now.clone(),
            evolution_history: vec![
                AgenticEvolution {
                    timestamp: now.clone(),
                    summary: "Initial memory creation".to_string(),
                    changes: None,
                }
            ],
        };

        // Trigger evolution in similar memories
        for (similar_id, similarity, _metadata) in &similar_memories {
            if similarity > &self.evolution_threshold {
                debug!("Triggering evolution for {} due to similarity: {}", similar_id, similarity);
                self.evolve_memory(similar_id, &memory).await?;
            }
        }

        // Create links to related memories
        if let Some(ref llm) = self.llm_provider {
            let similar_mems = self.load_memories_by_ids(
                similar_memories.iter()
                    .filter(|(_, sim, _)| sim > &self.link_threshold)
                    .map(|(id, _, _)| id.clone())
                    .collect()
            )?;

            let suggested_links = llm.suggest_links(&memory, similar_mems.iter().collect()).await?;

            for link in suggested_links {
                if link.strength > self.link_threshold {
                    memory.links.push(AgenticLink {
                        target: link.target_id,
                        strength: link.strength,
                        rationale: Some(link.rationale),
                    });
                }
            }
        } else {
            // Fallback: Create simple links based on similarity
            for (similar_id, similarity, _) in similar_memories {
                if similarity > self.link_threshold {
                    memory.links.push(AgenticLink {
                        target: MemoryId(similar_id),
                        strength: similarity,
                        rationale: Some(format!("Semantic similarity: {:.2}", similarity)),
                    });
                }
            }
        }

        // Store in database
        self.store_memory(&memory)?;

        // Add to vector store
        let mut metadata = HashMap::new();
        metadata.insert("context".to_string(), memory.context.clone());
        metadata.insert("category".to_string(), memory.category.clone().unwrap_or_default());
        metadata.insert("tags".to_string(), memory.tags.join(","));

        self.vector_store.add(&memory.id.0, embedding, metadata)?;

        info!("Memory {} added with {} links", memory_id, memory.links.len());

        Ok(memory_id)
    }

    /// Evolve an existing memory based on new information
    async fn evolve_memory(&mut self, memory_id: &str, trigger: &AgenticMemory) -> Result<()> {
        let Some(mut memory) = self.get_memory(&MemoryId(memory_id.to_string()))? else {
            return Ok(());
        };

        info!("Evolving memory {} triggered by {}", memory_id, trigger.id);

        // Extract new insights from the trigger memory
        let mut new_keywords = HashSet::<String>::from_iter(memory.keywords.clone());
        let mut new_tags = HashSet::<String>::from_iter(memory.tags.clone());

        // Add relevant keywords and tags from trigger
        for keyword in &trigger.keywords {
            if trigger.content.contains(keyword) || memory.content.contains(keyword) {
                new_keywords.insert(keyword.clone());
            }
        }

        for tag in &trigger.tags {
            new_tags.insert(tag.clone());
        }

        // Update category if not set
        let new_category = memory.category.clone().or(trigger.category.clone());

        // Create evolution record
        let evolution = AgenticEvolution {
            timestamp: Utc::now().to_rfc3339(),
            summary: format!("Evolved from interaction with memory {}", trigger.id),
            changes: Some(vec![
                format!("Added {} new keywords", new_keywords.len()),
                format!("Added {} new tags", new_tags.len()),
            ]),
        };

        memory.keywords = new_keywords.into_iter().collect();
        memory.tags = new_tags.into_iter().collect();
        memory.category = new_category;
        memory.evolution_history.push(evolution);

        // Check for new link opportunities
        let should_link = !memory.links.iter().any(|l| l.target == trigger.id);
        if should_link {
            memory.links.push(AgenticLink {
                target: trigger.id.clone(),
                strength: 0.8, // High strength for evolution-based links
                rationale: Some("Memories evolved together".to_string()),
            });
        }

        // Update in database
        self.update_memory(&memory)?;

        // Update vector store metadata
        let mut metadata = HashMap::new();
        metadata.insert("context".to_string(), memory.context.clone());
        metadata.insert("category".to_string(), memory.category.clone().unwrap_or_default());
        metadata.insert("tags".to_string(), memory.tags.join(","));

        let embedding = self.embedding_engine.embed(&memory.content)?;
        self.vector_store.update(&memory.id.0, embedding, metadata)?;

        Ok(())
    }

    /// Search memories using A-mem's semantic search
    pub fn search_agentic(&self, query: &str, k: usize) -> Result<Vec<AgenticMemory>> {
        let query_embedding = self.embedding_engine.embed(query)?;
        let results = self.vector_store.search(query_embedding, k)?;

        let mut memories = Vec::new();
        for (id, _score, _metadata) in results {
            if let Some(memory) = self.get_memory(&MemoryId(id))? {
                memories.push(memory);
            }
        }

        Ok(memories)
    }

    /// Reflect on memories to generate insights
    pub async fn reflect(&self, query: &str, k: usize) -> Result<String> {
        let memories = self.search_agentic(query, k)?;

        if let Some(ref llm) = self.llm_provider {
            let memory_refs: Vec<&AgenticMemory> = memories.iter().collect();
            llm.reflect(memory_refs, query).await
        } else {
            // Fallback: Simple concatenation
            Ok(memories.iter()
                .map(|m| format!("- {}: {}", m.context, m.content.chars().take(100).collect::<String>()))
                .collect::<Vec<_>>()
                .join("\n"))
        }
    }

    /// Store memory in database
    fn store_memory(&self, memory: &AgenticMemory) -> Result<()> {
        self.conn.execute(
            "INSERT INTO amem_memories (
                memory_id, content, context, summary, keywords, tags, category,
                links, retrieval_count, last_accessed, created_at, evolution_history
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                memory.id.0,
                memory.content,
                memory.context,
                memory.content.chars().take(200).collect::<String>(), // Simple summary
                serde_json::to_string(&memory.keywords)?,
                serde_json::to_string(&memory.tags)?,
                memory.category,
                serde_json::to_string(&memory.links)?,
                memory.retrieval_count as i64,
                memory.last_accessed,
                memory.created_at,
                serde_json::to_string(&memory.evolution_history)?,
            ],
        )?;
        Ok(())
    }

    /// Update existing memory
    fn update_memory(&self, memory: &AgenticMemory) -> Result<()> {
        self.conn.execute(
            "UPDATE amem_memories SET
                content = ?2, context = ?3, keywords = ?4, tags = ?5,
                category = ?6, links = ?7, evolution_history = ?8
            WHERE memory_id = ?1",
            params![
                memory.id.0,
                memory.content,
                memory.context,
                serde_json::to_string(&memory.keywords)?,
                serde_json::to_string(&memory.tags)?,
                memory.category,
                serde_json::to_string(&memory.links)?,
                serde_json::to_string(&memory.evolution_history)?,
            ],
        )?;
        Ok(())
    }

    /// Get a memory by ID
    pub fn get_memory(&self, id: &MemoryId) -> Result<Option<AgenticMemory>> {
        let record = self.conn.query_row(
            "SELECT content, context, keywords, tags, category, links, retrieval_count,
                    last_accessed, created_at, evolution_history
             FROM amem_memories WHERE memory_id = ?1",
            params![id.0],
            |row| {
                let keywords: String = row.get(2)?;
                let tags: String = row.get(3)?;
                let links: String = row.get(5)?;
                let evolution: String = row.get(9)?;

                Ok(AgenticMemory {
                    id: id.clone(),
                    content: row.get(0)?,
                    context: row.get(1)?,
                    keywords: serde_json::from_str(&keywords).unwrap_or_default(),
                    tags: serde_json::from_str(&tags).unwrap_or_default(),
                    category: row.get(4)?,
                    links: serde_json::from_str(&links).unwrap_or_default(),
                    retrieval_count: row.get::<_, i64>(6)? as u32,
                    last_accessed: row.get(7)?,
                    created_at: row.get(8)?,
                    evolution_history: serde_json::from_str(&evolution).unwrap_or_default(),
                })
            },
        ).optional()?;

        // Update access timestamp
        if record.is_some() {
            self.conn.execute(
                "UPDATE amem_memories SET retrieval_count = retrieval_count + 1,
                 last_accessed = ?2 WHERE memory_id = ?1",
                params![id.0, Utc::now().to_rfc3339()],
            )?;
        }

        Ok(record)
    }

    /// Load multiple memories by IDs
    fn load_memories_by_ids(&self, ids: Vec<String>) -> Result<Vec<AgenticMemory>> {
        let mut memories = Vec::new();
        for id in ids {
            if let Some(memory) = self.get_memory(&MemoryId(id))? {
                memories.push(memory);
            }
        }
        Ok(memories)
    }

    /// Simple keyword extraction (fallback when no LLM)
    fn extract_keywords_simple(&self, content: &str) -> Vec<String> {
        let stop_words = HashSet::from([
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
            "of", "with", "by", "from", "as", "is", "was", "are", "were", "been",
        ]);

        let words: Vec<String> = content
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() > 3 && !stop_words.contains(w))
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .take(10)
            .collect();

        words
    }

    /// Delete a memory
    pub fn delete_memory(&mut self, id: &MemoryId) -> Result<()> {
        self.conn.execute(
            "DELETE FROM amem_memories WHERE memory_id = ?1",
            params![id.0],
        )?;

        self.vector_store.delete(&id.0)?;

        Ok(())
    }

    /// Get memory graph for visualization
    pub fn get_memory_graph(&self, limit: usize) -> Result<MemoryGraph> {
        let mut stmt = self.conn.prepare(
            "SELECT memory_id, content, context, keywords, tags, category, links,
                    retrieval_count, last_accessed, created_at
             FROM amem_memories
             ORDER BY last_accessed DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit], |row| {
            let keywords: String = row.get(3)?;
            let tags: String = row.get(4)?;
            let links: String = row.get(6)?;

            Ok(MemoryNode {
                id: MemoryId(row.get(0)?),
                content: row.get(1)?,
                context: row.get(2)?,
                keywords: serde_json::from_str(&keywords).unwrap_or_default(),
                tags: serde_json::from_str(&tags).unwrap_or_default(),
                category: row.get(5)?,
                links: serde_json::from_str(&links).unwrap_or_default(),
                retrieval_count: row.get::<_, i64>(7)? as u32,
                last_accessed: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?;

        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        for node in rows {
            let node = node?;

            // Create edges from links
            for link in &node.links {
                edges.push(MemoryEdge {
                    source: node.id.clone(),
                    target: link.target.clone(),
                    strength: link.strength,
                    rationale: link.rationale.clone(),
                });
            }

            nodes.push(node);
        }

        Ok(MemoryGraph { nodes, edges })
    }
}

// Simple embedding engine trait
pub trait EmbeddingEngine: Send + Sync {
    fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

// In-memory vector store for testing
pub struct InMemoryVectorStore {
    embeddings: HashMap<String, (Vec<f32>, HashMap<String, String>)>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self {
            embeddings: HashMap::new(),
        }
    }
}

impl VectorStore for InMemoryVectorStore {
    fn add(&mut self, id: &str, embedding: Vec<f32>, metadata: HashMap<String, String>) -> Result<()> {
        self.embeddings.insert(id.to_string(), (embedding, metadata));
        Ok(())
    }

    fn search(&self, query_embedding: Vec<f32>, k: usize) -> Result<Vec<(String, f32, HashMap<String, String>)>> {
        let mut scores: Vec<(String, f32, HashMap<String, String>)> = Vec::new();

        for (id, (embedding, metadata)) in &self.embeddings {
            let score = cosine_similarity(&query_embedding, embedding);
            scores.push((id.clone(), score, metadata.clone()));
        }

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scores.truncate(k);

        Ok(scores)
    }

    fn update(&mut self, id: &str, embedding: Vec<f32>, metadata: HashMap<String, String>) -> Result<()> {
        self.add(id, embedding, metadata)
    }

    fn delete(&mut self, id: &str) -> Result<()> {
        self.embeddings.remove(id);
        Ok(())
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        0.0
    } else {
        dot / (mag_a * mag_b)
    }
}

#[derive(Debug, Serialize)]
pub struct MemoryNode {
    pub id: MemoryId,
    pub content: String,
    pub context: String,
    pub keywords: Vec<String>,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub links: Vec<AgenticLink>,
    pub retrieval_count: u32,
    pub last_accessed: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct MemoryEdge {
    pub source: MemoryId,
    pub target: MemoryId,
    pub strength: f32,
    pub rationale: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MemoryGraph {
    pub nodes: Vec<MemoryNode>,
    pub edges: Vec<MemoryEdge>,
}