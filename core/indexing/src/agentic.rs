use anyhow::Result;
use chrono::Utc;
use memory_layer_schemas::{AgenticEvolution, AgenticLink, AgenticMemory, MemoryId};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

pub struct AgenticMemoryBase {
    conn: Connection,
}

impl AgenticMemoryBase {
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        Ok(Self { conn })
    }

    pub fn list_recent(&self, limit: usize) -> Result<Vec<AgenticMemorySummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT memory_id, content, context, tags, keywords, retrieval_count, last_accessed
             FROM agentic_memories
             ORDER BY last_accessed DESC
             LIMIT ?1",
        )?;

        let rows = stmt
            .query_map(params![limit as i64], |row| {
                let tags_json: String = row.get(3)?;
                let keywords_json: String = row.get(4)?;

                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let keywords: Vec<String> =
                    serde_json::from_str(&keywords_json).unwrap_or_default();

                let content: String = row.get(1)?;
                let preview = content
                    .chars()
                    .take(160)
                    .collect::<String>()
                    .trim()
                    .to_string();

                Ok(AgenticMemorySummary {
                    id: MemoryId(row.get::<_, String>(0)?),
                    context: row.get(2)?,
                    preview,
                    tags,
                    keywords,
                    retrieval_count: row.get::<_, i64>(5)? as u32,
                    last_accessed: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<AgenticMemorySummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT m.memory_id, m.content, m.context, m.tags, m.keywords, m.retrieval_count,
                    m.last_accessed, rank
             FROM agentic_memories m
             JOIN agentic_memories_fts fts ON m.rowid = fts.rowid
             WHERE agentic_memories_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![query, limit as i64], |row| {
                let tags_json: String = row.get(3)?;
                let keywords_json: String = row.get(4)?;

                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let keywords: Vec<String> =
                    serde_json::from_str(&keywords_json).unwrap_or_default();

                let content: String = row.get(1)?;
                let preview = content
                    .chars()
                    .take(160)
                    .collect::<String>()
                    .trim()
                    .to_string();

                Ok(AgenticMemorySummary {
                    id: MemoryId(row.get::<_, String>(0)?),
                    context: row.get(2)?,
                    preview,
                    tags,
                    keywords,
                    retrieval_count: row.get::<_, i64>(5)? as u32,
                    last_accessed: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    pub fn get(&self, id: &MemoryId) -> Result<Option<AgenticMemory>> {
        let record = self
            .conn
            .query_row(
                "SELECT content, context, keywords, tags, category, links, retrieval_count,
                        last_accessed, evolution_history, created_at
                 FROM agentic_memories
                 WHERE memory_id = ?1",
                params![id.0.as_str()],
                |row| {
                    let keywords_json: String = row.get(2)?;
                    let tags_json: String = row.get(3)?;
                    let links_json: String = row.get(5)?;
                    let evo_json: String = row.get(8)?;

                    let keywords: Vec<String> =
                        serde_json::from_str(&keywords_json).unwrap_or_default();
                    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                    let links: Vec<AgenticLink> =
                        serde_json::from_str(&links_json).unwrap_or_default();
                    let evolution_history: Vec<AgenticEvolution> =
                        serde_json::from_str(&evo_json).unwrap_or_default();

                    Ok(AgenticMemory {
                        id: id.clone(),
                        content: row.get(0)?,
                        context: row.get(1)?,
                        keywords,
                        tags,
                        category: row.get(4)?,
                        links,
                        retrieval_count: row.get::<_, i64>(6)? as u32,
                        last_accessed: row.get(7)?,
                        created_at: row.get(9)?,
                        evolution_history,
                    })
                },
            )
            .optional()?;

        Ok(record)
    }

    pub fn record_access(&self, id: &MemoryId) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE agentic_memories
             SET retrieval_count = retrieval_count + 1,
                 last_accessed = ?2
             WHERE memory_id = ?1",
            params![id.0.as_str(), now],
        )?;
        Ok(())
    }

    pub fn graph(&self, limit: usize) -> Result<AgenticGraph> {
        let mut stmt = self.conn.prepare(
            "SELECT memory_id, content, context, keywords, tags, category,
                    retrieval_count, last_accessed, created_at, links
             FROM agentic_memories
             ORDER BY last_accessed DESC
             LIMIT ?1",
        )?;

        let mut nodes = Vec::new();
        let mut node_index = HashMap::new();

        let rows = stmt.query_map(params![limit as i64], |row| {
            let id = MemoryId(row.get::<_, String>(0)?);
            let keywords_json: String = row.get(3)?;
            let tags_json: String = row.get(4)?;

            let keywords: Vec<String> = serde_json::from_str(&keywords_json).unwrap_or_default();
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

            Ok(AgenticGraphNode {
                id,
                content: row.get(1)?,
                context: row.get(2)?,
                keywords,
                tags,
                category: row.get(5)?,
                retrieval_count: row.get::<_, i64>(6)? as u32,
                last_accessed: row.get(7)?,
                created_at: row.get(8)?,
                links_json: row.get(9)?,
            })
        })?;

        for node in rows {
            let node = node?;
            node_index.insert(node.id.clone(), nodes.len());
            nodes.push(node);
        }

        let mut edges = Vec::new();
        for node in &nodes {
            let links: Vec<AgenticLink> =
                serde_json::from_str(&node.links_json).unwrap_or_default();
            for link in links {
                if node_index.contains_key(&link.target) {
                    edges.push(AgenticGraphEdge {
                        source: node.id.clone(),
                        target: link.target,
                        strength: link.strength,
                        rationale: link.rationale.clone(),
                    });
                }
            }
        }

        let exported_nodes = nodes
            .into_iter()
            .map(|node| AgenticGraphNodeExport {
                id: node.id,
                content: node.content,
                context: node.context,
                keywords: node.keywords,
                tags: node.tags,
                category: node.category,
                retrieval_count: node.retrieval_count,
                last_accessed: node.last_accessed,
                created_at: node.created_at,
            })
            .collect();

        Ok(AgenticGraph {
            nodes: exported_nodes,
            edges,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct AgenticMemorySummary {
    pub id: MemoryId,
    pub context: String,
    pub preview: String,
    pub tags: Vec<String>,
    pub keywords: Vec<String>,
    pub retrieval_count: u32,
    pub last_accessed: String,
}

#[derive(Debug, Serialize)]
pub struct AgenticGraph {
    pub nodes: Vec<AgenticGraphNodeExport>,
    pub edges: Vec<AgenticGraphEdge>,
}

#[derive(Debug, Serialize)]
pub struct AgenticGraphNodeExport {
    pub id: MemoryId,
    pub content: String,
    pub context: String,
    pub keywords: Vec<String>,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub retrieval_count: u32,
    pub last_accessed: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct AgenticGraphEdge {
    pub source: MemoryId,
    pub target: MemoryId,
    pub strength: f32,
    pub rationale: Option<String>,
}

struct AgenticGraphNode {
    id: MemoryId,
    content: String,
    context: String,
    keywords: Vec<String>,
    tags: Vec<String>,
    category: Option<String>,
    retrieval_count: u32,
    last_accessed: String,
    created_at: String,
    links_json: String,
}
