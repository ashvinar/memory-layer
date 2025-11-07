import Cocoa
import ApplicationServices

private let AXUIElementCreatedNotification = "AXUIElementCreatedNotification" as CFString
private let AXValueChangedNotification = "AXValueChangedNotification" as CFString
private let AXWindowCreatedNotification = "AXWindowCreatedNotification" as CFString

final class ClaudeDesktopAdapter {
    private let canonicalBundleId = "com.anthropic.claude-desktop"
    private let bundleCandidates: [String]
    private let ingestion: IngestionClient

    private var observer: AXObserver?
    private var runLoopSource: CFRunLoopSource?
    private var processedMessages: Set<String> = []
    private var isActive = false
    private var lastScan: Date = .distantPast
    private let processingQueue = DispatchQueue(label: "com.memorylayer.claude-adapter", qos: .utility)
    private let scanCooldown: TimeInterval = 0.8
    private let maxTraversalDepth = 6
    private var activeBundleIdentifier: String?

    init(ingestionClient: IngestionClient) {
        self.ingestion = ingestionClient
        self.bundleCandidates = BundleIdentifierResolver.candidates(for: canonicalBundleId)
    }

    func setEnabled(_ enabled: Bool) {
        if enabled {
            start()
        } else {
            stop()
        }
    }

    func start() {
        guard !isActive else { return }
        guard let app = NSWorkspace.shared.runningApplications.first(where: { application in
            if let identifier = application.bundleIdentifier {
                return bundleCandidates.contains(identifier)
            }
            return false
        }) else {
            return
        }

        activeBundleIdentifier = app.bundleIdentifier

        let pid = app.processIdentifier
        var observerRef: AXObserver?
        let error = AXObserverCreate(pid, observerCallback, &observerRef)

        guard error == .success, let observerRef else { return }

        observer = observerRef
        runLoopSource = AXObserverGetRunLoopSource(observerRef)

        if let source = runLoopSource {
            CFRunLoopAddSource(CFRunLoopGetCurrent(), source, .defaultMode)
        }

        let appElement = AXUIElementCreateApplication(pid)

        let pointer = Unmanaged.passUnretained(self).toOpaque()
        AXObserverAddNotification(observerRef, appElement, AXUIElementCreatedNotification, pointer)
        AXObserverAddNotification(observerRef, appElement, AXValueChangedNotification, pointer)
        AXObserverAddNotification(observerRef, appElement, AXWindowCreatedNotification, pointer)

        isActive = true

        // Initial sweep
        scanExistingConversation(appElement: appElement)
    }

    func stop() {
        guard isActive else { return }
        if let source = runLoopSource {
            CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, .defaultMode)
        }
        runLoopSource = nil
        observer = nil
        isActive = false
        processingQueue.async { [weak self] in
            self?.processedMessages.removeAll()
            self?.lastScan = .distantPast
            self?.activeBundleIdentifier = nil
        }
    }

    private func scanExistingConversation(appElement: AXUIElement) {
        processingQueue.async { [weak self] in
            guard let self else { return }
            guard let windows = self.copyAttribute(appElement, attribute: kAXWindowsAttribute as CFString) as? [AXUIElement] else {
                return
            }

            for window in windows {
                self.ingestMessages(in: window, depth: 0)
            }
        }
    }

    func handleNotification(element: AXUIElement) {
        processingQueue.async { [weak self] in
            guard let self else { return }

            let now = Date()
            if now.timeIntervalSince(self.lastScan) < self.scanCooldown {
                return
            }
            self.lastScan = now

            let target = self.rootWindow(for: element) ?? element
            self.ingestMessages(in: target, depth: 0)
        }
    }

    private func ingestMessages(in element: AXUIElement, depth: Int) {
        if depth > maxTraversalDepth {
            return
        }

        if let message = buildMessage(from: element) {
            emit(message: message)
        }

        guard let children = copyAttribute(element, attribute: kAXChildrenAttribute as CFString) as? [AXUIElement] else {
            return
        }

        for child in children {
            ingestMessages(in: child, depth: depth + 1)
        }
    }

    private func buildMessage(from element: AXUIElement) -> ClaudeMessage? {
        guard let role = copyAttribute(element, attribute: kAXRoleAttribute as CFString) as? String else {
            return nil
        }

        guard role == kAXGroupRole || role == kAXStaticTextRole else {
            return nil
        }

        var textContent: String?
        if let value = copyAttribute(element, attribute: kAXValueAttribute as CFString) as? String, !value.isEmpty {
            textContent = value
        } else if let description = copyAttribute(element, attribute: kAXDescriptionAttribute as CFString) as? String, !description.isEmpty {
            textContent = description
        }

        guard let textContent, textContent.count > 10 else {
            return nil
        }

        let id = messageIdentifier(element: element, textContent: textContent)
        guard !processedMessages.contains(id) else { return nil }

        let speaker = resolveSpeaker(for: element)
        return ClaudeMessage(id: id, text: textContent, speaker: speaker)
    }

    private func resolveSpeaker(for element: AXUIElement) -> ClaudeSpeaker {
        if let label = copyAttribute(element, attribute: kAXLabelValueAttribute as CFString) as? String {
            if label.localizedCaseInsensitiveContains("you") {
                return .user
            }
            if label.localizedCaseInsensitiveContains("claude") || label.localizedCaseInsensitiveContains("assistant") {
                return .assistant
            }
        }

        if let description = copyAttribute(element, attribute: kAXDescriptionAttribute as CFString) as? String {
            if description.localizedCaseInsensitiveContains("sent") || description.localizedCaseInsensitiveContains("you") {
                return .user
            }
            if description.localizedCaseInsensitiveContains("assistant") || description.localizedCaseInsensitiveContains("claude") {
                return .assistant
            }
        }

        return .unknown
    }

    private func emit(message: ClaudeMessage) {
        processedMessages.insert(message.id)

        switch message.speaker {
        case .assistant:
            let turn = TurnPayload(
                id: generateTurnId(),
                threadId: "claude_default",
                tsUser: claudeFormatter.string(from: Date()),
                userText: "",
                tsAi: claudeFormatter.string(from: Date()),
                aiText: message.text,
                source: SourcePayload(app: "Claude", url: nil, path: nil)
            )
            ingestion.ingest(turn: turn)
            print("Claude capture · assistant: \(message.text.prefix(80))…")
        default:
            let sourceId = activeBundleIdentifier ?? canonicalBundleId
            ingestion.ingestUserTurn(text: message.text, bundleId: sourceId, appName: "Claude Desktop", threadId: "claude_default")
            print("Claude capture · user: \(message.text.prefix(80))…")
        }
    }

    private func messageIdentifier(element: AXUIElement, textContent: String) -> String {
        if let identifier = copyAttribute(element, attribute: kAXIdentifierAttribute as CFString) as? String, !identifier.isEmpty {
            return "claude_" + identifier
        }
        return "claude_hash_" + String(textContent.hashValue)
    }

    private func copyAttribute(_ element: AXUIElement, attribute: CFString) -> AnyObject? {
        var value: AnyObject?
        let result = AXUIElementCopyAttributeValue(element, attribute, &value)
        guard result == .success else { return nil }
        return value
    }

    private func rootWindow(for element: AXUIElement) -> AXUIElement? {
        var current: AXUIElement? = element
        var depth = 0

        while depth < maxTraversalDepth, let candidate = current {
            if let role = copyAttribute(candidate, attribute: kAXRoleAttribute as CFString) as? String,
               role == (kAXWindowRole as String) {
                return candidate
            }

            guard let parentValue = copyAttribute(candidate, attribute: kAXParentAttribute as CFString) else {
                break
            }

            let parent = parentValue as! AXUIElement
            current = parent
            depth += 1
        }

        return nil
    }
}

private func observerCallback(observer: AXObserver, element: AXUIElement, notification: CFString, refcon: UnsafeMutableRawPointer?) {
    guard let refcon else { return }
    let adapter = Unmanaged<ClaudeDesktopAdapter>.fromOpaque(refcon).takeUnretainedValue()

    adapter.handleNotification(element: element)
}

private struct ClaudeMessage {
    let id: String
    let text: String
    let speaker: ClaudeSpeaker
}

private enum ClaudeSpeaker {
    case user
    case assistant
    case unknown
}

private func generateTurnId() -> String {
    "tur_" + UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
}

private let claudeFormatter: ISO8601DateFormatter = {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return formatter
}()
