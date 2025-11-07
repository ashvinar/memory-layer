import Foundation

enum MemoryConsoleSampleData {
    static let memories: [MemoryRecord] = [
        MemoryRecord(
            id: "mem_sample_claude",
            kind: "fact",
            topic: "Claude Hand-off",
            text: "Summarized Claude chat about onboarding tasks and API tokens.",
            snippet: MemorySnippet(
                title: "Claude Onboarding",
                text: "Remember to rotate the Anthropic API key weekly and pin the SOP in the workspace.",
                loc: nil,
                language: nil
            ),
            entities: ["Claude", "API token"],
            provenance: ["tur_sample_1"],
            createdAt: Date().addingTimeInterval(-3600)
        ),
        MemoryRecord(
            id: "mem_sample_chatgpt",
            kind: "task",
            topic: "ChatGPT Context Capsules",
            text: "Follow up on smaller context capsule for ChatGPT prefill lane.",
            entities: ["ChatGPT", "Context capsule"],
            provenance: ["tur_sample_2"],
            createdAt: Date().addingTimeInterval(-7200)
        )
    ]

    static let topics: [TopicSummaryRecord] = [
        TopicSummaryRecord(
            topic: "Claude Hand-off",
            memoryCount: 4,
            lastMemoryAt: Date().addingTimeInterval(-3600)
        ),
        TopicSummaryRecord(
            topic: "Context Capsules",
            memoryCount: 3,
            lastMemoryAt: Date().addingTimeInterval(-5400)
        )
    ]

    static let graph: AgenticGraph = {
        let claude = AgenticGraphNode(
            idWrapper: MemoryIdentifierWrapper(value: "mem_sample_claude"),
            content: "Summarized Claude chat about onboarding tasks and API tokens.",
            context: "Claude onboarding checklist",
            keywords: ["claude", "onboarding", "api token"],
            tags: ["kind:fact", "topic:claude_hand-off"],
            category: "fact",
            retrievalCount: 8,
            lastAccessedRaw: MemoryRecord.isoFormatter.string(from: Date().addingTimeInterval(-1200)),
            createdAtRaw: MemoryRecord.isoFormatter.string(from: Date().addingTimeInterval(-10_000))
        )

        let chatgpt = AgenticGraphNode(
            idWrapper: MemoryIdentifierWrapper(value: "mem_sample_chatgpt"),
            content: "Follow up on smaller context capsule for ChatGPT prefill lane.",
            context: "Context capsule roadmap",
            keywords: ["chatgpt", "prefill", "capsule"],
            tags: ["kind:task", "topic:context_capsules"],
            category: "task",
            retrievalCount: 5,
            lastAccessedRaw: MemoryRecord.isoFormatter.string(from: Date().addingTimeInterval(-2200)),
            createdAtRaw: MemoryRecord.isoFormatter.string(from: Date().addingTimeInterval(-15_000))
        )

        let memorySearch = AgenticGraphNode(
            idWrapper: MemoryIdentifierWrapper(value: "mem_sample_search"),
            content: "Improve memory search with hybrid scoring for agentic graph.",
            context: "Indexing backlog",
            keywords: ["search", "graph", "bm25"],
            tags: ["kind:task", "topic:memory_search"],
            category: "task",
            retrievalCount: 2,
            lastAccessedRaw: MemoryRecord.isoFormatter.string(from: Date().addingTimeInterval(-3600)),
            createdAtRaw: MemoryRecord.isoFormatter.string(from: Date().addingTimeInterval(-20_000))
        )

        let edges = [
            AgenticGraphEdge(
                sourceWrapper: MemoryIdentifierWrapper(value: "mem_sample_claude"),
                targetWrapper: MemoryIdentifierWrapper(value: "mem_sample_chatgpt"),
                strength: 0.8,
                rationale: "shared onboarding checklist"
            ),
            AgenticGraphEdge(
                sourceWrapper: MemoryIdentifierWrapper(value: "mem_sample_chatgpt"),
                targetWrapper: MemoryIdentifierWrapper(value: "mem_sample_search"),
                strength: 0.6,
                rationale: "context capsule roadmap links to search backlog"
            )
        ]

        return AgenticGraph(nodes: [claude, chatgpt, memorySearch], edges: edges)
    }()
}
