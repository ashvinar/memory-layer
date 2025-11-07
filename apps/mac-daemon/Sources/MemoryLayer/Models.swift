import Foundation

// MARK: - Context Request

struct ContextRequest: Codable {
    let topicHint: String?
    let intent: String?
    let budgetTokens: Int
    let scopes: [String]
    let threadKey: String?
    let lastCapsuleId: String?

    enum CodingKeys: String, CodingKey {
        case topicHint = "topic_hint"
        case intent
        case budgetTokens = "budget_tokens"
        case scopes
        case threadKey = "thread_key"
        case lastCapsuleId = "last_capsule_id"
    }

    init(topicHint: String? = nil,
         intent: String? = nil,
         budgetTokens: Int = 220,
         scopes: [String] = ["assistant"],
         threadKey: String? = nil,
         lastCapsuleId: String? = nil) {
        self.topicHint = topicHint
        self.intent = intent
        self.budgetTokens = budgetTokens
        self.scopes = scopes
        self.threadKey = threadKey
        self.lastCapsuleId = lastCapsuleId
    }
}

// MARK: - Context Capsule

struct ContextCapsule: Codable {
    let capsuleId: String
    let preambleText: String
    let messages: [Message]
    let provenance: [ProvenanceItem]
    let deltaOf: String?
    let ttlSec: Int
    let tokenCount: Int?
    let style: String?

    enum CodingKeys: String, CodingKey {
        case capsuleId = "capsule_id"
        case preambleText = "preamble_text"
        case messages
        case provenance
        case deltaOf = "delta_of"
        case ttlSec = "ttl_sec"
        case tokenCount = "token_count"
        case style
    }
}

struct Message: Codable {
    let role: String
    let content: String
}

struct ProvenanceItem: Codable {
    let type: String
    let ref: String
    let when: String?
}

// MARK: - Undo Request/Response

struct UndoRequest: Codable {
    let capsuleId: String
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

// MARK: - Search

struct SearchResult: Codable {
    let memory: Memory
    let score: Double
}

struct Memory: Codable {
    let id: String
    let kind: String
    let topic: String
    let text: String
    let snippet: Snippet?
    let entities: [String]
    let provenance: [String]
    let createdAt: String
    let ttl: Int?

    enum CodingKeys: String, CodingKey {
        case id, kind, topic, text, snippet, entities, provenance
        case createdAt = "created_at"
        case ttl
    }
}

struct Snippet: Codable {
    let title: String
    let text: String
    let loc: String?
    let language: String?
}

// MARK: - Helper Extensions

extension ContextCapsule {
    var tokenCountDisplay: String {
        if let count = tokenCount {
            return "\(count) tokens"
        }
        return "Unknown tokens"
    }

    var styleDisplay: String {
        return style?.capitalized ?? "Standard"
    }
}
