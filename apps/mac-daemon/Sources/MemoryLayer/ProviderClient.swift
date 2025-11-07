import Foundation

enum ProviderError: Error {
    case invalidURL
    case networkError(Error)
    case invalidResponse
    case decodingError(Error)
    case serviceUnavailable
}

extension ProviderError: LocalizedError {
    var errorDescription: String? {
        switch self {
        case .invalidURL:
            return "App configuration error: invalid URL."
        case .networkError(let underlying):
            let nsError = underlying as NSError
            if nsError.domain == NSURLErrorDomain {
                switch nsError.code {
                case NSURLErrorCannotConnectToHost,
                     NSURLErrorNetworkConnectionLost,
                     NSURLErrorDNSLookupFailed,
                     NSURLErrorNotConnectedToInternet,
                     NSURLErrorTimedOut:
                    return "Can’t reach the Memory Layer services. Start them with `make run`."
                default:
                    break
                }
            }
            return "Network error: \(underlying.localizedDescription)"
        case .invalidResponse:
            return "Received an unexpected response from the Memory Layer service."
        case .decodingError(let underlying):
            return "Failed to decode response: \(underlying.localizedDescription)"
        case .serviceUnavailable:
            return "Memory Layer service is unavailable. Start it with `make run`."
        }
    }
}

class ProviderClient {
    private let baseURL: URL
    private let session: URLSession

    init(baseURL: String = "http://127.0.0.1:21955") {
        self.baseURL = URL(string: baseURL)!
        self.session = URLSession.shared
    }

    // MARK: - Context API

    func getContext(request: ContextRequest) async throws -> ContextCapsule {
        let url = baseURL.appendingPathComponent("/v1/context")

        var urlRequest = URLRequest(url: url)
        urlRequest.httpMethod = "POST"
        urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")

        let encoder = JSONEncoder()
        urlRequest.httpBody = try encoder.encode(request)

        do {
            let (data, response) = try await session.data(for: urlRequest)

            guard let httpResponse = response as? HTTPURLResponse else {
                throw ProviderError.invalidResponse
            }

            guard httpResponse.statusCode == 200 else {
                if httpResponse.statusCode >= 500 {
                    throw ProviderError.serviceUnavailable
                }
                throw ProviderError.invalidResponse
            }

            let decoder = JSONDecoder()
            let capsule = try decoder.decode(ContextCapsule.self, from: data)
            return capsule

        } catch let error as DecodingError {
            throw ProviderError.decodingError(error)
        } catch let error as ProviderError {
            throw error
        } catch {
            throw ProviderError.networkError(error)
        }
    }

    func undo(capsuleId: String, threadKey: String) async throws -> UndoResponse {
        let url = baseURL.appendingPathComponent("/v1/undo")

        var urlRequest = URLRequest(url: url)
        urlRequest.httpMethod = "POST"
        urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")

        let request = UndoRequest(capsuleId: capsuleId, threadKey: threadKey)
        let encoder = JSONEncoder()
        urlRequest.httpBody = try encoder.encode(request)

        do {
            let (data, response) = try await session.data(for: urlRequest)

            guard let httpResponse = response as? HTTPURLResponse,
                  httpResponse.statusCode == 200 else {
                throw ProviderError.invalidResponse
            }

            let decoder = JSONDecoder()
            let undoResponse = try decoder.decode(UndoResponse.self, from: data)
            return undoResponse

        } catch let error as DecodingError {
            throw ProviderError.decodingError(error)
        } catch let error as ProviderError {
            throw error
        } catch {
            throw ProviderError.networkError(error)
        }
    }

    // MARK: - Health Check

    func checkHealth() async -> Bool {
        let url = baseURL.appendingPathComponent("/health")

        do {
            let (_, response) = try await session.data(from: url)
            guard let httpResponse = response as? HTTPURLResponse else {
                return false
            }
            return httpResponse.statusCode == 200
        } catch {
            return false
        }
    }

    // MARK: - Search (via indexing service)

    func search(query: String, limit: Int = 10) async throws -> [SearchResult] {
        let indexingURL = URL(string: "http://127.0.0.1:21954")!
        var components = URLComponents(url: indexingURL.appendingPathComponent("/search"), resolvingAgainstBaseURL: false)!
        components.queryItems = [
            URLQueryItem(name: "q", value: query),
            URLQueryItem(name: "limit", value: "\(limit)")
        ]

        guard let url = components.url else {
            throw ProviderError.invalidURL
        }

        do {
            let (data, response) = try await session.data(from: url)

            guard let httpResponse = response as? HTTPURLResponse,
                  httpResponse.statusCode == 200 else {
                throw ProviderError.invalidResponse
            }

            let decoder = JSONDecoder()
            let results = try decoder.decode([SearchResult].self, from: data)
            return results

        } catch let error as DecodingError {
            throw ProviderError.decodingError(error)
        } catch let error as ProviderError {
            throw error
        } catch {
            throw ProviderError.networkError(error)
        }
    }
}
