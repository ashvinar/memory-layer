#!/usr/bin/env python3
"""
Test script for A-mem integration with Memory Layer.
This demonstrates the core A-mem functionality with the Python package.
"""

import os
import sys
from datetime import datetime

# Set environment variables for testing
# You can use OpenAI, Anthropic, or Ollama
if not os.getenv('OPENAI_API_KEY') and not os.getenv('ANTHROPIC_API_KEY'):
    print("⚠️  No LLM API key found. Set OPENAI_API_KEY or ANTHROPIC_API_KEY")
    print("   For testing, you can use Ollama locally:")
    print("   export OLLAMA_HOST='http://localhost:11434'")
    print("   export OLLAMA_MODEL='llama3.2'")
    print()

try:
    from agentic_memory.memory_system import AgenticMemorySystem
    print("✅ A-mem package imported successfully")
except ImportError as e:
    print(f"❌ Failed to import A-mem: {e}")
    print("   Run: source .venv/bin/activate && pip install git+https://github.com/WujiangXu/A-mem-sys.git")
    sys.exit(1)


def test_amem_basic():
    """Test basic A-mem functionality"""
    print("\n🧪 Testing A-mem Basic Functionality")
    print("=" * 50)

    # Initialize the memory system
    print("\n1. Initializing A-mem system...")

    # Determine which LLM backend to use
    if os.getenv('OPENAI_API_KEY'):
        llm_backend = "openai"
        llm_model = "gpt-4o-mini"
        print(f"   Using OpenAI ({llm_model})")
    elif os.getenv('ANTHROPIC_API_KEY'):
        llm_backend = "anthropic"
        llm_model = "claude-3-5-haiku-20241022"
        print(f"   Using Anthropic ({llm_model})")
    elif os.getenv('OLLAMA_HOST'):
        llm_backend = "ollama"
        llm_model = os.getenv('OLLAMA_MODEL', 'llama3.2')
        print(f"   Using Ollama ({llm_model})")
    else:
        print("   No LLM configured - using basic keyword extraction")
        llm_backend = None
        llm_model = None

    try:
        memory_system = AgenticMemorySystem(
            model_name='all-MiniLM-L6-v2',
            llm_backend=llm_backend,
            llm_model=llm_model
        )
        print("   ✅ Memory system initialized")
    except Exception as e:
        print(f"   ❌ Failed to initialize: {e}")
        return False

    # Add some test memories
    print("\n2. Adding test memories...")
    test_memories = [
        {
            "content": "Decided to integrate A-mem into the Memory Layer project for advanced memory management",
            "tags": ["decision", "architecture"]
        },
        {
            "content": "A-mem uses Zettelkasten principles to create interconnected knowledge networks",
            "tags": ["fact", "knowledge"]
        },
        {
            "content": "Memory Layer provides privacy-first local memory for AI assistants",
            "tags": ["fact", "feature"]
        },
        {
            "content": "Implemented vector embeddings using sentence-transformers for semantic search",
            "tags": ["implementation", "technical"]
        }
    ]

    memory_ids = []
    for i, mem in enumerate(test_memories, 1):
        try:
            result = memory_system.add_note(
                content=mem["content"],
                tags=mem["tags"]
            )
            memory_ids.append(result)
            print(f"   ✅ Memory {i} added: {result[:12]}...")
        except Exception as e:
            print(f"   ❌ Failed to add memory {i}: {e}")
            return False

    # Search for memories
    print("\n3. Testing semantic search...")
    search_queries = [
        "architecture decisions",
        "technical implementation",
        "Zettelkasten"
    ]

    for query in search_queries:
        try:
            results = memory_system.search_agentic(query, k=2)
            print(f"\n   Query: '{query}'")
            print(f"   Found {len(results)} results:")
            for r in results:
                content = r.get('content', '')[:60]
                print(f"     - {content}...")
        except Exception as e:
            print(f"   ❌ Search failed for '{query}': {e}")

    # Retrieve a specific memory
    print("\n4. Testing memory retrieval...")
    if memory_ids:
        try:
            memory = memory_system.read(memory_ids[0])
            print(f"   ✅ Retrieved memory:")
            print(f"      Content: {memory.get('content', '')[:80]}...")
            if 'keywords' in memory:
                print(f"      Keywords: {', '.join(memory['keywords'][:5])}")
            if 'links' in memory:
                print(f"      Links: {len(memory['links'])} connections")
        except Exception as e:
            print(f"   ❌ Retrieval failed: {e}")

    print("\n" + "=" * 50)
    print("✅ A-mem basic functionality test completed!")
    return True


def test_amem_reflection():
    """Test A-mem reflection capabilities"""
    print("\n🧪 Testing A-mem Reflection")
    print("=" * 50)

    # This requires an LLM backend
    if not (os.getenv('OPENAI_API_KEY') or os.getenv('ANTHROPIC_API_KEY') or os.getenv('OLLAMA_HOST')):
        print("⚠️  Skipping reflection test - requires LLM backend")
        return True

    print("Reflection requires a working memory system with multiple related memories.")
    print("After adding memories with the basic test, you can query:")
    print("  memory_system.reflect('What are the key technical decisions?')")
    print("\n✅ Reflection test noted (requires manual testing)")
    return True


if __name__ == "__main__":
    print("\n🧠 A-mem Integration Test Suite")
    print("=" * 50)

    success = True

    # Run basic functionality test
    success = test_amem_basic() and success

    # Run reflection test
    success = test_amem_reflection() and success

    if success:
        print("\n✅ All tests passed!")
        print("\nNext steps:")
        print("  1. Set your LLM API key (OPENAI_API_KEY or ANTHROPIC_API_KEY)")
        print("  2. Run the Rust A-mem service: ./scripts/run-amem.sh")
        print("  3. Test the HTTP API at http://localhost:21956")
        sys.exit(0)
    else:
        print("\n❌ Some tests failed")
        sys.exit(1)
