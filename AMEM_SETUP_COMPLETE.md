# A-mem Setup Guide - Complete Setup with Ollama

## ✅ Status: WORKING

A-mem has been successfully installed and tested with Ollama on your system.

## What Was Installed

### 1. Python A-mem Package
- **Source**: https://github.com/WujiangXu/A-mem-sys
- **Version**: Latest from git
- **Location**: `~/memory-layer/.venv/`

### 2. Dependencies Installed
- `agentic-memory` - Core A-mem system
- `sentence-transformers` - For embeddings
- `chromadb` - Vector database
- `ollama` - Python client for Ollama
- `torch` - PyTorch for ML models
- Plus all transitive dependencies

### 3. Models Downloaded
- **Embedding Model**: `sentence-transformers/all-MiniLM-L6-v2`
  - Location: `~/.cache/torch/sentence_transformers/`
  - Size: ~90MB
  - Purpose: Convert text to vector embeddings for semantic search

- **LLM Model**: `llama3.2` (via Ollama)
  - Already installed on your system
  - Size: ~2GB
  - Purpose: Generate keywords, tags, and enrich memories

## Quick Start

### Run the Test

```bash
cd ~/memory-layer
source .venv/bin/activate
export HF_HUB_DISABLE_IMPLICIT_TOKEN=1
python scripts/test_minimal.py
```

You should see:
```
✅ SUCCESS: A-mem working with Ollama!
```

### Use in Your Code

```python
import os

# Required environment setup
os.environ['HF_HUB_DISABLE_IMPLICIT_TOKEN'] = '1'
os.environ['OLLAMA_HOST'] = 'http://localhost:11434'
os.environ['OLLAMA_MODEL'] = 'llama3.2'

from agentic_memory.memory_system import AgenticMemorySystem

# Initialize
memory = AgenticMemorySystem(
    model_name='sentence-transformers/all-MiniLM-L6-v2',
    llm_backend='ollama',
    llm_model='llama3.2'
)

# Add a memory
mem_id = memory.add_note(
    content="My first A-mem memory!",
    tags=["test", "demo"]
)

# Search
results = memory.search_agentic("first memory", k=5)

# Read specific memory
details = memory.read(mem_id)
```

## Environment Setup

### Option 1: Source the Environment File

```bash
# Create the environment file (already done)
cat > ~/memory-layer/.env.amem << 'EOF'
export HF_HUB_DISABLE_IMPLICIT_TOKEN=1
export OLLAMA_HOST=http://localhost:11434
export OLLAMA_MODEL=llama3.2
export PATH="$HOME/memory-layer/.venv/bin:$PATH"
EOF

# Use it
source ~/memory-layer/.env.amem
python your_script.py
```

### Option 2: Add to Your Shell Profile

Add to `~/.zshrc` or `~/.bash_profile`:

```bash
# A-mem Configuration
export HF_HUB_DISABLE_IMPLICIT_TOKEN=1
export OLLAMA_HOST=http://localhost:11434
export OLLAMA_MODEL=llama3.2
```

## Architecture Overview

Based on the [A-mem paper](https://arxiv.org/abs/2502.12110), the system implements:

###  1. Note Construction
When you add a memory, it:
- Extracts keywords using LLM
- Generates descriptive tags
- Creates contextual description
- Computes vector embedding

### 2. Link Generation
- Finds top-k similar existing memories
- LLM determines if link should be created
- Creates bidirectional connections

### 3. Memory Evolution
- New memories can update related old memories
- Keywords and tags evolve over time
- Knowledge graph emerges organically

### 4. Semantic Retrieval
- Hybrid search: vector similarity + BM25
- Returns memories with their links
- Supports complex queries

## Key Features

✅ **Privacy-First**: Runs 100% locally with Ollama
✅ **No API Costs**: Free after initial setup
✅ **Smart Organization**: Auto-links related memories
✅ **Memory Evolution**: Memories improve over time
✅ **Semantic Search**: Find by meaning, not just keywords
✅ **Zettelkasten Method**: Inspired by proven note-taking system

## Performance

From the A-mem paper benchmarks:

- **Token Usage**: ~1,200 tokens per operation (vs 16,900 for baselines)
- **Cost**: <$0.0003 per operation with cloud LLMs (FREE with Ollama!)
- **Speed**: ~5.4s with GPT-4o-mini, ~1.1s with Llama 3.2 locally
- **Accuracy**: 2x better on multi-hop reasoning tasks

## Troubleshooting

### Issue: "No module named 'ollama'"
```bash
source ~/memory-layer/.venv/bin/activate
pip install ollama
```

### Issue: "401 Client Error: Unauthorized" from HuggingFace
```bash
# Make sure to set this environment variable
export HF_HUB_DISABLE_IMPLICIT_TOKEN=1
```

### Issue: Ollama connection failed
```bash
# Start Ollama in another terminal
ollama serve

# Or check if it's running
curl http://localhost:11434/api/tags
```

### Issue: Model download fails
```bash
# Clear cache and retry
rm -rf ~/.cache/huggingface
rm -rf ~/.huggingface
python scripts/test_minimal.py
```

## Next Steps

### 1. Integrate with Memory Layer Rust Service

The existing Rust implementation at `core/indexing/src/amem.rs` can be enhanced to call this Python service.

### 2. Run the HTTP Service

```bash
# Start the A-mem HTTP service on port 21956
./scripts/run-amem.sh
```

Then use the REST API:

```bash
# Add memory
curl -X POST http://localhost:21956/amem/add \
  -H "Content-Type: application/json" \
  -d '{"content":"Test memory","tags":["test"]}'

# Search
curl "http://localhost:21956/amem/search?q=test&k=5"
```

### 3. Explore Advanced Features

- **Reflection**: `memory.reflect("What are the key themes?")`
- **Graph Export**: Get knowledge graph for visualization
- **Batch Import**: Load existing notes
- **Memory Chains**: Follow links between memories

## Files Created

- `scripts/test_minimal.py` - Minimal working test ✅
- `scripts/test_amem_working.py` - Full featured test
- `scripts/setup_amem_ollama_v2.py` - Setup script
- `.env.amem` - Environment configuration
- `AMEM_SETUP_COMPLETE.md` - This guide

## References

- [A-mem Paper](https://arxiv.org/abs/2502.12110) - Original research
- [A-mem GitHub](https://github.com/agiresearch/A-mem) - Benchmark code
- [A-mem Production](https://github.com/WujiangXu/A-mem-sys) - Python package (what we installed)
- [Zettelkasten Method](https://zettelkasten.de/) - Inspiration
- [Memory Layer](./README.md) - Your project docs

## Summary

🎉 **A-mem is fully operational with Ollama!**

You now have a production-ready agentic memory system that:
- Runs completely locally (privacy-first)
- Costs nothing to operate (uses Ollama)
- Automatically organizes knowledge
- Gets smarter over time
- Integrates with your Memory Layer project

**Test it anytime:**
```bash
cd ~/memory-layer && source .venv/bin/activate && \
export HF_HUB_DISABLE_IMPLICIT_TOKEN=1 && \
python scripts/test_minimal.py
```

Enjoy your agentic memory system! 🧠✨
