import Foundation

struct MemoryRecord: Decodable, Identifiable {
    let id: String
    let kind: String
    let topic: String
    let text: String
    let snippet: MemorySnippet?
    let entities: [String]
    let provenance: [String]
    private let createdAtRaw: String?
    let ttl: UInt64?

    var shortPreview: String {
        if let snippet = snippet, !snippet.text.isEmpty {
            return snippet.text
        }
        if text.count > 160 {
            let endIndex = text.index(text.startIndex, offsetBy: 160)
            return "\(text[text.startIndex..<endIndex])…"
        }
        return text
    }

    var createdAt: Date? {
        guard let createdAtRaw, !createdAtRaw.isEmpty else { return nil }
        return MemoryRecord.isoFormatter.date(from: createdAtRaw)
    }

    var createdAtDisplay: String {
        guard let createdAt else { return "—" }
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter.string(from: createdAt)
    }

    private enum CodingKeys: String, CodingKey {
        case id
        case kind
        case topic
        case text
        case snippet
        case entities
        case provenance
        case createdAtRaw = "created_at"
        case ttl
    }

    static let isoFormatter: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter
    }()
}

struct MemorySnippet: Decodable {
    let title: String
    let text: String
    let loc: String?
    let language: String?
}

extension MemoryRecord {
    init(
        id: String,
        kind: String,
        topic: String,
        text: String,
        snippet: MemorySnippet? = nil,
        entities: [String] = [],
        provenance: [String] = [],
        createdAt: Date,
        ttl: UInt64? = nil
    ) {
        self.id = id
        self.kind = kind
        self.topic = topic
        self.text = text
        self.snippet = snippet
        self.entities = entities
        self.provenance = provenance
        self.createdAtRaw = MemoryRecord.isoFormatter.string(from: createdAt)
        self.ttl = ttl
    }
}

struct TopicSummaryRecord: Decodable, Identifiable {
    let topic: String
    let memoryCount: Int
    private let lastMemoryRaw: String?

    var lastUpdatedDisplay: String {
        guard let lastMemoryAt else { return "No data" }
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter.string(from: lastMemoryAt)
    }

    var lastMemoryAt: Date? {
        guard let lastMemoryRaw, !lastMemoryRaw.isEmpty else { return nil }
        return MemoryRecord.isoFormatter.date(from: lastMemoryRaw)
    }

    private enum CodingKeys: String, CodingKey {
        case topic
        case memoryCount = "memory_count"
        case lastMemoryRaw = "last_memory_at"
    }

    var id: String { topic }
}

extension TopicSummaryRecord {
    init(topic: String, memoryCount: Int, lastMemoryAt: Date?) {
        self.topic = topic
        self.memoryCount = memoryCount
        if let lastMemoryAt {
            self.lastMemoryRaw = MemoryRecord.isoFormatter.string(from: lastMemoryAt)
        } else {
            self.lastMemoryRaw = nil
        }
    }
}

final class MemoryServiceClient {
    private let baseURL: URL
    private let ingestionURL: URL
    private let indexingURL: URL
    private let session: URLSession
    private let decoder: JSONDecoder

    init(baseURL: String = "http://127.0.0.1:21953", indexingURL: String = "http://127.0.0.1:21954") {
        self.baseURL = URL(string: baseURL)!
        self.ingestionURL = URL(string: baseURL)!  // Ingestion is the same as baseURL
        self.indexingURL = URL(string: indexingURL)!
        self.session = URLSession.shared
        self.decoder = JSONDecoder()
    }

    func fetchRecentMemories(limit: Int = 100) async throws -> [MemoryRecord] {
        var components = URLComponents(url: baseURL.appendingPathComponent("/memories/recent"), resolvingAgainstBaseURL: false)!
        components.queryItems = [
            URLQueryItem(name: "limit", value: "\(limit)")
        ]

        do {
            let (data, response) = try await session.data(from: components.url!)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                if let http = response as? HTTPURLResponse,
                   http.statusCode >= 500 || http.statusCode == 503 || http.statusCode == 404 {
                    throw ProviderError.serviceUnavailable
                }
                throw ProviderError.invalidResponse
            }

            let payload = try decoder.decode(MemoryEnvelope.self, from: data)
            return payload.memories
        } catch let error as DecodingError {
            throw ProviderError.decodingError(error)
        } catch let error as ProviderError {
            throw error
        } catch {
            throw ProviderError.networkError(error)
        }
    }

    func fetchTopicSummaries(limit: Int = 50) async throws -> [TopicSummaryRecord] {
        var components = URLComponents(url: baseURL.appendingPathComponent("/memories/topics"), resolvingAgainstBaseURL: false)!
        components.queryItems = [
            URLQueryItem(name: "limit", value: "\(limit)")
        ]

        do {
            let (data, response) = try await session.data(from: components.url!)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                if let http = response as? HTTPURLResponse,
                   http.statusCode >= 500 || http.statusCode == 503 || http.statusCode == 404 {
                    throw ProviderError.serviceUnavailable
                }
                throw ProviderError.invalidResponse
            }

            let payload = try decoder.decode(TopicEnvelope.self, from: data)
            return payload.topics
        } catch let error as DecodingError {
            throw ProviderError.decodingError(error)
        } catch let error as ProviderError {
            throw error
        } catch {
            throw ProviderError.networkError(error)
        }
    }

    func fetchAgenticGraph(limit: Int = 200) async throws -> AgenticGraph {
        var components = URLComponents(url: indexingURL.appendingPathComponent("/agentic/graph"), resolvingAgainstBaseURL: false)!
        components.queryItems = [
            URLQueryItem(name: "limit", value: "\(limit)")
        ]

        do {
            let (data, response) = try await session.data(from: components.url!)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                if let http = response as? HTTPURLResponse,
                   http.statusCode >= 500 || http.statusCode == 503 || http.statusCode == 404 {
                    throw ProviderError.serviceUnavailable
                }
                throw ProviderError.invalidResponse
            }

            return try decoder.decode(AgenticGraph.self, from: data)
        } catch let error as DecodingError {
            throw ProviderError.decodingError(error)
        } catch let error as ProviderError {
            throw error
        } catch {
            throw ProviderError.networkError(error)
        }
    }

    // MARK: - Hierarchy Navigation

    func fetchWorkspaces() async throws -> [WorkspaceRecord] {
        let url = ingestionURL.appendingPathComponent("/hierarchy/workspaces")

        do {
            let (data, response) = try await session.data(from: url)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                if let http = response as? HTTPURLResponse,
                   http.statusCode >= 500 || http.statusCode == 503 || http.statusCode == 404 {
                    throw ProviderError.serviceUnavailable
                }
                throw ProviderError.invalidResponse
            }

            let payload = try decoder.decode(WorkspacesEnvelope.self, from: data)
            return payload.workspaces
        } catch let error as DecodingError {
            throw ProviderError.decodingError(error)
        } catch let error as ProviderError {
            throw error
        } catch {
            throw ProviderError.networkError(error)
        }
    }

    func fetchProjects(workspaceId: String? = nil) async throws -> [ProjectRecord] {
        var components = URLComponents(url: ingestionURL.appendingPathComponent("/hierarchy/projects"), resolvingAgainstBaseURL: false)!
        if let workspaceId {
            components.queryItems = [URLQueryItem(name: "workspace_id", value: workspaceId)]
        }

        do {
            let (data, response) = try await session.data(from: components.url!)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                if let http = response as? HTTPURLResponse,
                   http.statusCode >= 500 || http.statusCode == 503 || http.statusCode == 404 {
                    throw ProviderError.serviceUnavailable
                }
                throw ProviderError.invalidResponse
            }

            let payload = try decoder.decode(ProjectsEnvelope.self, from: data)
            return payload.projects
        } catch let error as DecodingError {
            throw ProviderError.decodingError(error)
        } catch let error as ProviderError {
            throw error
        } catch {
            throw ProviderError.networkError(error)
        }
    }

    func fetchAreas(projectId: String? = nil) async throws -> [AreaRecord] {
        var components = URLComponents(url: ingestionURL.appendingPathComponent("/hierarchy/areas"), resolvingAgainstBaseURL: false)!
        if let projectId {
            components.queryItems = [URLQueryItem(name: "project_id", value: projectId)]
        }

        do {
            let (data, response) = try await session.data(from: components.url!)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                if let http = response as? HTTPURLResponse,
                   http.statusCode >= 500 || http.statusCode == 503 || http.statusCode == 404 {
                    throw ProviderError.serviceUnavailable
                }
                throw ProviderError.invalidResponse
            }

            let payload = try decoder.decode(AreasEnvelope.self, from: data)
            return payload.areas
        } catch let error as DecodingError {
            throw ProviderError.decodingError(error)
        } catch let error as ProviderError {
            throw error
        } catch {
            throw ProviderError.networkError(error)
        }
    }

    func fetchTopics(areaId: String? = nil) async throws -> [TopicRecord] {
        var components = URLComponents(url: ingestionURL.appendingPathComponent("/hierarchy/topics"), resolvingAgainstBaseURL: false)!
        if let areaId {
            components.queryItems = [URLQueryItem(name: "area_id", value: areaId)]
        }

        do {
            let (data, response) = try await session.data(from: components.url!)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                if let http = response as? HTTPURLResponse,
                   http.statusCode >= 500 || http.statusCode == 503 || http.statusCode == 404 {
                    throw ProviderError.serviceUnavailable
                }
                throw ProviderError.invalidResponse
            }

            let payload = try decoder.decode(TopicsEnvelope.self, from: data)
            return payload.topics
        } catch let error as DecodingError {
            throw ProviderError.decodingError(error)
        } catch let error as ProviderError {
            throw error
        } catch {
            throw ProviderError.networkError(error)
        }
    }
}

private struct MemoryEnvelope: Decodable {
    let memories: [MemoryRecord]
}

private struct TopicEnvelope: Decodable {
    let topics: [TopicSummaryRecord]
}

private struct WorkspacesEnvelope: Decodable {
    let workspaces: [WorkspaceRecord]
}

private struct ProjectsEnvelope: Decodable {
    let projects: [ProjectRecord]
}

private struct AreasEnvelope: Decodable {
    let areas: [AreaRecord]
}

private struct TopicsEnvelope: Decodable {
    let topics: [TopicRecord]
}

struct AgenticGraph: Codable {
    let nodes: [AgenticGraphNode]
    let edges: [AgenticGraphEdge]
}

struct AgenticGraphNode: Codable, Identifiable {
    let idWrapper: MemoryIdentifierWrapper
    let content: String
    let context: String
    let keywords: [String]
    let tags: [String]
    let category: String?
    let retrievalCount: UInt32
    let lastAccessedRaw: String
    let createdAtRaw: String

    var id: String { idWrapper.value }

    var lastAccessed: Date? {
        MemoryRecord.isoFormatter.date(from: lastAccessedRaw)
    }

    var createdAt: Date? {
        MemoryRecord.isoFormatter.date(from: createdAtRaw)
    }

    private enum CodingKeys: String, CodingKey {
        case idWrapper = "id"
        case content
        case context
        case keywords
        case tags
        case category
        case retrievalCount = "retrieval_count"
        case lastAccessedRaw = "last_accessed"
        case createdAtRaw = "created_at"
    }
}

struct AgenticGraphEdge: Codable {
    let sourceWrapper: MemoryIdentifierWrapper
    let targetWrapper: MemoryIdentifierWrapper
    let strength: Double
    let rationale: String?

    var source: String { sourceWrapper.value }
    var target: String { targetWrapper.value }

    private enum CodingKeys: String, CodingKey {
        case sourceWrapper = "source"
        case targetWrapper = "target"
        case strength
        case rationale
    }
}

struct MemoryIdentifierWrapper: Codable {
    let value: String

    init(value: String) {
        self.value = value
    }

    init(from decoder: Decoder) throws {
        // Backend returns plain string IDs, not wrapped objects
        let container = try decoder.singleValueContainer()
        self.value = try container.decode(String.self)
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(value)
    }
}

// MARK: - Hierarchy Models

struct WorkspaceRecord: Decodable, Identifiable {
    let id: String
    let name: String

    init(from decoder: Decoder) throws {
        var container = try decoder.unkeyedContainer()
        id = try container.decode(String.self)
        name = try container.decode(String.self)
    }
}

struct ProjectRecord: Decodable, Identifiable {
    let id: String
    let name: String
    let workspaceName: String?

    var displayName: String {
        if let ws = workspaceName, !ws.isEmpty {
            return "\(ws) → \(name)"
        }
        return name
    }

    init(from decoder: Decoder) throws {
        var container = try decoder.unkeyedContainer()
        id = try container.decode(String.self)
        name = try container.decode(String.self)
        workspaceName = try container.decode(String.self)
    }
}

struct AreaRecord: Decodable, Identifiable {
    let id: String
    let name: String
    let projectName: String?

    var displayName: String {
        if let proj = projectName, !proj.isEmpty {
            return "\(proj) → \(name)"
        }
        return name
    }

    init(from decoder: Decoder) throws {
        var container = try decoder.unkeyedContainer()
        id = try container.decode(String.self)
        name = try container.decode(String.self)
        projectName = try container.decode(String.self)
    }
}

struct TopicRecord: Decodable, Identifiable {
    let id: String
    let name: String
    let areaName: String?
    let memoryCount: Int?

    var displayName: String {
        if let area = areaName, !area.isEmpty {
            return "\(area) → \(name)"
        }
        return name
    }

    init(from decoder: Decoder) throws {
        var container = try decoder.unkeyedContainer()
        id = try container.decode(String.self)
        name = try container.decode(String.self)
        areaName = try container.decode(String.self)
        memoryCount = try? container.decode(Int.self)
    }
}
