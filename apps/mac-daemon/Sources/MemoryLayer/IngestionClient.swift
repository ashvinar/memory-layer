import Foundation

struct SourcePayload: Codable {
    let app: String
    let url: String?
    let path: String?
}

struct TurnPayload: Codable {
    let id: String
    let threadId: String
    let tsUser: String
    let userText: String
    let tsAi: String?
    let aiText: String?
    let source: SourcePayload

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

final class IngestionClient {
    private let endpoint: URL
    private let session: URLSession
    private var lastFailure: Date?
    private let noticeThrottle: TimeInterval = 60

    init(endpoint: URL = URL(string: "http://127.0.0.1:21953/ingest/turn")!) {
        self.endpoint = endpoint
        self.session = URLSession(configuration: .default)
    }

    func ingestUserTurn(
        text: String,
        bundleId: String,
        appName: String,
        threadId: String = "thr_default"
    ) {
        let turn = TurnPayload(
            id: generateTurnId(),
            threadId: threadId,
            tsUser: isoFormatter.string(from: Date()),
            userText: text,
            tsAi: nil,
            aiText: nil,
            source: SourcePayload(
                app: mapBundleToSourceApp(bundleID: bundleId),
                url: nil,
                path: appName
            )
        )

        ingest(turn: turn)
    }

    func ingest(turn: TurnPayload) {
        Task {
            do {
                var request = URLRequest(url: endpoint)
                request.httpMethod = "POST"
                request.setValue("application/json", forHTTPHeaderField: "Content-Type")
                request.httpBody = try JSONEncoder().encode(turn)

                let (_, response) = try await session.data(for: request)

                guard let http = response as? HTTPURLResponse else { return }
                switch http.statusCode {
                case 200:
                    lastFailure = nil
                case 422:
                    throttleNotice("Ingestion ignored turn (422). The schema may be ahead of the client.")
                default:
                    throttleNotice("Ingestion request failed with status \(http.statusCode).")
                }
            } catch {
                throttleNotice("Ingestion request failed: \(error.localizedDescription)")
            }
        }
    }

    private func throttleNotice(_ message: String) {
        let now = Date()
        if let last = lastFailure, now.timeIntervalSince(last) < noticeThrottle {
            return
        }
        lastFailure = now
        print("Ingestion warning — \(message)")
    }

    private func generateTurnId() -> String {
        "tur_" + UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
    }

    private func mapBundleToSourceApp(bundleID: String) -> String {
        switch bundleID {
        case "com.anthropic.claude-desktop":
            return "Claude"
        case "com.anthropic.claudefordesktop":
            return "Claude"
        case "com.openai.ChatGPT":
            return "ChatGPT"
        case "com.microsoft.VSCode":
            return "VSCode"
        case "com.google.Chrome":
            return "Chrome"
        case "com.apple.Safari":
            return "Safari"
        case "com.apple.Terminal":
            return "Terminal"
        default:
            return "Other"
        }
    }
}

private let isoFormatter: ISO8601DateFormatter = {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return formatter
}()
