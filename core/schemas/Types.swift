import Foundation

// ============================================================================
// ID Types
// ============================================================================

typealias TurnId = String // "turn_{ULID}"
typealias ThreadId = String // "thr_{ULID}"
typealias MemoryId = String // "mem_{ULID}"
typealias CapsuleId = String // "cap_{ULID}"

// ============================================================================
// Turn Schema
// ============================================================================

struct Turn: Codable {
    let id: TurnId
    let threadId: ThreadId
    let tsUser: String // RFC3339
    let userText: String
    let tsAi: String? // RFC3339
    let aiText: String?
    let source: TurnSource

    enum CodingKeys: String, CodingKey {
        case id
        case threadId = "thread_id"
        case tsUser = "ts_user"
        case userText = "user_text"
        case tsAi = "ts_ai"
        case aiText = "ai_text"
        case source
    }
}

struct TurnSource: Codable {
    let app: SourceApp
    let url: String?
    let path: String?
}

enum SourceApp: String, Codable {
    case claude = "Claude"
    case chatGPT = "ChatGPT"
    case vsCode = "VSCode"
    case mail = "Mail"
    case notes = "Notes"
    case terminal = "Terminal"
    case other = "Other"
}

// ============================================================================
// Memory Schema
// ============================================================================

struct Memory: Codable {
    let id: MemoryId
    let kind: MemoryKind
    let topic: String
    let text: String
    let snippet: Snippet?
    let entities: [String]
    let provenance: [TurnId]
    let createdAt: String // RFC3339
    let ttl: Int?

    enum CodingKeys: String, CodingKey {
        case id, kind, topic, text, snippet, entities, provenance
        case createdAt = "created_at"
        case ttl
    }
}

enum MemoryKind: String, Codable {
    case decision
    case fact
    case snippet
    case task
}

struct Snippet: Codable {
    let title: String
    let text: String
    let loc: String? // e.g., "L18-L44"
    let language: String?
}

// ============================================================================
// Context Capsule Schema
// ============================================================================

struct ContextCapsule: Codable {
    let capsuleId: CapsuleId
    let preambleText: String
    let messages: [Message]
    let provenance: [ProvenanceItem]
    let deltaOf: CapsuleId?
    let ttlSec: Int
    let tokenCount: Int?
    let style: ContextStyle?

    enum CodingKeys: String, CodingKey {
        case capsuleId = "capsule_id"
        case preambleText = "preamble_text"
        case messages, provenance
        case deltaOf = "delta_of"
        case ttlSec = "ttl_sec"
        case tokenCount = "token_count"
        case style
    }
}

struct Message: Codable {
    let role: MessageRole
    let content: String
}

enum MessageRole: String, Codable {
    case system
    case user
    case assistant
}

struct ProvenanceItem: Codable {
    let type: ProvenanceType
    let ref: String
    let when: String? // RFC3339
}

enum ProvenanceType: String, Codable {
    case assistant
    case file
    case page
    case terminal
    case memory
}

enum ContextStyle: String, Codable {
    case short
    case standard
    case detailed
}

// ============================================================================
// API Request/Response Types
// ============================================================================

struct ContextRequest: Codable {
    let topicHint: String?
    let intent: String?
    let budgetTokens: Int
    let scopes: [String]
    let threadKey: String?
    let lastCapsuleId: CapsuleId?

    enum CodingKeys: String, CodingKey {
        case topicHint = "topic_hint"
        case intent
        case budgetTokens = "budget_tokens"
        case scopes
        case threadKey = "thread_key"
        case lastCapsuleId = "last_capsule_id"
    }
}

struct UndoRequest: Codable {
    let capsuleId: CapsuleId
    let threadKey: String

    enum CodingKeys: String, CodingKey {
        case capsuleId = "capsule_id"
        case threadKey = "thread_key"
    }
}

struct UndoResponse: Codable {
    let success: Bool
    let message: String?
}

// ============================================================================
// Helper Functions
// ============================================================================

func generateULID() -> String {
    // Simple ULID-like generation (in production, use a proper ULID library)
    let timestamp = UInt64(Date().timeIntervalSince1970 * 1000)
    let randomPart = UUID().uuidString.replacingOccurrences(of: "-", with: "")
    return "\(timestamp)\(randomPart)".prefix(26).uppercased()
}

func generateTurnId() -> TurnId {
    return "turn_\(generateULID())"
}

func generateThreadId() -> ThreadId {
    return "thr_\(generateULID())"
}

func generateMemoryId() -> MemoryId {
    return "mem_\(generateULID())"
}

func generateCapsuleId() -> CapsuleId {
    return "cap_\(generateULID())"
}

// ============================================================================
// Validation
// ============================================================================

func isValidId(_ id: String, prefix: String) -> Bool {
    let pattern = "^\(prefix)_[0-9A-HJKMNP-TV-Z]{26}$"
    let regex = try? NSRegularExpression(pattern: pattern)
    let range = NSRange(location: 0, length: id.utf16.count)
    return regex?.firstMatch(in: id, range: range) != nil
}

func isValidTurnId(_ id: String) -> Bool {
    return isValidId(id, prefix: "turn")
}

func isValidThreadId(_ id: String) -> Bool {
    return isValidId(id, prefix: "thr")
}

func isValidMemoryId(_ id: String) -> Bool {
    return isValidId(id, prefix: "mem")
}

func isValidCapsuleId(_ id: String) -> Bool {
    return isValidId(id, prefix: "cap")
}

// ============================================================================
// Extensions
// ============================================================================

extension Turn {
    var timestamp: Date? {
        return ISO8601DateFormatter().date(from: tsUser)
    }
}

extension Memory {
    var createdDate: Date? {
        return ISO8601DateFormatter().date(from: createdAt)
    }

    var hasExpiry: Bool {
        return ttl != nil
    }
}

extension ContextCapsule {
    var expiresAt: Date {
        return Date().addingTimeInterval(TimeInterval(ttlSec))
    }

    var isExpired: Bool {
        return Date() > expiresAt
    }

    var isDelta: Bool {
        return deltaOf != nil
    }
}
