use anyhow::Result;
use chrono::Utc;
use memory_layer_schemas::{
    AgenticEvolution, AgenticLink, AgenticMemory, Memory, MemoryId, MemoryKind, Snippet, ThreadId,
    Turn, TurnId,
};
use regex::Regex;
use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::{debug, info};

const STOPWORDS: &[&str] = &[
    "the",
    "and",
    "that",
    "this",
    "with",
    "from",
    "have",
    "would",
    "there",
    "could",
    "should",
    "about",
    "after",
    "before",
    "while",
    "since",
    "where",
    "which",
    "into",
    "using",
    "also",
    "because",
    "these",
    "those",
    "been",
    "being",
    "were",
    "does",
    "done",
    "make",
    "made",
    "when",
    "then",
    "than",
    "your",
    "their",
    "them",
    "they",
    "what",
    "ever",
    "over",
    "just",
    "more",
    "only",
    "each",
    "such",
    "very",
    "much",
    "like",
    "into",
    "onto",
    "upon",
    "ourselves",
    "himself",
    "herself",
    "itself",
];

const MAX_AGENTIC_LINKS: usize = 8;

/// Direction for querying memory relations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationDirection {
    /// Only outgoing relations (this memory → others)
    Outgoing,
    /// Only incoming relations (others → this memory)
    Incoming,
    /// Both incoming and outgoing relations
    Both,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Initialize database with schema and FTS5 tables
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;

        let db = Self { conn };
        db.init_schema()?;

        info!("Database initialized");
        Ok(db)
    }

    /// Check if a column exists in a table
    fn has_column(&self, table: &str, column: &str) -> Result<bool> {
        let query = format!("PRAGMA table_info({})", table);
        let mut stmt = self.conn.prepare(&query)?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(columns.contains(&column.to_string()))
    }

    /// Create all tables and indexes
    fn init_schema(&self) -> Result<()> {
        // Turns table (append-only conversation events)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS turns (
                id TEXT PRIMARY KEY,
                thread_id TEXT NOT NULL,
                ts_user TEXT NOT NULL,
                user_text TEXT NOT NULL,
                ts_ai TEXT,
                ai_text TEXT,
                source_app TEXT NOT NULL,
                source_url TEXT,
                source_path TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        // === HIERARCHY TABLES ===

        // Workspaces (top level)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS workspaces (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // Projects (belong to workspaces)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                status TEXT DEFAULT 'active',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Areas (belong to projects)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS areas (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Topics (belong to areas)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS topics (
                id TEXT PRIMARY KEY,
                area_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                is_index_note INTEGER DEFAULT 0,
                summary TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (area_id) REFERENCES areas(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // === RELATIONSHIP & VERSIONING TABLES ===

        // Memory relationships (typed links between memories)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS memory_relations (
                id TEXT PRIMARY KEY,
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                relation_type TEXT NOT NULL,
                rationale TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (source_id) REFERENCES memories(id) ON DELETE CASCADE,
                FOREIGN KEY (target_id) REFERENCES memories(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Memory versions (full history)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS memory_versions (
                id TEXT PRIMARY KEY,
                memory_id TEXT NOT NULL,
                content TEXT NOT NULL,
                version_number INTEGER NOT NULL,
                change_summary TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (memory_id) REFERENCES memories(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Index Notes (Zettelkasten hub notes)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS index_notes (
                id TEXT PRIMARY KEY,
                scope_type TEXT NOT NULL,
                scope_id TEXT NOT NULL,
                name TEXT NOT NULL,
                content TEXT NOT NULL,
                memory_count INTEGER DEFAULT 0,
                key_memories TEXT,
                tags TEXT,
                created_at TEXT NOT NULL,
                last_updated TEXT NOT NULL
            )",
            [],
        )?;

        // Progressive Summarization (layered note refinement)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS progressive_summaries (
                id TEXT PRIMARY KEY,
                memory_id TEXT NOT NULL,
                layer INTEGER NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (memory_id) REFERENCES memories(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Memories table (durable knowledge)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                topic TEXT NOT NULL,
                text TEXT NOT NULL,
                snippet_title TEXT,
                snippet_text TEXT,
                snippet_loc TEXT,
                snippet_language TEXT,
                entities TEXT,
                provenance TEXT NOT NULL,
                created_at TEXT NOT NULL,
                ttl INTEGER
            )",
            [],
        )?;

        // Add new columns to existing memories table (idempotent with IF NOT EXISTS equivalent)
        // SQLite doesn't have IF NOT EXISTS for ALTER COLUMN, so we check column existence
        let has_topic_id = self.has_column("memories", "topic_id")?;
        if !has_topic_id {
            self.conn.execute(
                "ALTER TABLE memories ADD COLUMN topic_id TEXT REFERENCES topics(id) ON DELETE SET NULL",
                [],
            )?;
            self.conn.execute(
                "ALTER TABLE memories ADD COLUMN importance TEXT DEFAULT 'normal'",
                [],
            )?;
            self.conn.execute(
                "ALTER TABLE memories ADD COLUMN status TEXT DEFAULT 'fleeting'",
                [],
            )?;
            self.conn.execute(
                "ALTER TABLE memories ADD COLUMN version INTEGER DEFAULT 1",
                [],
            )?;
            self.conn.execute(
                "ALTER TABLE memories ADD COLUMN superseded_by TEXT REFERENCES memories(id)",
                [],
            )?;
        }

        // Agentic memory base derived from A-mem concepts
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS agentic_memories (
                memory_id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                context TEXT NOT NULL,
                keywords TEXT NOT NULL,
                tags TEXT NOT NULL,
                category TEXT,
                links TEXT NOT NULL,
                retrieval_count INTEGER NOT NULL DEFAULT 0,
                last_accessed TEXT NOT NULL,
                evolution_history TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY(memory_id) REFERENCES memories(id) ON DELETE CASCADE
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS agentic_memories_fts USING fts5(
                content,
                context,
                keywords,
                tags,
                content='agentic_memories',
                content_rowid='rowid'
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS agentic_memories_ai AFTER INSERT ON agentic_memories BEGIN
                INSERT INTO agentic_memories_fts(rowid, content, context, keywords, tags)
                VALUES (new.rowid, new.content, new.context, new.keywords, new.tags);
            END",
            [],
        )?;

        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS agentic_memories_ad AFTER DELETE ON agentic_memories BEGIN
                DELETE FROM agentic_memories_fts WHERE rowid = old.rowid;
            END",
            [],
        )?;

        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS agentic_memories_au AFTER UPDATE ON agentic_memories BEGIN
                DELETE FROM agentic_memories_fts WHERE rowid = old.rowid;
                INSERT INTO agentic_memories_fts(rowid, content, context, keywords, tags)
                VALUES (new.rowid, new.content, new.context, new.keywords, new.tags);
            END",
            [],
        )?;

        // FTS5 virtual table for full-text search on memories
        self.conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                topic,
                text,
                snippet_text,
                entities,
                content='memories',
                content_rowid='rowid'
            )",
            [],
        )?;

        // FTS5 triggers to keep index in sync
        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
                INSERT INTO memories_fts(rowid, topic, text, snippet_text, entities)
                VALUES (new.rowid, new.topic, new.text, new.snippet_text, new.entities);
            END",
            [],
        )?;

        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
                DELETE FROM memories_fts WHERE rowid = old.rowid;
            END",
            [],
        )?;

        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
                DELETE FROM memories_fts WHERE rowid = old.rowid;
                INSERT INTO memories_fts(rowid, topic, text, snippet_text, entities)
                VALUES (new.rowid, new.topic, new.text, new.snippet_text, new.entities);
            END",
            [],
        )?;

        // Indexes for performance
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_turns_thread ON turns(thread_id)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_turns_created ON turns(created_at DESC)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_topic ON memories(topic)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_kind ON memories(kind)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agentic_context ON agentic_memories(context)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agentic_last_accessed ON agentic_memories(last_accessed DESC)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_created ON memories(created_at DESC)",
            [],
        )?;

        // Indexes for hierarchy tables
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_projects_workspace ON projects(workspace_id)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_projects_status ON projects(status)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_areas_project ON areas(project_id)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_topics_area ON topics(area_id)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_topics_is_index ON topics(is_index_note)",
            [],
        )?;

        // Indexes for new memory columns
        let has_topic_id_index = self.has_column("memories", "topic_id")?;
        if has_topic_id_index {
            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_memories_topic_id ON memories(topic_id)",
                [],
            )?;

            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_memories_importance ON memories(importance)",
                [],
            )?;

            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_memories_status ON memories(status)",
                [],
            )?;
        }

        // Indexes for relationships
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_relations_source ON memory_relations(source_id)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_relations_target ON memory_relations(target_id)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_relations_type ON memory_relations(relation_type)",
            [],
        )?;

        // Indexes for versions
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_versions_memory ON memory_versions(memory_id)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_versions_number ON memory_versions(memory_id, version_number DESC)",
            [],
        )?;

        debug!("Database schema initialized");
        Ok(())
    }

    /// Insert a turn into the database
    pub fn insert_turn(&self, turn: &Turn) -> Result<()> {
        self.conn.execute(
            "INSERT INTO turns (id, thread_id, ts_user, user_text, ts_ai, ai_text,
                               source_app, source_url, source_path, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                turn.id.0,
                turn.thread_id.0,
                turn.ts_user,
                turn.user_text,
                turn.ts_ai,
                turn.ai_text,
                format!("{:?}", turn.source.app),
                turn.source.url,
                turn.source.path,
                Utc::now().to_rfc3339(),
            ],
        )?;

        debug!("Inserted turn: {}", turn.id);
        Ok(())
    }

    /// Get a turn by ID
    pub fn get_turn(&self, id: &TurnId) -> Result<Option<Turn>> {
        let turn = self
            .conn
            .query_row(
                "SELECT id, thread_id, ts_user, user_text, ts_ai, ai_text,
                        source_app, source_url, source_path
                 FROM turns WHERE id = ?1",
                params![id.0],
                |row| {
                    Ok(Turn {
                        id: TurnId(row.get(0)?),
                        thread_id: ThreadId(row.get(1)?),
                        ts_user: row.get(2)?,
                        user_text: row.get(3)?,
                        ts_ai: row.get(4)?,
                        ai_text: row.get(5)?,
                        source: memory_layer_schemas::TurnSource {
                            app: memory_layer_schemas::SourceApp::Other, // Parse properly in production
                            url: row.get(7)?,
                            path: row.get(8)?,
                        },
                    })
                },
            )
            .optional()?;

        Ok(turn)
    }

    /// Get turns by thread ID
    pub fn get_turns_by_thread(&self, thread_id: &ThreadId, limit: usize) -> Result<Vec<Turn>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, thread_id, ts_user, user_text, ts_ai, ai_text,
                    source_app, source_url, source_path
             FROM turns
             WHERE thread_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;

        let turns = stmt
            .query_map(params![thread_id.0, limit], |row| {
                Ok(Turn {
                    id: TurnId(row.get(0)?),
                    thread_id: ThreadId(row.get(1)?),
                    ts_user: row.get(2)?,
                    user_text: row.get(3)?,
                    ts_ai: row.get(4)?,
                    ai_text: row.get(5)?,
                    source: memory_layer_schemas::TurnSource {
                        app: memory_layer_schemas::SourceApp::Other,
                        url: row.get(7)?,
                        path: row.get(8)?,
                    },
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(turns)
    }

    fn row_to_memory(&self, row: &Row) -> rusqlite::Result<Memory> {
        let entities_json: String = row.get(8)?;
        let provenance_json: String = row.get(9)?;

        let entities: Vec<String> = serde_json::from_str(&entities_json).map_err(json_error)?;
        let provenance: Vec<TurnId> = serde_json::from_str(&provenance_json).map_err(json_error)?;

        let snippet_title: Option<String> = row.get(4)?;
        let snippet = if let Some(title) = snippet_title {
            Some(Snippet {
                title,
                text: row.get(5)?,
                loc: row.get(6)?,
                language: row.get(7)?,
            })
        } else {
            None
        };

        let kind_raw: String = row.get(1)?;
        let kind = parse_memory_kind(&kind_raw);

        Ok(Memory {
            id: MemoryId(row.get(0)?),
            kind,
            topic: row.get(2)?,
            text: row.get(3)?,
            snippet,
            entities,
            provenance,
            created_at: row.get(10)?,
            ttl: row.get::<_, Option<i64>>(11)?.map(|t| t as u64),
        })
    }

    /// Insert a memory into the database
    pub fn insert_memory(&self, memory: &Memory) -> Result<()> {
        let entities_json = serde_json::to_string(&memory.entities)?;
        let provenance_json = serde_json::to_string(&memory.provenance)?;

        let (snippet_title, snippet_text, snippet_loc, snippet_language) =
            if let Some(ref snippet) = memory.snippet {
                (
                    Some(&snippet.title),
                    Some(&snippet.text),
                    snippet.loc.as_ref(),
                    snippet.language.as_ref(),
                )
            } else {
                (None, None, None, None)
            };

        let kind_label = format!("{:?}", memory.kind);

        self.conn.execute(
            "INSERT INTO memories (id, kind, topic, text, snippet_title, snippet_text,
                                  snippet_loc, snippet_language, entities, provenance,
                                  created_at, ttl)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                memory.id.0.as_str(),
                kind_label,
                memory.topic.as_str(),
                memory.text.as_str(),
                snippet_title,
                snippet_text,
                snippet_loc,
                snippet_language,
                entities_json,
                provenance_json,
                memory.created_at,
                memory.ttl.map(|t| t as i64),
            ],
        )?;

        debug!("Inserted memory: {} (kind: {:?})", memory.id, memory.kind);
        Ok(())
    }

    /// Materialize an agentic memory record inspired by A-mem metadata
    pub fn upsert_agentic_memory(&self, memory: &Memory) -> Result<AgenticMemory> {
        let content = memory
            .snippet
            .as_ref()
            .map(|snippet| snippet.text.trim().to_string())
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| memory.text.trim().to_string());

        let context = if memory.topic.trim().is_empty() {
            self.derive_context(&content)
        } else {
            memory.topic.trim().to_string()
        };

        let keywords = self.derive_keywords(&content);
        let tags = self.derive_tags(memory, &context, &keywords);

        let now = Utc::now().to_rfc3339();

        let existing = self
            .conn
            .query_row(
                "SELECT created_at, retrieval_count, links, evolution_history
                 FROM agentic_memories
                 WHERE memory_id = ?1",
                params![memory.id.0.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;

        let mut created_at = now.clone();
        let mut retrieval_count: u32 = 0;
        let mut existing_links: Vec<AgenticLink> = Vec::new();
        let mut evolution_history: Vec<AgenticEvolution> = Vec::new();

        if let Some((created, count, links_json, history_json)) = existing {
            created_at = created;
            retrieval_count = (count.max(0)) as u32;
            existing_links = serde_json::from_str(&links_json).unwrap_or_default();
            evolution_history = serde_json::from_str(&history_json).unwrap_or_default();
        }

        let mut proposed_links =
            self.suggest_links(&context, &keywords, &memory.id, MAX_AGENTIC_LINKS.saturating_sub(1))?;
        existing_links = self.merge_links(existing_links, &mut proposed_links);

        // Record evolution event
        evolution_history.push(AgenticEvolution {
            timestamp: now.clone(),
            summary: "Refreshed agentic attributes".to_string(),
            changes: Some(vec![
                format!("context:{}", context),
                format!("keywords:{}", keywords.join(",")),
            ]),
        });
        if evolution_history.len() > 25 {
            let overflow = evolution_history.len() - 25;
            evolution_history.drain(0..overflow);
        }

        let agentic = AgenticMemory {
            id: memory.id.clone(),
            content: content.clone(),
            context: context.clone(),
            keywords: keywords.clone(),
            tags: tags.clone(),
            category: Some(memory.kind.as_str().to_string()),
            links: existing_links.clone(),
            retrieval_count,
            last_accessed: now.clone(),
            created_at: created_at.clone(),
            evolution_history: evolution_history.clone(),
        };

        let keywords_json = serde_json::to_string(&agentic.keywords)?;
        let tags_json = serde_json::to_string(&agentic.tags)?;
        let links_json = serde_json::to_string(&agentic.links)?;
        let evolution_json = serde_json::to_string(&agentic.evolution_history)?;
        let category = agentic.category.clone();
        let content_value = agentic.content.clone();
        let context_value = agentic.context.clone();
        let last_accessed_value = agentic.last_accessed.clone();
        let created_at_value = agentic.created_at.clone();
        let retrieval_value = agentic.retrieval_count;

        self.conn.execute(
            "INSERT INTO agentic_memories (
                memory_id, content, context, keywords, tags, category, links,
                retrieval_count, last_accessed, evolution_history, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(memory_id) DO UPDATE SET
                content = excluded.content,
                context = excluded.context,
                keywords = excluded.keywords,
                tags = excluded.tags,
                category = excluded.category,
                links = excluded.links,
                last_accessed = excluded.last_accessed,
                evolution_history = excluded.evolution_history",
            params![
                agentic.id.0.as_str(),
                content_value,
                context_value,
                keywords_json,
                tags_json,
                category,
                links_json,
                retrieval_value,
                last_accessed_value,
                evolution_json,
                created_at_value
            ],
        )?;

        // Ensure reverse links exist for neighbors
        for link in &agentic.links {
            self.merge_single_link(&link.target, &agentic.id, link.strength, "topic-match")?;
        }

        Ok(agentic)
    }

    pub fn get_agentic_memory(&self, memory_id: &MemoryId) -> Result<Option<AgenticMemory>> {
        self.conn
            .query_row(
                "SELECT content, context, keywords, tags, category, links,
                        retrieval_count, last_accessed, evolution_history, created_at
                 FROM agentic_memories
                 WHERE memory_id = ?1",
                params![memory_id.0.as_str()],
                |row| {
                    let keywords: Vec<String> =
                        serde_json::from_str(&row.get::<_, String>(2)?).unwrap_or_default();
                    let tags: Vec<String> =
                        serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default();
                    let links: Vec<AgenticLink> =
                        serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default();
                    let evolution_history: Vec<AgenticEvolution> =
                        serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default();

                    Ok(AgenticMemory {
                        id: memory_id.clone(),
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
            .optional()
            .map_err(Into::into)
    }

    fn derive_context(&self, content: &str) -> String {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return "general".to_string();
        }

        let first_line = trimmed.lines().next().unwrap_or(trimmed).trim();
        let mut candidate = first_line
            .split(['.', '!', '?'])
            .next()
            .unwrap_or(first_line)
            .trim()
            .to_string();

        if candidate.is_empty() {
            candidate = trimmed.chars().take(96).collect();
        }

        if candidate.len() > 96 {
            candidate.truncate(96);
        }

        if candidate.is_empty() {
            "general".to_string()
        } else {
            candidate
        }
    }

    fn derive_keywords(&self, content: &str) -> Vec<String> {
        let normalized = content.to_lowercase();
        let regex = Regex::new(r"[a-z0-9][a-z0-9_\-/]{3,}").unwrap();
        let mut counts: HashMap<String, usize> = HashMap::new();

        for capture in regex.find_iter(&normalized) {
            let token = capture
                .as_str()
                .trim_matches(|c: char| !c.is_alphanumeric());
            if token.len() < 4 {
                continue;
            }
            if STOPWORDS.iter().any(|stop| stop == &token) {
                continue;
            }
            *counts.entry(token.to_string()).or_default() += 1;
        }

        let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
        ranked.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.0.len().cmp(&b.0.len()).reverse())
                .then_with(|| a.0.cmp(&b.0))
        });

        ranked.into_iter().map(|(token, _)| token).take(8).collect()
    }

    fn derive_tags(&self, memory: &Memory, context: &str, keywords: &[String]) -> Vec<String> {
        let mut tags = Vec::new();
        tags.push(format!("kind:{}", memory.kind.as_str()));

        if !memory.topic.trim().is_empty() {
            tags.push(format!("topic:{}", memory.topic.to_lowercase()));
        }

        if !context.trim().is_empty() {
            tags.push(format!("context:{}", context.to_lowercase()));
        }

        if let Some(snippet) = &memory.snippet {
            if let Some(lang) = &snippet.language {
                tags.push(format!("lang:{}", lang.to_lowercase()));
            }
        }

        for keyword in keywords.iter().take(4) {
            tags.push(format!("kw:{}", keyword));
        }

        tags.iter_mut().for_each(|tag| {
            *tag = tag
                .replace('\n', " ")
                .replace('\t', " ")
                .split_whitespace()
                .collect::<Vec<&str>>()
                .join(" ");
        });

        tags.retain(|tag| !tag.is_empty());
        tags.sort();
        tags.dedup();
        tags.truncate(12);
        tags
    }

    fn suggest_links(
        &self,
        context: &str,
        current_keywords: &[String],
        current_memory: &MemoryId,
        limit: usize,
    ) -> Result<Vec<AgenticLink>> {
        if limit == 0 {
            return Ok(vec![]);
        }

        // Get all candidate memories with their keywords
        let mut stmt = self.conn.prepare(
            "SELECT memory_id, keywords, context
             FROM agentic_memories
             WHERE memory_id != ?1
             ORDER BY last_accessed DESC",
        )?;

        let candidates: Vec<(MemoryId, Vec<String>, String)> = stmt
            .query_map(params![current_memory.0.as_str()], |row| {
                let memory_id = MemoryId(row.get::<_, String>(0)?);
                let keywords_json: String = row.get(1)?;
                let keywords = serde_json::from_str::<Vec<String>>(&keywords_json).unwrap_or_default();
                let context: String = row.get(2)?;
                Ok((memory_id, keywords, context))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Calculate keyword similarity and filter by threshold
        let mut links: Vec<AgenticLink> = candidates
            .into_iter()
            .filter_map(|(memory_id, keywords, mem_context)| {
                // Link memories with same context (legacy behavior)
                let same_context = mem_context == context;

                if keywords.is_empty() {
                    // If no keywords but same context, create a basic link
                    if same_context {
                        return Some(AgenticLink {
                            target: memory_id,
                            strength: 0.6,
                            rationale: Some("same-context".to_string()),
                        });
                    }
                    return None;
                }

                // Calculate Jaccard similarity: |A ∩ B| / |A ∪ B|
                let current_set: HashSet<_> = current_keywords.iter().collect();
                let candidate_set: HashSet<_> = keywords.iter().collect();

                let intersection_count = current_set.intersection(&candidate_set).count();
                let union_count = current_set.union(&candidate_set).count();

                let similarity = if union_count > 0 {
                    intersection_count as f32 / union_count as f32
                } else {
                    0.0
                };

                // Link if similarity > 65% OR same context
                if similarity > 0.65 || same_context {
                    let (strength, rationale) = if similarity > 0.65 {
                        if same_context {
                            (similarity, format!("keyword-similarity: {:.1}% (same context)", similarity * 100.0))
                        } else {
                            (similarity, format!("keyword-similarity: {:.1}%", similarity * 100.0))
                        }
                    } else {
                        (0.6, "same-context".to_string())
                    };

                    Some(AgenticLink {
                        target: memory_id,
                        strength,
                        rationale: Some(rationale),
                    })
                } else {
                    None
                }
            })
            .collect();

        // Sort by strength (descending) and limit results
        links.sort_by(|a, b| {
            b.strength
                .partial_cmp(&a.strength)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        links.truncate(limit);

        Ok(links)
    }

    fn merge_links(
        &self,
        mut existing: Vec<AgenticLink>,
        proposed: &mut Vec<AgenticLink>,
    ) -> Vec<AgenticLink> {
        for link in proposed.drain(..) {
            if let Some(existing_link) = existing.iter_mut().find(|l| l.target == link.target) {
                if link.strength > existing_link.strength {
                    existing_link.strength = link.strength;
                }
                if link.rationale.is_some() {
                    existing_link.rationale = link.rationale.clone();
                }
            } else {
                existing.push(link);
            }
        }

        existing.sort_by(|a, b| {
            b.strength
                .partial_cmp(&a.strength)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        existing.truncate(MAX_AGENTIC_LINKS);
        existing
    }

    fn merge_single_link(
        &self,
        from: &MemoryId,
        to: &MemoryId,
        strength: f32,
        rationale: &str,
    ) -> Result<()> {
        let links_json: Option<String> = self
            .conn
            .query_row(
                "SELECT links FROM agentic_memories WHERE memory_id = ?1",
                params![from.0.as_str()],
                |row| row.get(0),
            )
            .optional()?;

        let mut links: Vec<AgenticLink> = links_json
            .map(|json| serde_json::from_str(&json).unwrap_or_default())
            .unwrap_or_default();

        let mut found = false;
        for link in &mut links {
            if link.target == *to {
                found = true;
                if strength > link.strength {
                    link.strength = strength;
                }
                if link.rationale.is_none() && !rationale.is_empty() {
                    link.rationale = Some(rationale.to_string());
                }
                break;
            }
        }

        if !found {
            links.push(AgenticLink {
                target: to.clone(),
                strength,
                rationale: if rationale.is_empty() {
                    None
                } else {
                    Some(rationale.to_string())
                },
            });
        }

        links.sort_by(|a, b| {
            b.strength
                .partial_cmp(&a.strength)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        links.truncate(MAX_AGENTIC_LINKS);

        let updated_links = serde_json::to_string(&links)?;

        self.conn.execute(
            "UPDATE agentic_memories SET links = ?1 WHERE memory_id = ?2",
            params![updated_links, from.0.as_str()],
        )?;

        Ok(())
    }

    /// Search memories using FTS5
    pub fn search_memories(&self, query: &str, limit: usize) -> Result<Vec<Memory>> {
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.kind, m.topic, m.text, m.snippet_title, m.snippet_text,
                    m.snippet_loc, m.snippet_language, m.entities, m.provenance,
                    m.created_at, m.ttl
             FROM memories m
             JOIN memories_fts fts ON m.rowid = fts.rowid
             WHERE memories_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let memories = stmt
            .query_map(params![query, limit], |row| self.row_to_memory(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(memories)
    }

    /// Get most recent memories
    pub fn get_recent_memories(&self, limit: usize) -> Result<Vec<Memory>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, topic, text, snippet_title, snippet_text,
                    snippet_loc, snippet_language, entities, provenance,
                    created_at, ttl
             FROM memories
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;

        let memories = stmt
            .query_map(params![limit as i64], |row| self.row_to_memory(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(memories)
    }

    /// Get recent memories by topic
    pub fn get_memories_by_topic(&self, topic: &str, limit: usize) -> Result<Vec<Memory>> {
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
            .query_map(params![topic, limit], |row| self.row_to_memory(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(memories)
    }

    /// Summaries for topics to power memory maps/analytics
    pub fn topic_summaries(&self, limit: usize) -> Result<Vec<TopicSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT topic, COUNT(*) as memory_count, MAX(created_at) as last_memory
             FROM memories
             GROUP BY topic
             ORDER BY memory_count DESC, last_memory DESC
             LIMIT ?1",
        )?;

        let summaries = stmt
            .query_map(params![limit as i64], |row| {
                Ok(TopicSummary {
                    topic: row.get(0)?,
                    memory_count: row.get::<_, i64>(1)? as usize,
                    last_memory_at: row.get::<_, Option<String>>(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(summaries)
    }

    /// Count total memories
    pub fn count_memories(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Count total turns
    pub fn count_turns(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM turns", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    // ========== HIERARCHY MIGRATION METHODS ==========

    /// Get all distinct topics from memories with their memory counts
    pub fn get_all_distinct_topics(&self) -> Result<Vec<(String, usize)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT topic, COUNT(*) as count
             FROM memories
             WHERE topic IS NOT NULL AND topic != ''
             GROUP BY topic
             ORDER BY count DESC",
        )?;

        let topics = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(topics)
    }

    /// Get or create a workspace by name, returning its ID
    pub fn get_or_create_workspace(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<memory_layer_schemas::WorkspaceId> {
        // Try to find existing workspace
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM workspaces WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing {
            return Ok(memory_layer_schemas::WorkspaceId(id));
        }

        // Create new workspace
        let id = memory_layer_schemas::generate_workspace_id();
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO workspaces (id, name, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id.0, name, description, now, now],
        )?;

        info!("Created workspace: {} ({})", name, id.0);
        Ok(id)
    }

    /// Get or create a project by name, returning its ID
    pub fn get_or_create_project(
        &self,
        workspace_id: &memory_layer_schemas::WorkspaceId,
        name: &str,
        description: Option<&str>,
        status: memory_layer_schemas::ProjectStatus,
    ) -> Result<memory_layer_schemas::ProjectId> {
        // Try to find existing project in this workspace
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM projects WHERE workspace_id = ?1 AND name = ?2",
                params![workspace_id.0, name],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing {
            return Ok(memory_layer_schemas::ProjectId(id));
        }

        // Create new project
        let id = memory_layer_schemas::generate_project_id();
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO projects (id, workspace_id, name, description, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id.0,
                workspace_id.0,
                name,
                description,
                status.as_str(),
                now,
                now
            ],
        )?;

        info!("Created project: {} ({})", name, id.0);
        Ok(id)
    }

    /// Get or create an area by name, returning its ID
    pub fn get_or_create_area(
        &self,
        project_id: &memory_layer_schemas::ProjectId,
        name: &str,
        description: Option<&str>,
    ) -> Result<memory_layer_schemas::AreaId> {
        // Try to find existing area in this project
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM areas WHERE project_id = ?1 AND name = ?2",
                params![project_id.0, name],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing {
            return Ok(memory_layer_schemas::AreaId(id));
        }

        // Create new area
        let id = memory_layer_schemas::generate_area_id();
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO areas (id, project_id, name, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id.0, project_id.0, name, description, now, now],
        )?;

        info!("Created area: {} ({})", name, id.0);
        Ok(id)
    }

    /// Get or create a topic by name, returning its ID
    pub fn get_or_create_topic(
        &self,
        area_id: &memory_layer_schemas::AreaId,
        name: &str,
        description: Option<&str>,
        is_index_note: bool,
    ) -> Result<memory_layer_schemas::TopicId> {
        // Try to find existing topic in this area
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM topics WHERE area_id = ?1 AND name = ?2",
                params![area_id.0, name],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing {
            return Ok(memory_layer_schemas::TopicId(id));
        }

        // Create new topic
        let id = memory_layer_schemas::generate_topic_id();
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO topics (id, area_id, name, description, is_index_note, summary, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id.0,
                area_id.0,
                name,
                description,
                is_index_note as i32,
                None::<String>,
                now,
                now
            ],
        )?;

        info!("Created topic: {} ({})", name, id.0);
        Ok(id)
    }

    /// Update all memories with a given flat topic string to point to new hierarchical topic_id
    pub fn update_memories_topic(
        &self,
        old_topic: &str,
        new_topic_id: &memory_layer_schemas::TopicId,
    ) -> Result<usize> {
        let updated = self.conn.execute(
            "UPDATE memories SET topic_id = ?1 WHERE topic = ?2",
            params![new_topic_id.0, old_topic],
        )?;

        debug!(
            "Updated {} memories from topic '{}' to topic_id '{}'",
            updated, old_topic, new_topic_id.0
        );

        Ok(updated)
    }

    /// Get all topics with their area names for index note generation
    pub fn get_all_topics(&self) -> Result<Vec<(memory_layer_schemas::TopicId, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.name, a.name
             FROM topics t
             JOIN areas a ON t.area_id = a.id
             ORDER BY t.name",
        )?;

        let topics = stmt
            .query_map([], |row| {
                Ok((
                    memory_layer_schemas::TopicId(row.get(0)?),
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(topics)
    }

    /// Get all workspaces
    pub fn get_all_workspaces(&self) -> Result<Vec<(memory_layer_schemas::WorkspaceId, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name FROM workspaces ORDER BY name"
        )?;

        let workspaces = stmt
            .query_map([], |row| {
                Ok((
                    memory_layer_schemas::WorkspaceId(row.get(0)?),
                    row.get::<_, String>(1)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(workspaces)
    }

    /// Get all projects
    pub fn get_all_projects(&self) -> Result<Vec<(memory_layer_schemas::ProjectId, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.name, w.name
             FROM projects p
             JOIN workspaces w ON p.workspace_id = w.id
             ORDER BY p.name"
        )?;

        let projects = stmt
            .query_map([], |row| {
                Ok((
                    memory_layer_schemas::ProjectId(row.get(0)?),
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(projects)
    }

    /// Get projects by workspace
    pub fn get_projects_by_workspace(
        &self,
        workspace_id: &memory_layer_schemas::WorkspaceId,
    ) -> Result<Vec<(memory_layer_schemas::ProjectId, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name FROM projects WHERE workspace_id = ?1 ORDER BY name"
        )?;

        let projects = stmt
            .query_map(params![workspace_id.0], |row| {
                Ok((
                    memory_layer_schemas::ProjectId(row.get(0)?),
                    row.get::<_, String>(1)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(projects)
    }

    /// Get all areas
    pub fn get_all_areas(&self) -> Result<Vec<(memory_layer_schemas::AreaId, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.name, p.name
             FROM areas a
             JOIN projects p ON a.project_id = p.id
             ORDER BY a.name"
        )?;

        let areas = stmt
            .query_map([], |row| {
                Ok((
                    memory_layer_schemas::AreaId(row.get(0)?),
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(areas)
    }

    /// Get areas by project
    pub fn get_areas_by_project(
        &self,
        project_id: &memory_layer_schemas::ProjectId,
    ) -> Result<Vec<(memory_layer_schemas::AreaId, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name FROM areas WHERE project_id = ?1 ORDER BY name"
        )?;

        let areas = stmt
            .query_map(params![project_id.0], |row| {
                Ok((
                    memory_layer_schemas::AreaId(row.get(0)?),
                    row.get::<_, String>(1)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(areas)
    }

    /// Get topic info by topic ID
    pub fn get_topic_info(
        &self,
        topic_id: &memory_layer_schemas::TopicId,
    ) -> Result<(String, String)> {
        self.conn.query_row(
            "SELECT t.name, a.name FROM topics t
             JOIN areas a ON t.area_id = a.id
             WHERE t.id = ?1",
            params![topic_id.0],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|e| anyhow::anyhow!("Failed to get topic info: {}", e))
    }

    /// Count memories by hierarchical topic_id
    pub fn count_memories_by_topic_id(
        &self,
        topic_id: &memory_layer_schemas::TopicId,
    ) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE topic_id = ?1",
            params![topic_id.0],
            |row| row.get(0),
        )?;

        Ok(count as usize)
    }

    /// Generate or update an index note (Zettelkasten hub note) for a topic
    pub fn generate_or_update_index_note(
        &self,
        topic_id: &memory_layer_schemas::TopicId,
        topic_name: &str,
        area_name: &str,
        memory_count: usize,
    ) -> Result<()> {
        let summary = format!(
            "Index note for '{}' in area '{}'. Contains {} memories.",
            topic_name, area_name, memory_count
        );

        let now = Utc::now().to_rfc3339();

        // Update topics table (legacy support)
        self.conn.execute(
            "UPDATE topics SET is_index_note = 1, summary = ?1, updated_at = ?2 WHERE id = ?3",
            params![summary, now, topic_id.0],
        )?;

        // Create or update index_notes table entry
        let index_note_id = format!("idx_{}", topic_id.0);

        // Get key memories for this topic (top 5 by created_at)
        let key_memories: Vec<String> = self.conn
            .prepare("SELECT id FROM memories WHERE topic_id = ?1 ORDER BY created_at DESC LIMIT 5")?
            .query_map(params![topic_id.0], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let key_memories_json = serde_json::to_string(&key_memories)?;

        // Check if index note already exists
        let exists: bool = self.conn
            .query_row(
                "SELECT 1 FROM index_notes WHERE id = ?1",
                params![&index_note_id],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);

        if exists {
            // Update existing index note
            self.conn.execute(
                "UPDATE index_notes SET content = ?1, memory_count = ?2, key_memories = ?3, last_updated = ?4 WHERE id = ?5",
                params![summary, memory_count as i64, key_memories_json, now, index_note_id],
            )?;
        } else {
            // Insert new index note
            self.conn.execute(
                "INSERT INTO index_notes (id, scope_type, scope_id, name, content, memory_count, key_memories, tags, created_at, last_updated)
                 VALUES (?1, 'topic', ?2, ?3, ?4, ?5, ?6, '[]', ?7, ?7)",
                params![index_note_id, topic_id.0, topic_name, summary, memory_count as i64, key_memories_json, now],
            )?;
        }

        debug!("Generated index note for topic '{}'", topic_name);
        Ok(())
    }

    /// Get an index note for a specific scope (topic, area, or project)
    pub fn get_index_note(
        &self,
        scope_type: &str,
        scope_id: &str,
    ) -> Result<Option<IndexNote>> {
        let note = self.conn
            .query_row(
                "SELECT id, scope_type, scope_id, name, content, memory_count, key_memories, tags, created_at, last_updated
                 FROM index_notes WHERE scope_type = ?1 AND scope_id = ?2",
                params![scope_type, scope_id],
                |row| {
                    Ok(IndexNote {
                        id: row.get(0)?,
                        scope_type: row.get(1)?,
                        scope_id: row.get(2)?,
                        name: row.get(3)?,
                        content: row.get(4)?,
                        memory_count: row.get(5)?,
                        key_memories: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                        tags: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
                        created_at: row.get(8)?,
                        last_updated: row.get(9)?,
                    })
                },
            )
            .optional()?;

        Ok(note)
    }

    /// Get all index notes for a workspace hierarchy
    pub fn get_all_index_notes(&self) -> Result<Vec<IndexNote>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, scope_type, scope_id, name, content, memory_count, key_memories, tags, created_at, last_updated
             FROM index_notes ORDER BY last_updated DESC"
        )?;

        let notes = stmt
            .query_map([], |row| {
                Ok(IndexNote {
                    id: row.get(0)?,
                    scope_type: row.get(1)?,
                    scope_id: row.get(2)?,
                    name: row.get(3)?,
                    content: row.get(4)?,
                    memory_count: row.get(5)?,
                    key_memories: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                    tags: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
                    created_at: row.get(8)?,
                    last_updated: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(notes)
    }

    /// Update index note content (for manual edits/refinement)
    pub fn update_index_note_content(
        &self,
        index_note_id: &str,
        new_content: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "UPDATE index_notes SET content = ?1, last_updated = ?2 WHERE id = ?3",
            params![new_content, now, index_note_id],
        )?;

        debug!("Updated index note content for '{}'", index_note_id);
        Ok(())
    }

    /// Add tags to an index note
    pub fn add_index_note_tags(
        &self,
        index_note_id: &str,
        tags: Vec<String>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let tags_json = serde_json::to_string(&tags)?;

        self.conn.execute(
            "UPDATE index_notes SET tags = ?1, last_updated = ?2 WHERE id = ?3",
            params![tags_json, now, index_note_id],
        )?;

        debug!("Added tags to index note '{}'", index_note_id);
        Ok(())
    }

    // ========== MEMORY LIFECYCLE METHODS ==========

    /// Transition a memory to a new lifecycle status
    /// Statuses: fleeting, permanent, archived, deprecated
    pub fn update_memory_status(
        &self,
        memory_id: &MemoryId,
        new_status: &str,
    ) -> Result<()> {
        // Validate status
        let valid_statuses = ["fleeting", "permanent", "archived", "deprecated"];
        if !valid_statuses.contains(&new_status) {
            anyhow::bail!("Invalid status '{}'. Must be one of: fleeting, permanent, archived, deprecated", new_status);
        }

        self.conn.execute(
            "UPDATE memories SET status = ?1 WHERE id = ?2",
            params![new_status, memory_id.0],
        )?;

        debug!("Updated memory {} status to '{}'", memory_id.0, new_status);
        Ok(())
    }

    /// Get all memories with a specific lifecycle status
    pub fn get_memories_by_status(
        &self,
        status: &str,
        limit: Option<usize>,
    ) -> Result<Vec<Memory>> {
        let query = if let Some(lim) = limit {
            format!(
                "SELECT id, kind, topic, text, snippet_title, snippet_text, snippet_loc, snippet_language, entities, provenance, created_at, ttl
                 FROM memories WHERE status = ?1 ORDER BY created_at DESC LIMIT {}",
                lim
            )
        } else {
            "SELECT id, kind, topic, text, snippet_title, snippet_text, snippet_loc, snippet_language, entities, provenance, created_at, ttl
             FROM memories WHERE status = ?1 ORDER BY created_at DESC".to_string()
        };

        let mut stmt = self.conn.prepare(&query)?;
        let memories = stmt
            .query_map(params![status], |row| {
                Ok(Memory {
                    id: MemoryId(row.get(0)?),
                    kind: parse_memory_kind(&row.get::<_, String>(1)?),
                    topic: row.get(2)?,
                    text: row.get(3)?,
                    snippet: if row.get::<_, Option<String>>(4)?.is_some() {
                        Some(Snippet {
                            title: row.get(4)?,
                            text: row.get(5)?,
                            loc: row.get(6)?,
                            language: row.get(7)?,
                        })
                    } else {
                        None
                    },
                    entities: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
                    provenance: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
                    created_at: row.get(10)?,
                    ttl: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(memories)
    }

    /// Get lifecycle statistics for all memories
    pub fn get_lifecycle_stats(&self) -> Result<LifecycleStats> {
        let fleeting: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE status = 'fleeting'",
            [],
            |row| row.get(0),
        )?;

        let permanent: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE status = 'permanent'",
            [],
            |row| row.get(0),
        )?;

        let archived: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE status = 'archived'",
            [],
            |row| row.get(0),
        )?;

        let deprecated: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE status = 'deprecated'",
            [],
            |row| row.get(0),
        )?;

        Ok(LifecycleStats {
            fleeting,
            permanent,
            archived,
            deprecated,
            total: fleeting + permanent + archived + deprecated,
        })
    }

    /// Promote fleeting memories to permanent (manual workflow)
    pub fn promote_to_permanent(
        &self,
        memory_id: &MemoryId,
    ) -> Result<()> {
        // Check current status
        let current_status: String = self.conn.query_row(
            "SELECT status FROM memories WHERE id = ?1",
            params![memory_id.0],
            |row| row.get(0),
        )?;

        if current_status != "fleeting" {
            anyhow::bail!("Memory {} is already '{}', can only promote 'fleeting' memories", memory_id.0, current_status);
        }

        self.update_memory_status(memory_id, "permanent")?;
        debug!("Promoted memory {} to permanent", memory_id.0);
        Ok(())
    }

    /// Archive old memories based on age threshold (days)
    pub fn archive_old_memories(
        &self,
        age_threshold_days: u32,
    ) -> Result<usize> {
        let threshold_date = chrono::Utc::now() - chrono::Duration::days(age_threshold_days as i64);
        let threshold_str = threshold_date.to_rfc3339();

        let archived = self.conn.execute(
            "UPDATE memories SET status = 'archived'
             WHERE status = 'fleeting'
             AND created_at < ?1
             AND superseded_by IS NULL",
            params![threshold_str],
        )?;

        info!("Archived {} old fleeting memories (older than {} days)", archived, age_threshold_days);
        Ok(archived)
    }

    /// Get memories that need review (fleeting memories older than X days)
    pub fn get_memories_needing_review(
        &self,
        age_threshold_days: u32,
    ) -> Result<Vec<Memory>> {
        let threshold_date = chrono::Utc::now() - chrono::Duration::days(age_threshold_days as i64);
        let threshold_str = threshold_date.to_rfc3339();

        let mut stmt = self.conn.prepare(
            "SELECT id, kind, topic, text, snippet_title, snippet_text, snippet_loc, snippet_language, entities, provenance, created_at, ttl
             FROM memories
             WHERE status = 'fleeting'
             AND created_at < ?1
             ORDER BY created_at ASC
             LIMIT 50"
        )?;

        let memories = stmt
            .query_map(params![threshold_str], |row| {
                Ok(Memory {
                    id: MemoryId(row.get(0)?),
                    kind: parse_memory_kind(&row.get::<_, String>(1)?),
                    topic: row.get(2)?,
                    text: row.get(3)?,
                    snippet: if row.get::<_, Option<String>>(4)?.is_some() {
                        Some(Snippet {
                            title: row.get(4)?,
                            text: row.get(5)?,
                            loc: row.get(6)?,
                            language: row.get(7)?,
                        })
                    } else {
                        None
                    },
                    entities: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
                    provenance: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
                    created_at: row.get(10)?,
                    ttl: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(memories)
    }

    // ========== PROGRESSIVE SUMMARIZATION METHODS ==========

    /// Add a progressive summary layer to a memory
    /// Layer 1: Bolded highlights (most important passages)
    /// Layer 2: Highlighted sections (key ideas)
    /// Layer 3: Executive summary (1-2 sentence distillation)
    /// Layer 4: Remix (personal insights and connections)
    pub fn add_summary_layer(
        &self,
        memory_id: &MemoryId,
        layer: u8,
        content: &str,
    ) -> Result<String> {
        if layer == 0 || layer > 4 {
            anyhow::bail!("Layer must be between 1 and 4");
        }

        let id = ulid::Ulid::new().to_string();
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO progressive_summaries (id, memory_id, layer, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, memory_id.0, layer as i64, content, now],
        )?;

        debug!("Added layer {} summary to memory {}", layer, memory_id.0);
        Ok(id)
    }

    /// Get all summary layers for a memory
    pub fn get_summary_layers(
        &self,
        memory_id: &MemoryId,
    ) -> Result<Vec<ProgressiveSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, memory_id, layer, content, created_at
             FROM progressive_summaries
             WHERE memory_id = ?1
             ORDER BY layer ASC"
        )?;

        let layers = stmt
            .query_map(params![memory_id.0], |row| {
                Ok(ProgressiveSummary {
                    id: row.get(0)?,
                    memory_id: MemoryId(row.get(1)?),
                    layer: row.get::<_, i64>(2)? as u8,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(layers)
    }

    /// Get the highest summarization layer for a memory
    pub fn get_max_summary_layer(
        &self,
        memory_id: &MemoryId,
    ) -> Result<Option<u8>> {
        let layer: Option<i64> = self.conn
            .query_row(
                "SELECT MAX(layer) FROM progressive_summaries WHERE memory_id = ?1",
                params![memory_id.0],
                |row| row.get(0),
            )
            .optional()?
            .flatten();

        Ok(layer.map(|l| l as u8))
    }

    /// Get all memories that have been summarized to at least a certain layer
    pub fn get_summarized_memories(
        &self,
        min_layer: u8,
        limit: Option<usize>,
    ) -> Result<Vec<(Memory, u8)>> {
        let query = if let Some(lim) = limit {
            format!(
                "SELECT DISTINCT m.id, m.kind, m.topic, m.text, m.snippet_title, m.snippet_text, m.snippet_loc, m.snippet_language, m.entities, m.provenance, m.created_at, m.ttl, MAX(ps.layer) as max_layer
                 FROM memories m
                 JOIN progressive_summaries ps ON m.id = ps.memory_id
                 WHERE ps.layer >= ?1
                 GROUP BY m.id
                 ORDER BY max_layer DESC
                 LIMIT {}",
                lim
            )
        } else {
            "SELECT DISTINCT m.id, m.kind, m.topic, m.text, m.snippet_title, m.snippet_text, m.snippet_loc, m.snippet_language, m.entities, m.provenance, m.created_at, m.ttl, MAX(ps.layer) as max_layer
             FROM memories m
             JOIN progressive_summaries ps ON m.id = ps.memory_id
             WHERE ps.layer >= ?1
             GROUP BY m.id
             ORDER BY max_layer DESC".to_string()
        };

        let mut stmt = self.conn.prepare(&query)?;
        let results = stmt
            .query_map(params![min_layer as i64], |row| {
                Ok((
                    Memory {
                        id: MemoryId(row.get(0)?),
                        kind: parse_memory_kind(&row.get::<_, String>(1)?),
                        topic: row.get(2)?,
                        text: row.get(3)?,
                        snippet: if row.get::<_, Option<String>>(4)?.is_some() {
                            Some(Snippet {
                                title: row.get(4)?,
                                text: row.get(5)?,
                                loc: row.get(6)?,
                                language: row.get(7)?,
                            })
                        } else {
                            None
                        },
                        entities: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
                        provenance: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
                        created_at: row.get(10)?,
                        ttl: row.get(11)?,
                    },
                    row.get::<_, i64>(12)? as u8,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Update an existing summary layer
    pub fn update_summary_layer(
        &self,
        summary_id: &str,
        new_content: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE progressive_summaries SET content = ?1 WHERE id = ?2",
            params![new_content, summary_id],
        )?;

        debug!("Updated summary layer {}", summary_id);
        Ok(())
    }

    /// Get summarization statistics
    pub fn get_summarization_stats(&self) -> Result<SummarizationStats> {
        let total_memories: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM memories",
            [],
            |row| row.get(0),
        )?;

        let summarized_count: usize = self.conn.query_row(
            "SELECT COUNT(DISTINCT memory_id) FROM progressive_summaries",
            [],
            |row| row.get(0),
        )?;

        let layer_1_count: usize = self.conn.query_row(
            "SELECT COUNT(DISTINCT memory_id) FROM progressive_summaries WHERE layer >= 1",
            [],
            |row| row.get(0),
        )?;

        let layer_2_count: usize = self.conn.query_row(
            "SELECT COUNT(DISTINCT memory_id) FROM progressive_summaries WHERE layer >= 2",
            [],
            |row| row.get(0),
        )?;

        let layer_3_count: usize = self.conn.query_row(
            "SELECT COUNT(DISTINCT memory_id) FROM progressive_summaries WHERE layer >= 3",
            [],
            |row| row.get(0),
        )?;

        let layer_4_count: usize = self.conn.query_row(
            "SELECT COUNT(DISTINCT memory_id) FROM progressive_summaries WHERE layer >= 4",
            [],
            |row| row.get(0),
        )?;

        Ok(SummarizationStats {
            total_memories,
            summarized_count,
            layer_1_count,
            layer_2_count,
            layer_3_count,
            layer_4_count,
        })
    }

    // ========== ATOMIC NOTE ENFORCEMENT METHODS ==========

    /// Check if a memory follows atomic note principles
    /// Returns atomicity score (0.0 - 1.0) and list of issues
    pub fn check_atomicity(&self, memory_id: &MemoryId) -> Result<AtomicityCheck> {
        let memory = self.get_memory_by_id(memory_id)?
            .ok_or_else(|| anyhow::anyhow!("Memory not found"))?;

        let mut issues = Vec::new();
        let mut score: f32 = 1.0;

        // Check 1: Length (atomic notes should be focused, typically < 500 words)
        let word_count = memory.text.split_whitespace().count();
        if word_count > 500 {
            issues.push(format!("Note is too long ({} words). Consider splitting into multiple atomic notes.", word_count));
            score -= 0.2;
        }

        // Check 2: Multiple sentences with "and", "also", "furthermore" indicate multiple ideas
        let compound_indicators = ["and ", "also ", "furthermore ", "additionally ", "moreover "];
        let compound_count = compound_indicators.iter()
            .map(|&indicator| memory.text.matches(indicator).count())
            .sum::<usize>();

        if compound_count > 5 {
            issues.push(format!("High use of compound connectors ({} instances). May contain multiple ideas.", compound_count));
            score -= 0.15;
        }

        // Check 3: Check for bullet lists (often indicates multiple distinct ideas)
        let bullet_count = memory.text.matches("\n- ").count()
            + memory.text.matches("\n* ").count()
            + memory.text.matches("\n• ").count();

        if bullet_count > 7 {
            issues.push(format!("Contains {} bullet points. Consider splitting into separate notes.", bullet_count));
            score -= 0.15;
        }

        // Check 4: Relationship count (atomic notes should link to related concepts)
        let relations = self.get_all_memory_relations(memory_id)?;

        // Query status from database
        let status: String = self.conn.query_row(
            "SELECT status FROM memories WHERE id = ?1",
            params![memory_id.0],
            |row| row.get(0),
        )?;

        if relations.is_empty() && status == "permanent" {
            issues.push("No relationships to other notes. Atomic notes should connect to the knowledge graph.".to_string());
            score -= 0.1;
        }

        // Check 5: Self-containment - check for undefined references
        let reference_patterns = ["see above", "as mentioned", "the previous", "as discussed"];
        let has_vague_refs = reference_patterns.iter()
            .any(|&pattern| memory.text.to_lowercase().contains(pattern));

        if has_vague_refs {
            issues.push("Contains vague references ('see above', 'as mentioned'). Note may not be self-contained.".to_string());
            score -= 0.2;
        }

        // Check 6: Topic focus - should have clear entities
        if memory.entities.is_empty() {
            issues.push("No entities identified. Atomic notes should have clear subject matter.".to_string());
            score -= 0.1;
        }

        score = score.max(0.0);

        let is_atomic = score >= 0.7;
        let recommendation = if is_atomic {
            "Note follows atomic principles well.".to_string()
        } else if score >= 0.5 {
            "Note could be improved. Consider refining to focus on a single idea.".to_string()
        } else {
            "Note likely contains multiple ideas. Strongly consider splitting into atomic notes.".to_string()
        };

        Ok(AtomicityCheck {
            memory_id: memory_id.clone(),
            is_atomic,
            score,
            word_count,
            issues,
            recommendation,
        })
    }

    /// Get all memories that may need to be split (low atomicity)
    pub fn get_non_atomic_memories(
        &self,
        threshold: f32,
        limit: Option<usize>,
    ) -> Result<Vec<(Memory, AtomicityCheck)>> {
        // Get all permanent memories (fleeting notes are expected to be rough)
        let memories = self.get_memories_by_status("permanent", None)?;

        let mut results = Vec::new();
        for memory in memories {
            let check = self.check_atomicity(&memory.id)?;
            if check.score < threshold {
                results.push((memory, check));
            }

            if let Some(lim) = limit {
                if results.len() >= lim {
                    break;
                }
            }
        }

        // Sort by score (worst first)
        results.sort_by(|a, b| a.1.score.partial_cmp(&b.1.score).unwrap());

        Ok(results)
    }

    /// Suggest splitting a note based on structure
    pub fn suggest_note_splits(&self, memory_id: &MemoryId) -> Result<Vec<NoteSplitSuggestion>> {
        let memory = self.get_memory_by_id(memory_id)?
            .ok_or_else(|| anyhow::anyhow!("Memory not found"))?;

        let mut suggestions = Vec::new();

        // Split by headings (if markdown)
        let heading_pattern = regex::Regex::new(r"(?m)^#+\s+(.+)$")?;
        let headings: Vec<_> = heading_pattern.captures_iter(&memory.text).collect();

        if headings.len() > 1 {
            suggestions.push(NoteSplitSuggestion {
                reason: format!("Found {} headings - each could be a separate note", headings.len()),
                split_points: headings.iter()
                    .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
                    .collect(),
            });
        }

        // Split by distinct topics (using entities as proxy)
        if memory.entities.len() > 3 {
            suggestions.push(NoteSplitSuggestion {
                reason: format!("Contains {} distinct entities - may cover multiple topics", memory.entities.len()),
                split_points: memory.entities.clone(),
            });
        }

        // Split by numbered lists
        let numbered_pattern = regex::Regex::new(r"(?m)^\d+\.\s+(.+)$")?;
        let numbered_items: Vec<_> = numbered_pattern.captures_iter(&memory.text).collect();

        if numbered_items.len() > 5 {
            suggestions.push(NoteSplitSuggestion {
                reason: format!("Contains {} numbered items - consider one note per item", numbered_items.len()),
                split_points: numbered_items.iter()
                    .take(5)
                    .filter_map(|cap| cap.get(1).map(|m| {
                        let s = m.as_str();
                        if s.len() > 50 {
                            format!("{}...", &s[..50])
                        } else {
                            s.to_string()
                        }
                    }))
                    .collect(),
            });
        }

        Ok(suggestions)
    }

    /// Get atomicity statistics for all permanent memories
    pub fn get_atomicity_stats(&self) -> Result<AtomicityStats> {
        let permanent = self.get_memories_by_status("permanent", None)?;

        let mut total_checked = 0;
        let mut atomic_count = 0;
        let mut needs_improvement = 0;
        let mut needs_splitting = 0;

        for memory in permanent {
            let check = self.check_atomicity(&memory.id)?;
            total_checked += 1;

            if check.score >= 0.7 {
                atomic_count += 1;
            } else if check.score >= 0.5 {
                needs_improvement += 1;
            } else {
                needs_splitting += 1;
            }
        }

        Ok(AtomicityStats {
            total_checked,
            atomic_count,
            needs_improvement,
            needs_splitting,
        })
    }

    // ========== PROJECT-BASED CONTEXT RETRIEVAL ==========

    /// Get all memories for a project (across all areas and topics)
    pub fn get_project_memories(
        &self,
        project_id: &memory_layer_schemas::ProjectId,
        limit: Option<usize>,
    ) -> Result<Vec<memory_layer_schemas::Memory>> {
        let limit_clause = if let Some(l) = limit {
            format!("LIMIT {}", l)
        } else {
            String::new()
        };

        let query = format!(
            "SELECT m.id, m.kind, m.topic, m.text, m.snippet_title, m.snippet_text,
                    m.snippet_loc, m.snippet_language, m.entities, m.provenance,
                    m.created_at, m.ttl
             FROM memories m
             JOIN topics t ON m.topic_id = t.id
             JOIN areas a ON t.area_id = a.id
             WHERE a.project_id = ?1
             ORDER BY m.created_at DESC
             {}",
            limit_clause
        );

        let mut stmt = self.conn.prepare(&query)?;
        let memories = stmt
            .query_map(params![project_id.0], |row| self.row_to_memory(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(memories)
    }

    /// Get project summary with stats
    pub fn get_project_summary(
        &self,
        project_id: &memory_layer_schemas::ProjectId,
    ) -> Result<ProjectSummary> {
        // Get project details
        let (project_name, project_description): (String, Option<String>) = self.conn.query_row(
            "SELECT name, description FROM projects WHERE id = ?1",
            params![project_id.0],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        // Count memories
        let memory_count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM memories m
             JOIN topics t ON m.topic_id = t.id
             JOIN areas a ON t.area_id = a.id
             WHERE a.project_id = ?1",
            params![project_id.0],
            |row| row.get(0),
        )?;

        // Count topics
        let topic_count: usize = self.conn.query_row(
            "SELECT COUNT(DISTINCT t.id) FROM topics t
             JOIN areas a ON t.area_id = a.id
             WHERE a.project_id = ?1",
            params![project_id.0],
            |row| row.get(0),
        )?;

        // Count areas
        let area_count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM areas WHERE project_id = ?1",
            params![project_id.0],
            |row| row.get(0),
        )?;

        // Get most recent activity
        let last_activity: Option<String> = self.conn.query_row(
            "SELECT m.created_at FROM memories m
             JOIN topics t ON m.topic_id = t.id
             JOIN areas a ON t.area_id = a.id
             WHERE a.project_id = ?1
             ORDER BY m.created_at DESC
             LIMIT 1",
            params![project_id.0],
            |row| row.get(0),
        ).ok();

        // Get top entities
        let mut stmt = self.conn.prepare(
            "SELECT m.entities FROM memories m
             JOIN topics t ON m.topic_id = t.id
             JOIN areas a ON t.area_id = a.id
             WHERE a.project_id = ?1 AND m.entities != '[]'
             LIMIT 100"
        )?;

        let mut entity_counts = std::collections::HashMap::new();
        let entities_iter = stmt.query_map(params![project_id.0], |row| {
            let entities_json: String = row.get(0)?;
            Ok(entities_json)
        })?;

        for entities_json_result in entities_iter {
            if let Ok(entities_json) = entities_json_result {
                if let Ok(entities) = serde_json::from_str::<Vec<String>>(&entities_json) {
                    for entity in entities {
                        *entity_counts.entry(entity).or_insert(0) += 1;
                    }
                }
            }
        }

        let mut top_entities: Vec<_> = entity_counts.into_iter().collect();
        top_entities.sort_by(|a, b| b.1.cmp(&a.1));
        let top_entities: Vec<String> = top_entities.into_iter().take(10).map(|(e, _)| e).collect();

        Ok(ProjectSummary {
            project_id: project_id.clone(),
            project_name,
            project_description,
            memory_count,
            topic_count,
            area_count,
            last_activity,
            top_entities,
        })
    }

    /// Get related projects based on shared entities
    pub fn get_related_projects(
        &self,
        project_id: &memory_layer_schemas::ProjectId,
        limit: usize,
    ) -> Result<Vec<(memory_layer_schemas::ProjectId, String, usize)>> {
        // Get entities from current project
        let mut stmt = self.conn.prepare(
            "SELECT m.entities FROM memories m
             JOIN topics t ON m.topic_id = t.id
             JOIN areas a ON t.area_id = a.id
             WHERE a.project_id = ?1 AND m.entities != '[]'"
        )?;

        let mut current_entities = std::collections::HashSet::new();
        let entities_iter = stmt.query_map(params![project_id.0], |row| {
            let entities_json: String = row.get(0)?;
            Ok(entities_json)
        })?;

        for entities_json_result in entities_iter {
            if let Ok(entities_json) = entities_json_result {
                if let Ok(entities) = serde_json::from_str::<Vec<String>>(&entities_json) {
                    for entity in entities {
                        current_entities.insert(entity);
                    }
                }
            }
        }

        // Find other projects with shared entities
        let mut project_scores: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        let mut all_stmt = self.conn.prepare(
            "SELECT DISTINCT a.project_id, m.entities FROM memories m
             JOIN topics t ON m.topic_id = t.id
             JOIN areas a ON t.area_id = a.id
             WHERE a.project_id != ?1 AND m.entities != '[]'"
        )?;

        let all_iter = all_stmt.query_map(params![project_id.0], |row| {
            let proj_id: String = row.get(0)?;
            let entities_json: String = row.get(1)?;
            Ok((proj_id, entities_json))
        })?;

        for result in all_iter {
            if let Ok((proj_id, entities_json)) = result {
                if let Ok(entities) = serde_json::from_str::<Vec<String>>(&entities_json) {
                    let shared_count = entities.iter().filter(|e| current_entities.contains(*e)).count();
                    if shared_count > 0 {
                        *project_scores.entry(proj_id).or_insert(0) += shared_count;
                    }
                }
            }
        }

        // Sort by score and get project names
        let mut scored_projects: Vec<_> = project_scores.into_iter().collect();
        scored_projects.sort_by(|a, b| b.1.cmp(&a.1));

        let mut results = Vec::new();
        for (proj_id, score) in scored_projects.into_iter().take(limit) {
            let name: String = self.conn.query_row(
                "SELECT name FROM projects WHERE id = ?1",
                params![&proj_id],
                |row| row.get(0),
            )?;
            results.push((memory_layer_schemas::ProjectId(proj_id), name, score));
        }

        Ok(results)
    }

    /// Get recent activity in a project
    pub fn get_project_activity(
        &self,
        project_id: &memory_layer_schemas::ProjectId,
        days: usize,
    ) -> Result<Vec<memory_layer_schemas::Memory>> {
        let cutoff_date = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_str = cutoff_date.to_rfc3339();

        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.kind, m.topic, m.text, m.snippet_title, m.snippet_text,
                    m.snippet_loc, m.snippet_language, m.entities, m.provenance,
                    m.created_at, m.ttl
             FROM memories m
             JOIN topics t ON m.topic_id = t.id
             JOIN areas a ON t.area_id = a.id
             WHERE a.project_id = ?1 AND m.created_at > ?2
             ORDER BY m.created_at DESC"
        )?;

        let memories = stmt
            .query_map(params![project_id.0, cutoff_str], |row| self.row_to_memory(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(memories)
    }

    // ========== TEMPORAL VIEWS ==========

    /// Get memories from this week
    pub fn get_this_week_memories(&self) -> Result<Vec<memory_layer_schemas::Memory>> {
        let week_ago = chrono::Utc::now() - chrono::Duration::weeks(1);
        self.get_memories_since(week_ago.to_rfc3339())
    }

    /// Get memories from this month
    pub fn get_this_month_memories(&self) -> Result<Vec<memory_layer_schemas::Memory>> {
        let month_ago = chrono::Utc::now() - chrono::Duration::days(30);
        self.get_memories_since(month_ago.to_rfc3339())
    }

    /// Get memories from this year
    pub fn get_this_year_memories(&self) -> Result<Vec<memory_layer_schemas::Memory>> {
        let year_ago = chrono::Utc::now() - chrono::Duration::days(365);
        self.get_memories_since(year_ago.to_rfc3339())
    }

    /// Get memories since a specific timestamp
    pub fn get_memories_since(&self, since: String) -> Result<Vec<memory_layer_schemas::Memory>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, topic, text, snippet_title, snippet_text,
                    snippet_loc, snippet_language, entities, provenance,
                    created_at, ttl
             FROM memories
             WHERE created_at >= ?1
             ORDER BY created_at DESC"
        )?;

        let memories = stmt
            .query_map(params![since], |row| self.row_to_memory(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(memories)
    }

    /// Get memories within a date range
    pub fn get_memories_in_range(
        &self,
        start: String,
        end: String,
    ) -> Result<Vec<memory_layer_schemas::Memory>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, topic, text, snippet_title, snippet_text,
                    snippet_loc, snippet_language, entities, provenance,
                    created_at, ttl
             FROM memories
             WHERE created_at BETWEEN ?1 AND ?2
             ORDER BY created_at DESC"
        )?;

        let memories = stmt
            .query_map(params![start, end], |row| self.row_to_memory(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(memories)
    }

    /// Get activity timeline grouped by date
    pub fn get_activity_timeline(&self, days: usize) -> Result<Vec<ActivityDay>> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_str = cutoff.to_rfc3339();

        let mut stmt = self.conn.prepare(
            "SELECT DATE(created_at) as day, COUNT(*) as count
             FROM memories
             WHERE created_at >= ?1
             GROUP BY DATE(created_at)
             ORDER BY day DESC"
        )?;

        let days = stmt
            .query_map(params![cutoff_str], |row| {
                Ok(ActivityDay {
                    date: row.get(0)?,
                    memory_count: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(days)
    }

    /// Get trending topics over a time period
    pub fn get_trending_topics(&self, days: usize, limit: usize) -> Result<Vec<TrendingTopic>> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_str = cutoff.to_rfc3339();

        let mut stmt = self.conn.prepare(
            "SELECT t.name, COUNT(m.id) as memory_count
             FROM memories m
             JOIN topics t ON m.topic_id = t.id
             WHERE m.created_at >= ?1
             GROUP BY t.id, t.name
             ORDER BY memory_count DESC
             LIMIT ?2"
        )?;

        let topics = stmt
            .query_map(params![cutoff_str, limit], |row| {
                Ok(TrendingTopic {
                    topic_name: row.get(0)?,
                    memory_count: row.get(1)?,
                    period_days: days,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(topics)
    }

    /// Get memory creation velocity (memories per day over time)
    pub fn get_creation_velocity(&self, days: usize) -> Result<Vec<(String, f32)>> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_str = cutoff.to_rfc3339();

        // Group by week for longer periods, by day for shorter
        let group_by = if days > 60 {
            "strftime('%Y-%W', created_at)"
        } else {
            "DATE(created_at)"
        };

        let query = format!(
            "SELECT {} as period, COUNT(*) as count
             FROM memories
             WHERE created_at >= ?1
             GROUP BY period
             ORDER BY period ASC",
            group_by
        );

        let mut stmt = self.conn.prepare(&query)?;
        let results = stmt
            .query_map(params![cutoff_str], |row| {
                let period: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((period, count as f32))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(results)
    }

    // ========== ENTITY-CENTRIC NAVIGATION ==========

    /// Get all memories mentioning a specific entity
    pub fn get_memories_by_entity(&self, entity: &str) -> Result<Vec<memory_layer_schemas::Memory>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, topic, text, snippet_title, snippet_text,
                    snippet_loc, snippet_language, entities, provenance,
                    created_at, ttl
             FROM memories
             WHERE entities LIKE ?1
             ORDER BY created_at DESC"
        )?;

        let pattern = format!("%\"{}\"% ", entity);
        let memories = stmt
            .query_map(params![pattern], |row| self.row_to_memory(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(memories)
    }

    /// Get entity evolution - how an entity has been referenced over time
    pub fn get_entity_evolution(&self, entity: &str, days: usize) -> Result<Vec<EntityMention>> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_str = cutoff.to_rfc3339();

        let mut stmt = self.conn.prepare(
            "SELECT id, text, created_at
             FROM memories
             WHERE entities LIKE ?1 AND created_at >= ?2
             ORDER BY created_at ASC"
        )?;

        let pattern = format!("%\"{}\"% ", entity);
        let mentions = stmt
            .query_map(params![pattern, cutoff_str], |row| {
                Ok(EntityMention {
                    memory_id: memory_layer_schemas::MemoryId(row.get(0)?),
                    entity: entity.to_string(),
                    context: row.get::<_, String>(1)?,
                    mentioned_at: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mentions)
    }

    /// Get entity co-occurrence - what entities frequently appear with this entity
    pub fn get_entity_cooccurrence(&self, entity: &str, limit: usize) -> Result<Vec<(String, usize)>> {
        let mut stmt = self.conn.prepare(
            "SELECT entities
             FROM memories
             WHERE entities LIKE ?1"
        )?;

        let pattern = format!("%\"{}\"% ", entity);
        let entities_iter = stmt.query_map(params![pattern], |row| {
            let entities_json: String = row.get(0)?;
            Ok(entities_json)
        })?;

        let mut cooccurrence_counts = std::collections::HashMap::new();

        for entities_json_result in entities_iter {
            if let Ok(entities_json) = entities_json_result {
                if let Ok(entities) = serde_json::from_str::<Vec<String>>(&entities_json) {
                    for other_entity in entities {
                        if other_entity != entity {
                            *cooccurrence_counts.entry(other_entity).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        let mut cooccurrences: Vec<_> = cooccurrence_counts.into_iter().collect();
        cooccurrences.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(cooccurrences.into_iter().take(limit).collect())
    }

    /// Find all unique entities across memories
    pub fn get_all_entities(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT entities FROM memories WHERE entities != '[]'"
        )?;

        let mut all_entities = std::collections::HashSet::new();
        let entities_iter = stmt.query_map([], |row| {
            let entities_json: String = row.get(0)?;
            Ok(entities_json)
        })?;

        for entities_json_result in entities_iter {
            if let Ok(entities_json) = entities_json_result {
                if let Ok(entities) = serde_json::from_str::<Vec<String>>(&entities_json) {
                    for entity in entities {
                        all_entities.insert(entity);
                    }
                }
            }
        }

        let mut entities: Vec<_> = all_entities.into_iter().collect();
        entities.sort();

        Ok(entities)
    }

    /// Get entity statistics
    pub fn get_entity_stats(&self, entity: &str) -> Result<EntityStats> {
        let pattern = format!("%\"{}\"% ", entity);

        // Count total mentions
        let mention_count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE entities LIKE ?1",
            params![pattern],
            |row| row.get(0),
        )?;

        // Get first and last mention
        let first_mention: Option<String> = self.conn.query_row(
            "SELECT created_at FROM memories WHERE entities LIKE ?1 ORDER BY created_at ASC LIMIT 1",
            params![pattern],
            |row| row.get(0),
        ).ok();

        let last_mention: Option<String> = self.conn.query_row(
            "SELECT created_at FROM memories WHERE entities LIKE ?1 ORDER BY created_at DESC LIMIT 1",
            params![pattern],
            |row| row.get(0),
        ).ok();

        // Get related topics
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT t.name
             FROM memories m
             JOIN topics t ON m.topic_id = t.id
             WHERE m.entities LIKE ?1
             LIMIT 10"
        )?;

        let related_topics = stmt
            .query_map(params![pattern], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(EntityStats {
            entity: entity.to_string(),
            mention_count,
            first_mention,
            last_mention,
            related_topics,
        })
    }

    // ========== IMPORTANCE-BASED FILTERING ==========

    /// Get memories by minimum importance threshold
    pub fn get_memories_by_importance(
        &self,
        min_importance: u8,
        limit: Option<usize>,
    ) -> Result<Vec<memory_layer_schemas::Memory>> {
        let limit_clause = if let Some(l) = limit {
            format!("LIMIT {}", l)
        } else {
            String::new()
        };

        let query = format!(
            "SELECT id, kind, topic, text, snippet_title, snippet_text,
                    snippet_loc, snippet_language, entities, provenance,
                    created_at, ttl
             FROM memories
             WHERE importance >= ?1
             ORDER BY importance DESC, created_at DESC
             {}",
            limit_clause
        );

        let mut stmt = self.conn.prepare(&query)?;
        let memories = stmt
            .query_map(params![min_importance], |row| self.row_to_memory(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(memories)
    }

    /// Update memory importance
    pub fn update_memory_importance(
        &self,
        memory_id: &MemoryId,
        importance: u8,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE memories SET importance = ?1 WHERE id = ?2",
            params![importance, memory_id.0],
        )?;
        Ok(())
    }

    /// Auto-calculate importance based on relationships and references
    /// Returns the calculated importance score
    pub fn calculate_memory_importance(&self, memory_id: &MemoryId) -> Result<u8> {
        let mut importance = 5u8; // Base importance

        // Factor 1: Number of incoming relationships (+1 per relationship, max +3)
        let incoming_count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM memory_relations WHERE target_id = ?1",
            params![memory_id.0],
            |row| row.get(0),
        ).unwrap_or(0);
        importance = importance.saturating_add((incoming_count.min(3)) as u8);

        // Factor 2: Number of outgoing relationships (+0.5 per relationship, max +2)
        let outgoing_count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM memory_relations WHERE source_id = ?1",
            params![memory_id.0],
            |row| row.get(0),
        ).unwrap_or(0);
        importance = importance.saturating_add((outgoing_count.min(4) / 2) as u8);

        // Factor 3: Is permanent (+2)
        let status: String = self.conn.query_row(
            "SELECT status FROM memories WHERE id = ?1",
            params![memory_id.0],
            |row| row.get(0),
        ).unwrap_or_else(|_| "fleeting".to_string());

        if status == "permanent" {
            importance = importance.saturating_add(2);
        }

        // Factor 4: Has progressive summaries (+1 per layer, max +2)
        let summary_count: usize = self.conn.query_row(
            "SELECT COUNT(DISTINCT layer) FROM progressive_summaries WHERE memory_id = ?1",
            params![memory_id.0],
            |row| row.get(0),
        ).unwrap_or(0);
        importance = importance.saturating_add((summary_count.min(2)) as u8);

        // Factor 5: Referenced in index notes (+3)
        let in_index: bool = self.conn.query_row(
            "SELECT 1 FROM index_notes WHERE key_memories LIKE ?1 LIMIT 1",
            params![format!("%{}%", memory_id.0)],
            |_| Ok(true),
        ).unwrap_or(false);

        if in_index {
            importance = importance.saturating_add(3);
        }

        // Cap at 10
        importance = importance.min(10);

        Ok(importance)
    }

    /// Auto-calculate and update importance for a memory
    pub fn recalculate_and_update_importance(&self, memory_id: &MemoryId) -> Result<u8> {
        let importance = self.calculate_memory_importance(memory_id)?;
        self.update_memory_importance(memory_id, importance)?;
        Ok(importance)
    }

    /// Get most important memories (importance >= 8)
    pub fn get_high_priority_memories(&self, limit: usize) -> Result<Vec<memory_layer_schemas::Memory>> {
        self.get_memories_by_importance(8, Some(limit))
    }

    /// Get importance distribution stats
    pub fn get_importance_stats(&self) -> Result<ImportanceStats> {
        let mut distribution = vec![0; 11]; // 0-10 importance levels

        let mut stmt = self.conn.prepare(
            "SELECT importance, COUNT(*) FROM memories GROUP BY importance"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, u8>(0)?, row.get::<_, usize>(1)?))
        })?;

        for row_result in rows {
            if let Ok((importance, count)) = row_result {
                if (importance as usize) < distribution.len() {
                    distribution[importance as usize] = count;
                }
            }
        }

        let total: usize = distribution.iter().sum();
        let average = if total > 0 {
            distribution.iter().enumerate().map(|(i, &count)| i * count).sum::<usize>() as f32 / total as f32
        } else {
            0.0
        };

        Ok(ImportanceStats {
            distribution,
            average,
            total_memories: total,
        })
    }

    /// Batch recalculate importance for all memories in a topic
    pub fn recalculate_topic_importance(&self, topic_id: &memory_layer_schemas::TopicId) -> Result<usize> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM memories WHERE topic_id = ?1"
        )?;

        let memory_ids: Vec<String> = stmt
            .query_map(params![topic_id.0], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut updated = 0;
        for id in memory_ids {
            let memory_id = MemoryId(id);
            if self.recalculate_and_update_importance(&memory_id).is_ok() {
                updated += 1;
            }
        }

        Ok(updated)
    }

    // ========== MEMORY RELATIONSHIP METHODS ==========

    /// Create a typed relationship between two memories
    pub fn create_memory_relation(
        &self,
        source_id: &MemoryId,
        target_id: &MemoryId,
        relation_type: memory_layer_schemas::RelationType,
        rationale: Option<&str>,
    ) -> Result<memory_layer_schemas::RelationId> {
        // Check if both memories exist
        let source_exists: bool = self
            .conn
            .query_row(
                "SELECT 1 FROM memories WHERE id = ?1",
                params![source_id.0],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);

        let target_exists: bool = self
            .conn
            .query_row(
                "SELECT 1 FROM memories WHERE id = ?1",
                params![target_id.0],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);

        if !source_exists {
            anyhow::bail!("Source memory {} does not exist", source_id.0);
        }
        if !target_exists {
            anyhow::bail!("Target memory {} does not exist", target_id.0);
        }

        // Check if relation already exists
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM memory_relations
                 WHERE source_id = ?1 AND target_id = ?2 AND relation_type = ?3",
                params![source_id.0, target_id.0, relation_type.as_str()],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(existing_id) = existing {
            info!(
                "Relation already exists: {} -{:?}-> {}",
                source_id.0, relation_type, target_id.0
            );
            return Ok(memory_layer_schemas::RelationId(existing_id));
        }

        // Create new relation
        let relation_id = memory_layer_schemas::generate_relation_id();
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO memory_relations (id, source_id, target_id, relation_type, rationale, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                relation_id.0,
                source_id.0,
                target_id.0,
                relation_type.as_str(),
                rationale,
                now
            ],
        )?;

        // Special handling for supersedes relation
        if relation_type == memory_layer_schemas::RelationType::Supersedes {
            // Update the target memory's superseded_by field
            self.conn.execute(
                "UPDATE memories SET superseded_by = ?1 WHERE id = ?2",
                params![source_id.0, target_id.0],
            )?;
            info!("Memory {} superseded by {}", target_id.0, source_id.0);
        }

        info!(
            "Created relation: {} -{:?}-> {} ({})",
            source_id.0, relation_type, target_id.0, relation_id.0
        );
        Ok(relation_id)
    }

    /// Get all outgoing relations from a memory
    pub fn get_memory_relations_from(
        &self,
        source_id: &MemoryId,
    ) -> Result<Vec<memory_layer_schemas::MemoryRelation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, target_id, relation_type, rationale, created_at
             FROM memory_relations
             WHERE source_id = ?1
             ORDER BY created_at DESC",
        )?;

        let relations = stmt
            .query_map(params![source_id.0], |row| {
                let relation_type_str: String = row.get(3)?;
                let relation_type = parse_relation_type(&relation_type_str);

                Ok(memory_layer_schemas::MemoryRelation {
                    id: memory_layer_schemas::RelationId(row.get(0)?),
                    source_id: MemoryId(row.get(1)?),
                    target_id: MemoryId(row.get(2)?),
                    relation_type,
                    rationale: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(relations)
    }

    /// Get all incoming relations to a memory
    pub fn get_memory_relations_to(
        &self,
        target_id: &MemoryId,
    ) -> Result<Vec<memory_layer_schemas::MemoryRelation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, target_id, relation_type, rationale, created_at
             FROM memory_relations
             WHERE target_id = ?1
             ORDER BY created_at DESC",
        )?;

        let relations = stmt
            .query_map(params![target_id.0], |row| {
                let relation_type_str: String = row.get(3)?;
                let relation_type = parse_relation_type(&relation_type_str);

                Ok(memory_layer_schemas::MemoryRelation {
                    id: memory_layer_schemas::RelationId(row.get(0)?),
                    source_id: MemoryId(row.get(1)?),
                    target_id: MemoryId(row.get(2)?),
                    relation_type,
                    rationale: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(relations)
    }

    /// Get all relations (both incoming and outgoing) for a memory
    pub fn get_all_memory_relations(
        &self,
        memory_id: &MemoryId,
    ) -> Result<Vec<memory_layer_schemas::MemoryRelation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, target_id, relation_type, rationale, created_at
             FROM memory_relations
             WHERE source_id = ?1 OR target_id = ?1
             ORDER BY created_at DESC",
        )?;

        let relations = stmt
            .query_map(params![memory_id.0], |row| {
                let relation_type_str: String = row.get(3)?;
                let relation_type = parse_relation_type(&relation_type_str);

                Ok(memory_layer_schemas::MemoryRelation {
                    id: memory_layer_schemas::RelationId(row.get(0)?),
                    source_id: MemoryId(row.get(1)?),
                    target_id: MemoryId(row.get(2)?),
                    relation_type,
                    rationale: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(relations)
    }

    /// Get memories related by a specific relation type
    pub fn get_related_memories_by_type(
        &self,
        memory_id: &MemoryId,
        relation_type: memory_layer_schemas::RelationType,
        direction: RelationDirection,
    ) -> Result<Vec<Memory>> {
        let query = match direction {
            RelationDirection::Outgoing => {
                "SELECT m.id, m.kind, m.topic, m.text, m.snippet_title, m.snippet_text,
                        m.snippet_loc, m.snippet_language, m.entities, m.provenance,
                        m.created_at, m.ttl
                 FROM memories m
                 JOIN memory_relations r ON m.id = r.target_id
                 WHERE r.source_id = ?1 AND r.relation_type = ?2
                 ORDER BY r.created_at DESC"
            }
            RelationDirection::Incoming => {
                "SELECT m.id, m.kind, m.topic, m.text, m.snippet_title, m.snippet_text,
                        m.snippet_loc, m.snippet_language, m.entities, m.provenance,
                        m.created_at, m.ttl
                 FROM memories m
                 JOIN memory_relations r ON m.id = r.source_id
                 WHERE r.target_id = ?1 AND r.relation_type = ?2
                 ORDER BY r.created_at DESC"
            }
            RelationDirection::Both => {
                "SELECT m.id, m.kind, m.topic, m.text, m.snippet_title, m.snippet_text,
                        m.snippet_loc, m.snippet_language, m.entities, m.provenance,
                        m.created_at, m.ttl
                 FROM memories m
                 JOIN memory_relations r ON (m.id = r.target_id OR m.id = r.source_id)
                 WHERE (r.source_id = ?1 OR r.target_id = ?1)
                   AND r.relation_type = ?2
                   AND m.id != ?1
                 ORDER BY r.created_at DESC"
            }
        };

        let mut stmt = self.conn.prepare(query)?;
        let memories = stmt
            .query_map(params![memory_id.0, relation_type.as_str()], |row| {
                self.row_to_memory(row)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(memories)
    }

    /// Delete a specific relation
    pub fn delete_memory_relation(
        &self,
        relation_id: &memory_layer_schemas::RelationId,
    ) -> Result<()> {
        // First get the relation details to handle special cases
        let relation: Option<(String, String, String)> = self
            .conn
            .query_row(
                "SELECT source_id, target_id, relation_type FROM memory_relations WHERE id = ?1",
                params![relation_id.0],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        if let Some((_source_id, target_id, relation_type)) = relation {
            // If it's a supersedes relation, clear the superseded_by field
            if relation_type == "supersedes" {
                self.conn.execute(
                    "UPDATE memories SET superseded_by = NULL WHERE id = ?1",
                    params![target_id],
                )?;
            }

            // Delete the relation
            self.conn.execute(
                "DELETE FROM memory_relations WHERE id = ?1",
                params![relation_id.0],
            )?;

            info!("Deleted relation: {}", relation_id.0);
        }

        Ok(())
    }

    /// Find potential supersedes relationships based on similarity
    pub fn find_potential_supersedes(
        &self,
        memory_id: &MemoryId,
        threshold: f32,
    ) -> Result<Vec<(MemoryId, f32)>> {
        // Get the memory content
        let memory = self.get_memory_by_id(memory_id)?;
        if memory.is_none() {
            return Ok(vec![]);
        }

        let memory = memory.unwrap();

        // Find similar memories in the same topic that are older
        let mut stmt = self.conn.prepare(
            "SELECT id, text, created_at
             FROM memories
             WHERE topic = ?1
               AND id != ?2
               AND created_at < ?3
             ORDER BY created_at DESC
             LIMIT 20",
        )?;

        let candidates = stmt
            .query_map(
                params![memory.topic, memory_id.0, memory.created_at],
                |row| {
                    Ok((
                        MemoryId(row.get(0)?),
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;

        // Calculate similarity scores (simple word overlap for now)
        let mut results = Vec::new();
        for (candidate_id, candidate_text, _) in candidates {
            let similarity = calculate_text_similarity(&memory.text, &candidate_text);
            if similarity >= threshold {
                results.push((candidate_id, similarity));
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        Ok(results)
    }

    /// Get a memory by ID
    fn get_memory_by_id(&self, memory_id: &MemoryId) -> Result<Option<Memory>> {
        self.conn
            .query_row(
                "SELECT id, kind, topic, text, snippet_title, snippet_text,
                        snippet_loc, snippet_language, entities, provenance,
                        created_at, ttl
                 FROM memories
                 WHERE id = ?1",
                params![memory_id.0],
                |row| self.row_to_memory(row),
            )
            .optional()
            .map_err(Into::into)
    }

    // ========== MEMORY VERSIONING METHODS ==========

    /// Create a new version snapshot when updating a memory
    pub fn create_memory_version(
        &self,
        memory_id: &MemoryId,
        old_content: &str,
        change_summary: Option<&str>,
    ) -> Result<memory_layer_schemas::VersionId> {
        // Get the current version number from the memory
        let current_version: Option<i64> = self
            .conn
            .query_row(
                "SELECT version FROM memories WHERE id = ?1",
                params![memory_id.0],
                |row| row.get(0),
            )
            .optional()?;

        let version_number = current_version.unwrap_or(1) as u32;

        // Create the version record
        let version_id = memory_layer_schemas::generate_version_id();
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO memory_versions (id, memory_id, content, version_number, change_summary, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                version_id.0,
                memory_id.0,
                old_content,
                version_number,
                change_summary,
                now
            ],
        )?;

        // Increment the version number in the memories table
        self.conn.execute(
            "UPDATE memories SET version = ?1 WHERE id = ?2",
            params![version_number + 1, memory_id.0],
        )?;

        info!(
            "Created version {} for memory {} ({})",
            version_number, memory_id.0, version_id.0
        );
        Ok(version_id)
    }

    /// Get all versions of a memory in reverse chronological order
    pub fn get_memory_versions(
        &self,
        memory_id: &MemoryId,
    ) -> Result<Vec<memory_layer_schemas::MemoryVersion>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, memory_id, content, version_number, change_summary, created_at
             FROM memory_versions
             WHERE memory_id = ?1
             ORDER BY version_number DESC",
        )?;

        let versions = stmt
            .query_map(params![memory_id.0], |row| {
                Ok(memory_layer_schemas::MemoryVersion {
                    id: memory_layer_schemas::VersionId(row.get(0)?),
                    memory_id: MemoryId(row.get(1)?),
                    content: row.get(2)?,
                    version_number: row.get::<_, i64>(3)? as u32,
                    change_summary: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(versions)
    }

    /// Get a specific version by ID
    pub fn get_version_by_id(
        &self,
        version_id: &memory_layer_schemas::VersionId,
    ) -> Result<Option<memory_layer_schemas::MemoryVersion>> {
        self.conn
            .query_row(
                "SELECT id, memory_id, content, version_number, change_summary, created_at
                 FROM memory_versions
                 WHERE id = ?1",
                params![version_id.0],
                |row| {
                    Ok(memory_layer_schemas::MemoryVersion {
                        id: memory_layer_schemas::VersionId(row.get(0)?),
                        memory_id: MemoryId(row.get(1)?),
                        content: row.get(2)?,
                        version_number: row.get::<_, i64>(3)? as u32,
                        change_summary: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Get a specific version by memory ID and version number
    pub fn get_version_by_number(
        &self,
        memory_id: &MemoryId,
        version_number: u32,
    ) -> Result<Option<memory_layer_schemas::MemoryVersion>> {
        self.conn
            .query_row(
                "SELECT id, memory_id, content, version_number, change_summary, created_at
                 FROM memory_versions
                 WHERE memory_id = ?1 AND version_number = ?2",
                params![memory_id.0, version_number as i64],
                |row| {
                    Ok(memory_layer_schemas::MemoryVersion {
                        id: memory_layer_schemas::VersionId(row.get(0)?),
                        memory_id: MemoryId(row.get(1)?),
                        content: row.get(2)?,
                        version_number: row.get::<_, i64>(3)? as u32,
                        change_summary: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Get the diff between two versions (simple word-level diff)
    pub fn get_version_diff(
        &self,
        memory_id: &MemoryId,
        from_version: u32,
        to_version: u32,
    ) -> Result<VersionDiff> {
        let from = self.get_version_by_number(memory_id, from_version)?;
        let to = self.get_version_by_number(memory_id, to_version)?;

        match (from, to) {
            (Some(from_ver), Some(to_ver)) => {
                let similarity = calculate_text_similarity(&from_ver.content, &to_ver.content);
                let words_changed = count_word_changes(&from_ver.content, &to_ver.content);

                Ok(VersionDiff {
                    from_version,
                    to_version,
                    from_content: from_ver.content,
                    to_content: to_ver.content,
                    similarity,
                    words_added: words_changed.0,
                    words_removed: words_changed.1,
                    from_created_at: from_ver.created_at,
                    to_created_at: to_ver.created_at,
                })
            }
            _ => anyhow::bail!(
                "Could not find both versions {} and {} for memory {}",
                from_version,
                to_version,
                memory_id.0
            ),
        }
    }

    /// Revert a memory to a previous version
    pub fn revert_memory_to_version(
        &self,
        memory_id: &MemoryId,
        target_version: u32,
        revert_reason: &str,
    ) -> Result<()> {
        // Get the target version
        let target = self
            .get_version_by_number(memory_id, target_version)?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Version {} not found for memory {}",
                    target_version,
                    memory_id.0
                )
            })?;

        // Get the current memory content to save it as a version
        let memory = self
            .get_memory_by_id(memory_id)?
            .ok_or_else(|| anyhow::anyhow!("Memory {} not found", memory_id.0))?;

        // Save current content as a version before reverting
        let change_summary = format!("Reverted to version {}: {}", target_version, revert_reason);
        self.create_memory_version(memory_id, &memory.text, Some(&change_summary))?;

        // Update the memory with the target version's content
        self.conn.execute(
            "UPDATE memories SET text = ?1 WHERE id = ?2",
            params![target.content, memory_id.0],
        )?;

        info!(
            "Reverted memory {} to version {}",
            memory_id.0, target_version
        );
        Ok(())
    }

    /// Prune old versions, keeping only the most recent N versions
    pub fn prune_old_versions(&self, memory_id: &MemoryId, keep_count: u32) -> Result<usize> {
        // Get all versions
        let versions = self.get_memory_versions(memory_id)?;

        if versions.len() <= keep_count as usize {
            return Ok(0);
        }

        // Keep the most recent keep_count versions, delete the rest
        let to_delete = &versions[keep_count as usize..];
        let mut deleted = 0;

        for version in to_delete {
            self.conn.execute(
                "DELETE FROM memory_versions WHERE id = ?1",
                params![version.id.0],
            )?;
            deleted += 1;
        }

        info!(
            "Pruned {} old versions for memory {}",
            deleted, memory_id.0
        );
        Ok(deleted)
    }

    /// Get version statistics for a memory
    pub fn get_version_stats(&self, memory_id: &MemoryId) -> Result<VersionStats> {
        let versions = self.get_memory_versions(memory_id)?;

        if versions.is_empty() {
            return Ok(VersionStats {
                total_versions: 0,
                oldest_version: None,
                newest_version: None,
                total_changes: 0,
            });
        }

        let oldest = versions.last().cloned();
        let newest = versions.first().cloned();
        let total_changes = versions.len();

        Ok(VersionStats {
            total_versions: versions.len(),
            oldest_version: oldest,
            newest_version: newest,
            total_changes,
        })
    }

    // ========== NARRATIVE QUERY METHODS ==========

    /// Get a decision chain: decision → implementations → outcomes
    pub fn get_decision_chain(&self, decision_memory_id: &MemoryId) -> Result<DecisionChain> {
        // Get the root decision memory
        let decision = self
            .get_memory_by_id(decision_memory_id)?
            .ok_or_else(|| anyhow::anyhow!("Decision memory {} not found", decision_memory_id.0))?;

        // Find all implementations of this decision
        let implementations = self.get_related_memories_by_type(
            decision_memory_id,
            memory_layer_schemas::RelationType::Implements,
            RelationDirection::Incoming,
        )?;

        // Find all questions about this decision
        let questions = self.get_related_memories_by_type(
            decision_memory_id,
            memory_layer_schemas::RelationType::Questions,
            RelationDirection::Incoming,
        )?;

        // Find all related context
        let related = self.get_related_memories_by_type(
            decision_memory_id,
            memory_layer_schemas::RelationType::RelatesTo,
            RelationDirection::Both,
        )?;

        Ok(DecisionChain {
            decision,
            implementations,
            questions,
            related_context: related,
        })
    }

    /// Get an evolution trail: show how a concept evolved through supersedes relationships
    pub fn get_evolution_trail(&self, memory_id: &MemoryId) -> Result<Vec<Memory>> {
        let mut trail = Vec::new();
        let mut current_id = memory_id.clone();

        // Walk backwards through supersedes relationships
        loop {
            let memory = self.get_memory_by_id(&current_id)?;
            if memory.is_none() {
                break;
            }

            let memory = memory.unwrap();
            trail.push(memory.clone());

            // Check if this memory supersedes another
            let supersedes = self.get_related_memories_by_type(
                &current_id,
                memory_layer_schemas::RelationType::Supersedes,
                RelationDirection::Outgoing,
            )?;

            if supersedes.is_empty() {
                break;
            }

            current_id = supersedes[0].id.clone();
        }

        // Reverse so oldest is first
        trail.reverse();
        Ok(trail)
    }

    /// Find contradictions: memories that contradict each other
    pub fn find_contradictions(&self, topic: &str) -> Result<Vec<(Memory, Memory, String)>> {
        // Get all memories in this topic
        let memories = self.get_memories_by_topic(topic, 100)?;

        let mut contradictions = Vec::new();

        // Check for explicit contradiction relationships
        for memory in &memories {
            let contradicted = self.get_related_memories_by_type(
                &memory.id,
                memory_layer_schemas::RelationType::Contradicts,
                RelationDirection::Outgoing,
            )?;

            for contradicted_memory in contradicted {
                let rationale = self
                    .get_memory_relations_from(&memory.id)?
                    .into_iter()
                    .find(|r| {
                        r.target_id == contradicted_memory.id
                            && r.relation_type == memory_layer_schemas::RelationType::Contradicts
                    })
                    .and_then(|r| r.rationale)
                    .unwrap_or_else(|| "No rationale provided".to_string());

                contradictions.push((memory.clone(), contradicted_memory, rationale));
            }
        }

        Ok(contradictions)
    }

    /// Get question resolution chain: questions → answers
    pub fn get_question_resolution(&self, question_memory_id: &MemoryId) -> Result<QuestionResolution> {
        let question = self
            .get_memory_by_id(question_memory_id)?
            .ok_or_else(|| anyhow::anyhow!("Question memory {} not found", question_memory_id.0))?;

        // Find memories that this question relates to (potential answers)
        let related_memories = self.get_related_memories_by_type(
            question_memory_id,
            memory_layer_schemas::RelationType::Questions,
            RelationDirection::Outgoing,
        )?;

        // Find newer memories in the same topic that might answer the question
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, topic, text, snippet_title, snippet_text,
                    snippet_loc, snippet_language, entities, provenance,
                    created_at, ttl
             FROM memories
             WHERE topic = ?1 AND created_at > ?2
             ORDER BY created_at ASC
             LIMIT 10",
        )?;

        let potential_answers = stmt
            .query_map(params![question.topic, question.created_at], |row| {
                self.row_to_memory(row)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(QuestionResolution {
            question,
            questioned_memories: related_memories,
            potential_answers,
        })
    }

    /// Get implementation tracking: find all implementations of a concept/decision
    pub fn get_implementation_tracking(&self, concept_memory_id: &MemoryId) -> Result<ImplementationTracking> {
        let concept = self
            .get_memory_by_id(concept_memory_id)?
            .ok_or_else(|| anyhow::anyhow!("Concept memory {} not found", concept_memory_id.0))?;

        // Find direct implementations
        let direct_implementations = self.get_related_memories_by_type(
            concept_memory_id,
            memory_layer_schemas::RelationType::Implements,
            RelationDirection::Incoming,
        )?;

        // Find examples
        let examples = self.get_related_memories_by_type(
            concept_memory_id,
            memory_layer_schemas::RelationType::Exemplifies,
            RelationDirection::Incoming,
        )?;

        // Find related concepts
        let related_concepts = self.get_related_memories_by_type(
            concept_memory_id,
            memory_layer_schemas::RelationType::RelatesTo,
            RelationDirection::Both,
        )?;

        Ok(ImplementationTracking {
            concept,
            direct_implementations,
            examples,
            related_concepts,
        })
    }

    /// Get full memory narrative: combines relationships, versions, and context
    pub fn get_memory_narrative(&self, memory_id: &MemoryId) -> Result<MemoryNarrative> {
        let memory = self
            .get_memory_by_id(memory_id)?
            .ok_or_else(|| anyhow::anyhow!("Memory {} not found", memory_id.0))?;

        // Get all relationships
        let relations = self.get_all_memory_relations(memory_id)?;

        // Get version history
        let versions = self.get_memory_versions(memory_id)?;

        // Get evolution trail
        let evolution_trail = self.get_evolution_trail(memory_id)?;

        // Categorize relations by type
        let mut supersedes = Vec::new();
        let mut implements = Vec::new();
        let mut questions = Vec::new();
        let mut relates_to = Vec::new();
        let mut contradicts = Vec::new();
        let mut exemplifies = Vec::new();

        for relation in &relations {
            let target_memory = if relation.source_id == *memory_id {
                self.get_memory_by_id(&relation.target_id)?
            } else {
                self.get_memory_by_id(&relation.source_id)?
            };

            if let Some(target) = target_memory {
                match relation.relation_type {
                    memory_layer_schemas::RelationType::Supersedes => supersedes.push(target),
                    memory_layer_schemas::RelationType::Implements => implements.push(target),
                    memory_layer_schemas::RelationType::Questions => questions.push(target),
                    memory_layer_schemas::RelationType::RelatesTo => relates_to.push(target),
                    memory_layer_schemas::RelationType::Contradicts => contradicts.push(target),
                    memory_layer_schemas::RelationType::Exemplifies => exemplifies.push(target),
                }
            }
        }

        Ok(MemoryNarrative {
            memory,
            relations,
            versions,
            evolution_trail,
            supersedes,
            implements,
            questions,
            relates_to,
            contradicts,
            exemplifies,
        })
    }
}

fn json_error(err: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(err))
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_layer_schemas::{
        generate_memory_id, generate_thread_id, generate_turn_id, MemoryKind, SourceApp, TurnSource,
    };
    use tempfile::NamedTempFile;

    #[test]
    fn test_database_creation() {
        let temp = NamedTempFile::new().unwrap();
        let db = Database::new(temp.path()).unwrap();

        assert_eq!(db.count_turns().unwrap(), 0);
        assert_eq!(db.count_memories().unwrap(), 0);
    }

    #[test]
    fn test_turn_insert_and_retrieve() {
        let temp = NamedTempFile::new().unwrap();
        let db = Database::new(temp.path()).unwrap();

        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: Utc::now().to_rfc3339(),
            user_text: "Hello world".to_string(),
            ts_ai: None,
            ai_text: None,
            source: TurnSource {
                app: SourceApp::Claude,
                url: None,
                path: None,
            },
        };

        db.insert_turn(&turn).unwrap();
        assert_eq!(db.count_turns().unwrap(), 1);

        let retrieved = db.get_turn(&turn.id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().user_text, "Hello world");
    }

    #[test]
    fn test_memory_insert_and_search() {
        let temp = NamedTempFile::new().unwrap();
        let db = Database::new(temp.path()).unwrap();

        let memory = Memory {
            id: generate_memory_id(),
            kind: MemoryKind::Fact,
            topic: "testing".to_string(),
            text: "This is a test memory".to_string(),
            snippet: None,
            entities: vec!["test".to_string()],
            provenance: vec![generate_turn_id()],
            created_at: Utc::now().to_rfc3339(),
            ttl: None,
        };

        db.insert_memory(&memory).unwrap();
        assert_eq!(db.count_memories().unwrap(), 1);

        let results = db.search_memories("test", 10).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_recent_memories_and_topics() {
        let temp = NamedTempFile::new().unwrap();
        let db = Database::new(temp.path()).unwrap();

        let memory_a = Memory {
            id: generate_memory_id(),
            kind: MemoryKind::Fact,
            topic: "onboarding".to_string(),
            text: "Initial setup complete".to_string(),
            snippet: None,
            entities: vec!["setup".into()],
            provenance: vec![generate_turn_id()],
            created_at: Utc::now().to_rfc3339(),
            ttl: None,
        };

        let memory_b = Memory {
            id: generate_memory_id(),
            kind: MemoryKind::Task,
            topic: "onboarding".to_string(),
            text: "Follow up on permissions".to_string(),
            snippet: None,
            entities: vec!["permissions".into()],
            provenance: vec![generate_turn_id()],
            created_at: Utc::now().to_rfc3339(),
            ttl: None,
        };

        let memory_c = Memory {
            id: generate_memory_id(),
            kind: MemoryKind::Fact,
            topic: "research".to_string(),
            text: "Investigating context capsule size".to_string(),
            snippet: None,
            entities: vec!["context".into()],
            provenance: vec![generate_turn_id()],
            created_at: Utc::now().to_rfc3339(),
            ttl: None,
        };

        db.insert_memory(&memory_a).unwrap();
        db.insert_memory(&memory_b).unwrap();
        db.insert_memory(&memory_c).unwrap();

        let recent = db.get_recent_memories(10).unwrap();
        assert_eq!(recent.len(), 3);

        let topics = db.topic_summaries(10).unwrap();
        assert_eq!(topics.len(), 2);
        assert_eq!(topics[0].topic, "onboarding");
        assert_eq!(topics[0].memory_count, 2);
    }

    #[test]
    fn test_agentic_metadata_generation() {
        let temp = NamedTempFile::new().unwrap();
        let db = Database::new(temp.path()).unwrap();

        let turn_id = generate_turn_id();

        let memory_a = Memory {
            id: generate_memory_id(),
            kind: MemoryKind::Fact,
            topic: "agentic".into(),
            text: "Agentic memory links related context across tasks.".into(),
            snippet: None,
            entities: vec!["agentic".into()],
            provenance: vec![turn_id.clone()],
            created_at: Utc::now().to_rfc3339(),
            ttl: None,
        };

        let memory_b = Memory {
            id: generate_memory_id(),
            kind: MemoryKind::Task,
            topic: "agentic".into(),
            text: "Review agentic memory base integration".into(),
            snippet: None,
            entities: vec!["integration".into()],
            provenance: vec![turn_id],
            created_at: Utc::now().to_rfc3339(),
            ttl: None,
        };

        db.insert_memory(&memory_a).unwrap();
        let agentic_a = db.upsert_agentic_memory(&memory_a).unwrap();
        assert!(!agentic_a.keywords.is_empty());
        assert!(agentic_a.tags.iter().any(|tag| tag.starts_with("kind:")));

        db.insert_memory(&memory_b).unwrap();
        let agentic_b = db.upsert_agentic_memory(&memory_b).unwrap();
        assert!(
            agentic_b
                .links
                .iter()
                .any(|link| link.target == memory_a.id),
            "Expected second memory to link to the first"
        );

        let stored_a = db.get_agentic_memory(&memory_a.id).unwrap().unwrap();
        assert!(
            stored_a.links.iter().any(|link| link.target == memory_b.id),
            "Expected first memory to gain reverse link to second"
        );
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TopicSummary {
    pub topic: String,
    pub memory_count: usize,
    pub last_memory_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub project_id: memory_layer_schemas::ProjectId,
    pub project_name: String,
    pub project_description: Option<String>,
    pub memory_count: usize,
    pub topic_count: usize,
    pub area_count: usize,
    pub last_activity: Option<String>,
    pub top_entities: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionDiff {
    pub from_version: u32,
    pub to_version: u32,
    pub from_content: String,
    pub to_content: String,
    pub similarity: f32,
    pub words_added: usize,
    pub words_removed: usize,
    pub from_created_at: String,
    pub to_created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionStats {
    pub total_versions: usize,
    pub oldest_version: Option<memory_layer_schemas::MemoryVersion>,
    pub newest_version: Option<memory_layer_schemas::MemoryVersion>,
    pub total_changes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DecisionChain {
    pub decision: Memory,
    pub implementations: Vec<Memory>,
    pub questions: Vec<Memory>,
    pub related_context: Vec<Memory>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuestionResolution {
    pub question: Memory,
    pub questioned_memories: Vec<Memory>,
    pub potential_answers: Vec<Memory>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImplementationTracking {
    pub concept: Memory,
    pub direct_implementations: Vec<Memory>,
    pub examples: Vec<Memory>,
    pub related_concepts: Vec<Memory>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryNarrative {
    pub memory: Memory,
    pub relations: Vec<memory_layer_schemas::MemoryRelation>,
    pub versions: Vec<memory_layer_schemas::MemoryVersion>,
    pub evolution_trail: Vec<Memory>,
    pub supersedes: Vec<Memory>,
    pub implements: Vec<Memory>,
    pub questions: Vec<Memory>,
    pub relates_to: Vec<Memory>,
    pub contradicts: Vec<Memory>,
    pub exemplifies: Vec<Memory>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexNote {
    pub id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub name: String,
    pub content: String,
    pub memory_count: i64,
    pub key_memories: Vec<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LifecycleStats {
    pub fleeting: usize,
    pub permanent: usize,
    pub archived: usize,
    pub deprecated: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgressiveSummary {
    pub id: String,
    pub memory_id: MemoryId,
    pub layer: u8,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SummarizationStats {
    pub total_memories: usize,
    pub summarized_count: usize,
    pub layer_1_count: usize,
    pub layer_2_count: usize,
    pub layer_3_count: usize,
    pub layer_4_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AtomicityCheck {
    pub memory_id: MemoryId,
    pub is_atomic: bool,
    pub score: f32,
    pub word_count: usize,
    pub issues: Vec<String>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NoteSplitSuggestion {
    pub reason: String,
    pub split_points: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AtomicityStats {
    pub total_checked: usize,
    pub atomic_count: usize,
    pub needs_improvement: usize,
    pub needs_splitting: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityDay {
    pub date: String,
    pub memory_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrendingTopic {
    pub topic_name: String,
    pub memory_count: usize,
    pub period_days: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityMention {
    pub memory_id: memory_layer_schemas::MemoryId,
    pub entity: String,
    pub context: String,
    pub mentioned_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityStats {
    pub entity: String,
    pub mention_count: usize,
    pub first_mention: Option<String>,
    pub last_mention: Option<String>,
    pub related_topics: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportanceStats {
    pub distribution: Vec<usize>,  // Count of memories at each importance level (0-10)
    pub average: f32,
    pub total_memories: usize,
}

fn parse_memory_kind(raw: &str) -> MemoryKind {
    match raw.to_lowercase().as_str() {
        "decision" => MemoryKind::Decision,
        "fact" => MemoryKind::Fact,
        "snippet" => MemoryKind::Snippet,
        "task" => MemoryKind::Task,
        other => {
            debug!("Unknown memory kind '{}', defaulting to Fact", other);
            MemoryKind::Fact
        }
    }
}

fn parse_relation_type(raw: &str) -> memory_layer_schemas::RelationType {
    use memory_layer_schemas::RelationType;
    match raw.to_lowercase().as_str() {
        "supersedes" => RelationType::Supersedes,
        "implements" => RelationType::Implements,
        "questions" => RelationType::Questions,
        "relates_to" => RelationType::RelatesTo,
        "contradicts" => RelationType::Contradicts,
        "exemplifies" => RelationType::Exemplifies,
        other => {
            debug!("Unknown relation type '{}', defaulting to RelatesTo", other);
            RelationType::RelatesTo
        }
    }
}

/// Calculate text similarity using Jaccard similarity of word sets
fn calculate_text_similarity(text1: &str, text2: &str) -> f32 {
    use std::collections::HashSet;

    let text1_lower = text1.to_lowercase();
    let text2_lower = text2.to_lowercase();

    let words1: HashSet<String> = text1_lower
        .split_whitespace()
        .filter(|w| w.len() >= 3 && !STOPWORDS.contains(w))
        .map(|w| w.to_string())
        .collect();

    let words2: HashSet<String> = text2_lower
        .split_whitespace()
        .filter(|w| w.len() >= 3 && !STOPWORDS.contains(w))
        .map(|w| w.to_string())
        .collect();

    if words1.is_empty() && words2.is_empty() {
        return 0.0;
    }

    let intersection = words1.intersection(&words2).count();
    let union = words1.union(&words2).count();

    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

/// Count word changes between two texts (added, removed)
fn count_word_changes(text1: &str, text2: &str) -> (usize, usize) {
    use std::collections::HashSet;

    let text1_lower = text1.to_lowercase();
    let text2_lower = text2.to_lowercase();

    let words1: HashSet<String> = text1_lower
        .split_whitespace()
        .filter(|w| w.len() >= 3 && !STOPWORDS.contains(w))
        .map(|w| w.to_string())
        .collect();

    let words2: HashSet<String> = text2_lower
        .split_whitespace()
        .filter(|w| w.len() >= 3 && !STOPWORDS.contains(w))
        .map(|w| w.to_string())
        .collect();

    // Words in text2 but not in text1 = added
    let added = words2.difference(&words1).count();

    // Words in text1 but not in text2 = removed
    let removed = words1.difference(&words2).count();

    (added, removed)
}
