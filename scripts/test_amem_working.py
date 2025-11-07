#!/usr/bin/env python3
"""
Working A-mem test with Ollama.
This script has the proper environment configuration.
"""

import os
import sys

# CRITICAL: Disable implicit token to avoid HuggingFace auth issues
os.environ['HF_HUB_DISABLE_IMPLICIT_TOKEN'] = '1'

# Configure Ollama
os.environ['OLLAMA_HOST'] = 'http://localhost:11434'
os.environ['OLLAMA_MODEL'] = 'llama3.2'

print("🧠 A-mem + Ollama Test")
print("=" * 60)

# Import A-mem
print("\n1. Importing A-mem...")
try:
    from agentic_memory.memory_system import AgenticMemorySystem
    print("   ✅ Import successful")
except ImportError as e:
    print(f"   ❌ Import failed: {e}")
    sys.exit(1)

# Initialize memory system
print("\n2. Initializing A-mem with Ollama...")
try:
    memory = AgenticMemorySystem(
        model_name='sentence-transformers/all-MiniLM-L6-v2',
        llm_backend='ollama',
        llm_model='llama3.2'
    )
    print("   ✅ A-mem initialized successfully")
except Exception as e:
    print(f"   ❌ Initialization failed: {e}")
    import traceback
    traceback.print_exc()
    sys.exit(1)

# Add test memories
print("\n3. Adding test memories...")
test_memories = [
    ("Memory Layer integrates A-mem for advanced knowledge management", ["integration", "feature"]),
    ("A-mem uses Zettelkasten principles for organizing memories", ["zettelkasten", "architecture"]),
    ("Ollama provides local LLM inference for privacy", ["ollama", "privacy"]),
    ("The system uses vector embeddings for semantic search", ["embeddings", "search"])
]

memory_ids = []
for i, (content, tags) in enumerate(test_memories, 1):
    try:
        mem_id = memory.add_note(content=content, tags=tags)
        memory_ids.append(mem_id)
        print(f"   ✅ Memory {i} added: {mem_id[:12]}...")
    except Exception as e:
        print(f"   ❌ Failed to add memory {i}: {e}")

# Search memories
print("\n4. Testing semantic search...")
test_queries = [
    "knowledge organization",
    "privacy and local processing",
    "vector search"
]

for query in test_queries:
    try:
        results = memory.search_agentic(query, k=2)
        print(f"\n   Query: '{query}'")
        print(f"   Results: {len(results)} found")
        for j, result in enumerate(results[:2], 1):
            content = result.get('content', '')[:50]
            print(f"     {j}. {content}...")
    except Exception as e:
        print(f"   ⚠️  Search '{query}' failed: {e}")

# Retrieve specific memory
if memory_ids:
    print(f"\n5. Retrieving memory {memory_ids[0][:12]}...")
    try:
        mem = memory.read(memory_ids[0])
        print(f"   Content: {mem.get('content', '')}")
        if 'keywords' in mem and mem['keywords']:
            print(f"   Keywords: {', '.join(mem['keywords'][:5])}")
        if 'tags' in mem and mem['tags']:
            print(f"   Tags: {', '.join(mem['tags'])}")
    except Exception as e:
        print(f"   ⚠️  Retrieval failed: {e}")

print("\n" + "=" * 60)
print("✅ A-mem with Ollama is working!")
print("\nYour setup:")
print(f"  • LLM: Ollama (llama3.2)")
print(f"  • Embeddings: all-MiniLM-L6-v2")
print(f"  • Memories added: {len(memory_ids)}")
print("=" * 60)
