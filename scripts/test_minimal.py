#!/usr/bin/env python3
"""Minimal test to find where A-mem hangs"""
import os
import sys

# Disable HuggingFace auth
os.environ['HF_HUB_DISABLE_IMPLICIT_TOKEN'] = '1'
os.environ['OLLAMA_HOST'] = 'http://localhost:11434'

print("Step 1: Testing imports...")
try:
    from agentic_memory.memory_system import AgenticMemorySystem
    print("✅ Import OK")
except Exception as e:
    print(f"❌ Import failed: {e}")
    sys.exit(1)

print("\nStep 2: Testing embedding model...")
try:
    from sentence_transformers import SentenceTransformer
    print("Loading model...")
    model = SentenceTransformer('sentence-transformers/all-MiniLM-L6-v2', use_auth_token=False)
    print("✅ Embedding model OK")
except Exception as e:
    print(f"❌ Embedding failed: {e}")
    sys.exit(1)

print("\nStep 3: Testing Ollama connection...")
try:
    import requests
    resp = requests.get('http://localhost:11434/api/tags', timeout=5)
    print(f"✅ Ollama responding: {resp.status_code}")
except Exception as e:
    print(f"❌ Ollama connection failed: {e}")
    sys.exit(1)

print("\nStep 4: Initializing A-mem with Ollama...")
print("(This may take 10-20 seconds on first run)")
sys.stdout.flush()

try:
    memory = AgenticMemorySystem(
        model_name='sentence-transformers/all-MiniLM-L6-v2',
        llm_backend='ollama',
        llm_model='llama3.2'
    )
    print("✅ A-mem initialized with Ollama")

    # Test basic operation
    print("\nStep 5: Adding a test memory...")
    mem_id = memory.add_note("Test memory", tags=["test"])
    print(f"✅ Memory added: {mem_id[:12]}...")

    print("\nStep 6: Searching...")
    results = memory.search_agentic("test", k=1)
    print(f"✅ Search found {len(results)} results")

    print("\n✅ SUCCESS: A-mem working with Ollama!")
    print("Your setup:")
    print("  • LLM: Ollama (llama3.2)")
    print("  • Embeddings: all-MiniLM-L6-v2")
    print("  • Memory enrichment: Enabled")

except Exception as e:
    print(f"❌ Failed at initialization: {e}")
    import traceback
    traceback.print_exc()
    sys.exit(1)
