import CoreServices
import Foundation
import AppKit

struct BundleIdentifierResolver {
    private static let aliasMap: [String: [String]] = [
        "com.anthropic.claude-desktop": ["com.anthropic.claudefordesktop"],
        "com.anthropic.claudefordesktop": ["com.anthropic.claude-desktop"]
    ]

    static func candidates(for bundleId: String) -> [String] {
        var results: Set<String> = [bundleId]

        if let aliases = aliasMap[bundleId] {
            results.formUnion(aliases)
        }

        for (key, aliases) in aliasMap where aliases.contains(bundleId) {
            results.insert(key)
        }

        return Array(results)
    }

    static func canonical(for bundleId: String) -> String {
        if aliasMap.keys.contains(bundleId) {
            return bundleId
        }

        for (key, aliases) in aliasMap where aliases.contains(bundleId) {
            return key
        }

        return bundleId
    }

    static func expand(_ bundleIds: Set<String>) -> Set<String> {
        var expanded = bundleIds
        for id in bundleIds {
            expanded.formUnion(candidates(for: id))
        }
        return expanded
    }

    static func contains(_ bundleIds: Set<String>, candidate: String) -> Bool {
        expand(bundleIds).contains(candidate)
    }

    static func locate(bundleId: String, currentPath: String? = nil) -> URL? {
        let fileManager = FileManager.default
        let candidates = candidates(for: bundleId)

        if let currentPath,
           fileManager.fileExists(atPath: currentPath) {
            return URL(fileURLWithPath: currentPath, isDirectory: true)
        }

        if let running = NSWorkspace.shared.runningApplications.first(where: { app in
            if let identifier = app.bundleIdentifier {
                return candidates.contains(identifier)
            }
            return false
        }), let bundleURL = running.bundleURL {
            return bundleURL
        }

        for candidate in candidates {
            if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: candidate) {
                return url
            }
        }

        for candidate in candidates {
            if let unmanaged = LSCopyApplicationURLsForBundleIdentifier(candidate as CFString, nil) {
                let array = unmanaged.takeRetainedValue() as NSArray
                for case let url as NSURL in array {
                    if let path = url.path, fileManager.fileExists(atPath: path) {
                        return url as URL
                    }
                }
            }
        }

        let searchRoots: [URL] = [
            URL(fileURLWithPath: "/Applications", isDirectory: true),
            URL(fileURLWithPath: "/Applications/Utilities", isDirectory: true),
            URL(fileURLWithPath: "/System/Applications", isDirectory: true),
            URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true).appendingPathComponent("Applications", isDirectory: true)
        ]

        for root in searchRoots where fileManager.fileExists(atPath: root.path) {
            for candidate in candidates {
                if let found = findBundle(in: root, matching: candidate) {
                    return found
                }
            }
        }

        return nil
    }

    private static func findBundle(in directory: URL, matching bundleId: String) -> URL? {
        let fileManager = FileManager.default
        guard let enumerator = fileManager.enumerator(
            at: directory,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles, .skipsPackageDescendants],
            errorHandler: nil
        ) else {
            return nil
        }

        for case let candidate as URL in enumerator where candidate.pathExtension == "app" {
            if let bundle = Bundle(url: candidate),
               bundle.bundleIdentifier == bundleId {
                return candidate
            }
        }

        return nil
    }
}
