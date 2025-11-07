# A-mem Integration Guide

## Overview

The Memory Layer now includes a full implementation of [A-mem](https://github.com/agiresearch/A-mem), an advanced agentic memory management system that provides:

- **Memory Evolution**: Memories automatically refine and update when similar memories are added
- **Semantic Linking**: Automatic creation of connections between related memories
- **LLM-powered Enrichment**: Use GPT-4, Claude, or local models to enrich memories
- **Reflection & Planning**: Generate insights by synthesizing multiple memories
- **Zettelkasten Organization**: Smart tagging and categorization

## Architecture

```
┌─────────────────────────────────────────────┐
│              A-mem System                    │
├─────────────────────────────────────────────┤
│  • Memory Evolution Engine                   │
│  • Vector Store (Embeddings)                 │
│  • LLM Provider (OpenAI/Claude/Ollama)       │
│  • Semantic Link Generator                   │
│  • Reflection & Planning Module              │
└─────────────────────────────────────────────┘
        │                    │
        ▼                    ▼
┌──────────────┐    ┌──────────────┐
│   SQLite     │    │  Vector DB   │
│   Storage    │    │  (In-Memory) │
└──────────────┘    └──────────────┘
```

## Quick Start

### 1. Set Up LLM Provider

Choose one of the following providers for memory enrichment:

#### OpenAI (Recommended)
```bash
export OPENAI_API_KEY="your-api-key"
```

#### Anthropic Claude
```bash
export ANTHROPIC_API_KEY="your-api-key"
```

#### Local Ollama
```bash
# Install Ollama first: https://ollama.ai
ollama pull llama3.2
export OLLAMA_HOST="http://localhost:11434"
export OLLAMA_MODEL="llama3.2"  # Optional, defaults to llama3.2
```

### 2. Start the A-mem Service

```bash
cd ~/memory-layer

# Option 1: Use the convenience script
./scripts/run-amem.sh

# Option 2: Build and run manually
cd core/indexing
cargo build --release --bin amem-service
./target/release/amem-service
```

The service will start on port `21956`.

## API Endpoints

### Add Memory with Evolution

```bash
curl -X POST http://localhost:21956/amem/add \
  -H "Content-Type: application/json" \
  -d '{
    "content": "Decided to use A-mem for organizing memories in the Memory Layer project",
    "context": "Architecture Decision",
    "tags": ["decision", "architecture"],
    "category": "decision"
  }'
```

**Features:**
- Automatically enriches memory with keywords and enhanced context
- Triggers evolution in similar existing memories
- Creates semantic links to related memories
- Returns a unique memory ID

### Semantic Search

```bash
curl "http://localhost:21956/amem/search?q=memory+organization&k=5"
```

**Features:**
- Uses vector embeddings for semantic similarity
- Returns top-k most relevant memories
- Includes all metadata and links

### Reflection & Planning

```bash
curl -X POST http://localhost:21956/amem/reflect \
  -H "Content-Type: application/json" \
  -d '{
    "query": "What are the key architectural decisions for the Memory Layer?",
    "k": 10
  }'
```

**Features:**
- Synthesizes multiple memories to answer queries
- Identifies patterns and connections
- Provides actionable insights
- Highlights knowledge gaps

### Get Memory with Links

```bash
curl http://localhost:21956/amem/memory/mem_01HQXY...
```

**Response includes:**
- Full memory content and metadata
- Evolution history showing how the memory changed over time
- Links to related memories with strength scores
- Retrieval count and access timestamps

### Knowledge Graph

```bash
curl "http://localhost:21956/amem/graph?limit=100"
```

**Returns:**
- Nodes: All memories with their metadata
- Edges: Semantic links between memories
- Compatible with A-mem visualization tools

## Memory Evolution Example

When you add a new memory, the system:

1. **Searches** for similar existing memories
2. **Evolves** highly similar memories (>75% similarity) by:
   - Adding new keywords from the new memory
   - Merging tags
   - Updating categories
   - Recording the evolution in history
3. **Links** related memories (>65% similarity) with:
   - Strength scores (0.0-1.0)
   - Rationales explaining the connection
4. **Enriches** the new memory using LLM to extract:
   - Keywords for searchability
   - Tags for organization
   - Category classification
   - Enhanced context description

## Configuration

### Thresholds

In `core/indexing/src/amem.rs`:

```rust
evolution_threshold: 0.75,  // Similarity needed to trigger evolution
link_threshold: 0.65,       // Similarity needed to create links
```

### Memory Categories

The system uses these categories:
- `task` - Action items and todos
- `decision` - Architectural and design decisions
- `fact` - Factual information
- `code` - Code snippets and technical details
- `conversation` - Dialog and discussions
- `document` - Documentation and notes
- `reference` - External references and links

## Advanced Usage

### Batch Import Existing Memories

```python
import requests
import json

memories = [
    {
        "content": "Implemented vector search using embeddings",
        "context": "Development",
        "tags": ["implementation", "search"],
        "category": "task"
    },
    # ... more memories
]

for memory in memories:
    response = requests.post(
        "http://localhost:21956/amem/add",
        json=memory
    )
    print(f"Added: {response.json()['memory_id']}")
```

### Monitor Memory Evolution

```bash
# Get a memory and check its evolution history
curl http://localhost:21956/amem/memory/mem_01HQXY... | jq '.evolution_history'
```

### Build Memory Chains

The system automatically creates chains of related memories through links. You can traverse these chains:

```python
def get_memory_chain(memory_id, depth=3):
    """Recursively fetch linked memories"""
    memory = requests.get(f"http://localhost:21956/amem/memory/{memory_id}").json()

    if depth > 0:
        for link in memory.get('links', []):
            linked = get_memory_chain(link['target'], depth-1)
            # Process linked memory

    return memory
```

## Performance Considerations

### Vector Store

Currently using in-memory vector store. For production:
1. Consider integrating ChromaDB or Qdrant
2. Implement persistent vector storage
3. Add vector index optimization

### LLM Optimization

To reduce LLM costs:
1. Cache enrichment results
2. Batch similar memories for processing
3. Use smaller models for simple tasks
4. Implement fallback to keyword extraction

### Database Indexing

The system creates indexes on:
- `last_accessed` for recency queries
- `category` for filtered searches
- FTS5 for full-text search

## Troubleshooting

### No Enrichment Happening

Check if LLM provider is configured:
```bash
echo $OPENAI_API_KEY
echo $ANTHROPIC_API_KEY
echo $OLLAMA_HOST
```

### Memories Not Linking

Lower the link threshold in the code:
```rust
link_threshold: 0.5,  // More permissive linking
```

### Evolution Not Triggering

Check similarity scores:
```bash
curl "http://localhost:21956/amem/search?q=your+query&k=10" | jq '.memories[].similarity'
```

### High Latency

- Reduce LLM model size (use gpt-4o-mini instead of gpt-4)
- Implement caching for embeddings
- Use local Ollama for faster response

## Integration with Memory Layer

The A-mem system enhances the existing Memory Layer by:

1. **Ingestion Service** → Sends turns to A-mem for enrichment
2. **Composer Service** → Uses A-mem reflection for context generation
3. **macOS App** → Displays memory evolution in UI
4. **Chrome Extension** → Shows related memories as context

## Next Steps

1. **Production Deployment**
   - Replace in-memory vector store with ChromaDB
   - Add Redis caching layer
   - Implement horizontal scaling

2. **Enhanced Features**
   - Memory importance scoring
   - Temporal decay for old memories
   - Multi-agent memory sharing
   - Memory compression for long-term storage

3. **Visualization**
   - Interactive 3D knowledge graph
   - Memory timeline view
   - Evolution history browser
   - Link strength heatmap

## References

- [A-mem Paper](https://arxiv.org/abs/2410.10123)
- [A-mem GitHub](https://github.com/agiresearch/A-mem)
- [Zettelkasten Method](https://zettelkasten.de/)
- [Memory Layer Docs](../README.md)