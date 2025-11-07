use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// ULID and ID Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TurnId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThreadId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapsuleId(pub String);

impl fmt::Display for TurnId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for ThreadId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for MemoryId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for CapsuleId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// Turn Schema
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub id: TurnId,
    pub thread_id: ThreadId,
    pub ts_user: String, // RFC3339
    pub user_text: String,
    pub ts_ai: Option<String>, // RFC3339
    pub ai_text: Option<String>,
    pub source: TurnSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnSource {
    pub app: SourceApp,
    pub url: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceApp {
    Claude,
    ChatGPT,
    VSCode,
    Mail,
    Notes,
    Terminal,
    Other,
}

// ============================================================================
// Memory Schema
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: MemoryId,
    pub kind: MemoryKind,
    pub topic: String,
    pub text: String,
    pub snippet: Option<Snippet>,
    pub entities: Vec<String>,
    pub provenance: Vec<TurnId>,
    pub created_at: String, // RFC3339
    pub ttl: Option<u64>,
    pub topic_id: Option<TopicId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryKind {
    #[serde(rename = "decision")]
    Decision,
    #[serde(rename = "fact")]
    Fact,
    #[serde(rename = "snippet")]
    Snippet,
    #[serde(rename = "task")]
    Task,
}

impl MemoryKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryKind::Decision => "decision",
            MemoryKind::Fact => "fact",
            MemoryKind::Snippet => "snippet",
            MemoryKind::Task => "task",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub title: String,
    pub text: String,
    pub loc: Option<String>, // e.g., "L18-L44"
    pub language: Option<String>,
}

// ============================================================================
// Agentic Memory Schema (inspired by A-mem)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticMemory {
    pub id: MemoryId,
    pub content: String,
    pub context: String,
    pub keywords: Vec<String>,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub links: Vec<AgenticLink>,
    pub retrieval_count: u32,
    pub last_accessed: String, // RFC3339
    pub created_at: String,    // RFC3339
    pub evolution_history: Vec<AgenticEvolution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticLink {
    pub target: MemoryId,
    pub strength: f32,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticEvolution {
    pub timestamp: String, // RFC3339
    pub summary: String,
    pub changes: Option<Vec<String>>,
}

// ============================================================================
// Context Capsule Schema
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCapsule {
    pub capsule_id: CapsuleId,
    pub preamble_text: String,
    pub messages: Vec<Message>,
    pub provenance: Vec<ProvenanceItem>,
    pub delta_of: Option<CapsuleId>,
    pub ttl_sec: u64,
    pub token_count: Option<u64>,
    pub style: Option<ContextStyle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceItem {
    pub r#type: ProvenanceType,
    pub r#ref: String,
    pub when: Option<String>, // RFC3339
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProvenanceType {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "file")]
    File,
    #[serde(rename = "page")]
    Page,
    #[serde(rename = "terminal")]
    Terminal,
    #[serde(rename = "memory")]
    Memory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextStyle {
    #[serde(rename = "short")]
    Short,
    #[serde(rename = "standard")]
    Standard,
    #[serde(rename = "detailed")]
    Detailed,
}

// ============================================================================
// API Request/Response Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRequest {
    pub topic_hint: Option<String>,
    pub intent: Option<String>,
    pub budget_tokens: u64,
    pub scopes: Vec<String>,
    pub thread_key: Option<String>,
    pub last_capsule_id: Option<CapsuleId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoRequest {
    pub capsule_id: CapsuleId,
    pub thread_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoResponse {
    pub success: bool,
    pub message: Option<String>,
}

// ============================================================================
// Hierarchical Memory Organization
// ============================================================================

// New ID types for hierarchy
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkspaceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AreaId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TopicId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VersionId(pub String);

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for AreaId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for TopicId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for RelationId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for VersionId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Enums for hierarchical system

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "archived")]
    Archived,
    #[serde(rename = "planned")]
    Planned,
}

impl ProjectStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectStatus::Active => "active",
            ProjectStatus::Archived => "archived",
            ProjectStatus::Planned => "planned",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryStatus {
    #[serde(rename = "fleeting")]
    Fleeting,      // Just captured, not refined
    #[serde(rename = "permanent")]
    Permanent,     // Reviewed and confirmed
    #[serde(rename = "archived")]
    Archived,      // Old but kept for reference
    #[serde(rename = "deprecated")]
    Deprecated,    // Superseded by newer memory
}

impl MemoryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryStatus::Fleeting => "fleeting",
            MemoryStatus::Permanent => "permanent",
            MemoryStatus::Archived => "archived",
            MemoryStatus::Deprecated => "deprecated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportanceLevel {
    #[serde(rename = "critical")]
    Critical,      // Core knowledge, frequently referenced
    #[serde(rename = "high")]
    High,          // Important context
    #[serde(rename = "normal")]
    Normal,        // Standard memory
    #[serde(rename = "low")]
    Low,           // Minor detail, candidate for summarization
}

impl ImportanceLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImportanceLevel::Critical => "critical",
            ImportanceLevel::High => "high",
            ImportanceLevel::Normal => "normal",
            ImportanceLevel::Low => "low",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationType {
    #[serde(rename = "supersedes")]
    Supersedes,        // This replaces that older memory
    #[serde(rename = "implements")]
    Implements,        // This implements that decision
    #[serde(rename = "questions")]
    Questions,         // This challenges that assumption
    #[serde(rename = "relates_to")]
    RelatesTo,         // Generic semantic link
    #[serde(rename = "contradicts")]
    Contradicts,       // This conflicts with that
    #[serde(rename = "exemplifies")]
    Exemplifies,       // This is an example of that concept
}

impl RelationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RelationType::Supersedes => "supersedes",
            RelationType::Implements => "implements",
            RelationType::Questions => "questions",
            RelationType::RelatesTo => "relates_to",
            RelationType::Contradicts => "contradicts",
            RelationType::Exemplifies => "exemplifies",
        }
    }
}

// Hierarchy structs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,  // RFC3339
    pub updated_at: String,  // RFC3339
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub description: Option<String>,
    pub status: ProjectStatus,
    pub created_at: String,  // RFC3339
    pub updated_at: String,  // RFC3339
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Area {
    pub id: AreaId,
    pub project_id: ProjectId,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,  // RFC3339
    pub updated_at: String,  // RFC3339
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub id: TopicId,
    pub area_id: AreaId,
    pub name: String,
    pub description: Option<String>,
    pub is_index_note: bool,  // Zettelkasten hub note
    pub summary: Option<String>,  // Auto-generated summary of child memories
    pub created_at: String,  // RFC3339
    pub updated_at: String,  // RFC3339
}

// Memory relationship and versioning

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRelation {
    pub id: RelationId,
    pub source_id: MemoryId,
    pub target_id: MemoryId,
    pub relation_type: RelationType,
    pub rationale: Option<String>,
    pub created_at: String,  // RFC3339
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryVersion {
    pub id: VersionId,
    pub memory_id: MemoryId,
    pub content: String,
    pub version_number: u32,
    pub change_summary: Option<String>,
    pub created_at: String,  // RFC3339
}

// ============================================================================
// Helper Functions
// ============================================================================

pub fn generate_turn_id() -> TurnId {
    TurnId(format!("turn_{}", ulid::Ulid::new()))
}

pub fn generate_thread_id() -> ThreadId {
    ThreadId(format!("thr_{}", ulid::Ulid::new()))
}

pub fn generate_memory_id() -> MemoryId {
    MemoryId(format!("mem_{}", ulid::Ulid::new()))
}

pub fn generate_capsule_id() -> CapsuleId {
    CapsuleId(format!("cap_{}", ulid::Ulid::new()))
}

pub fn generate_workspace_id() -> WorkspaceId {
    WorkspaceId(format!("ws_{}", ulid::Ulid::new()))
}

pub fn generate_project_id() -> ProjectId {
    ProjectId(format!("proj_{}", ulid::Ulid::new()))
}

pub fn generate_area_id() -> AreaId {
    AreaId(format!("area_{}", ulid::Ulid::new()))
}

pub fn generate_topic_id() -> TopicId {
    TopicId(format!("topic_{}", ulid::Ulid::new()))
}

pub fn generate_relation_id() -> RelationId {
    RelationId(format!("rel_{}", ulid::Ulid::new()))
}

pub fn generate_version_id() -> VersionId {
    VersionId(format!("ver_{}", ulid::Ulid::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_generation() {
        let turn_id = generate_turn_id();
        assert!(turn_id.0.starts_with("turn_"));
        assert_eq!(turn_id.0.len(), 31); // "turn_" + 26 chars

        let thread_id = generate_thread_id();
        assert!(thread_id.0.starts_with("thr_"));

        let memory_id = generate_memory_id();
        assert!(memory_id.0.starts_with("mem_"));

        let capsule_id = generate_capsule_id();
        assert!(capsule_id.0.starts_with("cap_"));
    }

    #[test]
    fn test_turn_serialization() {
        let turn = Turn {
            id: generate_turn_id(),
            thread_id: generate_thread_id(),
            ts_user: "2025-11-02T18:00:00Z".to_string(),
            user_text: "Hello".to_string(),
            ts_ai: Some("2025-11-02T18:00:01Z".to_string()),
            ai_text: Some("Hi there!".to_string()),
            source: TurnSource {
                app: SourceApp::Claude,
                url: None,
                path: None,
            },
        };

        let json = serde_json::to_string(&turn).unwrap();
        let deserialized: Turn = serde_json::from_str(&json).unwrap();
        assert_eq!(turn.user_text, deserialized.user_text);
    }

    #[test]
    fn test_context_capsule_serialization() {
        let capsule = ContextCapsule {
            capsule_id: generate_capsule_id(),
            preamble_text: "Context: test".to_string(),
            messages: vec![Message {
                role: MessageRole::System,
                content: "System message".to_string(),
            }],
            provenance: vec![ProvenanceItem {
                r#type: ProvenanceType::Assistant,
                r#ref: "test".to_string(),
                when: None,
            }],
            delta_of: None,
            ttl_sec: 600,
            token_count: Some(50),
            style: Some(ContextStyle::Short),
        };

        let json = serde_json::to_string(&capsule).unwrap();
        let deserialized: ContextCapsule = serde_json::from_str(&json).unwrap();
        assert_eq!(capsule.preamble_text, deserialized.preamble_text);
    }

    #[test]
    fn test_agentic_memory_serialization() {
        let memory = AgenticMemory {
            id: generate_memory_id(),
            content: "Example content".to_string(),
            context: "project".to_string(),
            keywords: vec!["example".into(), "content".into()],
            tags: vec!["tag:project".into(), "kind:fact".into()],
            category: Some("fact".into()),
            links: vec![AgenticLink {
                target: generate_memory_id(),
                strength: 0.7,
                rationale: Some("topic-match".into()),
            }],
            retrieval_count: 0,
            last_accessed: "2025-01-01T00:00:00Z".into(),
            created_at: "2025-01-01T00:00:00Z".into(),
            evolution_history: vec![AgenticEvolution {
                timestamp: "2025-01-01T00:00:00Z".into(),
                summary: "Seeded memory".into(),
                changes: None,
            }],
        };

        let json = serde_json::to_string(&memory).unwrap();
        let restored: AgenticMemory = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.content, memory.content);
        assert_eq!(restored.links.len(), 1);
    }
}
