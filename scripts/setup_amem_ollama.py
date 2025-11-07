#!/usr/bin/env python3
"""
Setup script for A-mem with Ollama backend.
Downloads the embedding model and configures the environment.
"""

import os
import sys

print("🚀 Setting up A-mem with Ollama")
print("=" * 60)

# Step 1: Download the embedding model
print("\n📦 Step 1: Downloading embedding model...")
print("   Model: sentence-transformers/all-MiniLM-L6-v2")

try:
    from sentence_transformers import SentenceTransformer

    # Download the model (will cache it locally)
    print("   Downloading... (this may take a minute)")
    model = SentenceTransformer('sentence-transformers/all-MiniLM-L6-v2')
    print("   ✅ Embedding model downloaded successfully")

    # Test the model
    test_embedding = model.encode("test sentence")
    print(f"   ✅ Model working (embedding dimension: {len(test_embedding)})")

except Exception as e:
    print(f"   ❌ Failed to download model: {e}")
    print("\n   Trying alternative approach...")

    try:
        # Try downloading without the sentence-transformers prefix
        model = SentenceTransformer('all-MiniLM-L6-v2')
        print("   ✅ Embedding model downloaded successfully (alternative method)")
    except Exception as e2:
        print(f"   ❌ Alternative method also failed: {e2}")
        sys.exit(1)

# Step 2: Verify Ollama is running
print("\n🔌 Step 2: Verifying Ollama...")

import subprocess
import requests
import time

try:
    # Check if Ollama is running
    response = requests.get("http://localhost:11434/api/tags", timeout=5)
    if response.status_code == 200:
        print("   ✅ Ollama is running")
        models = response.json().get('models', [])
        llama_found = any('llama3.2' in m.get('name', '') for m in models)

        if llama_found:
            print("   ✅ llama3.2 model is available")
        else:
            print("   ⚠️  llama3.2 not found, but other models available")
            print(f"      Available: {[m.get('name') for m in models[:3]]}")
    else:
        print("   ⚠️  Ollama responded but with unexpected status")
except requests.exceptions.ConnectionError:
    print("   ⚠️  Ollama is not running")
    print("   Starting Ollama in background...")

    try:
        # Try to start Ollama
        subprocess.Popen(['ollama', 'serve'],
                        stdout=subprocess.DEVNULL,
                        stderr=subprocess.DEVNULL)
        print("   Waiting for Ollama to start...")
        time.sleep(3)

        # Check again
        response = requests.get("http://localhost:11434/api/tags", timeout=5)
        if response.status_code == 200:
            print("   ✅ Ollama started successfully")
        else:
            print("   ❌ Ollama failed to start properly")
    except Exception as e:
        print(f"   ❌ Failed to start Ollama: {e}")
        print("   Please run 'ollama serve' manually in another terminal")

# Step 3: Test A-mem initialization
print("\n🧪 Step 3: Testing A-mem initialization...")

try:
    from agentic_memory.memory_system import AgenticMemorySystem

    # Set Ollama environment variables
    os.environ['OLLAMA_HOST'] = 'http://localhost:11434'
    os.environ['OLLAMA_MODEL'] = 'llama3.2'

    print("   Initializing with Ollama backend...")
    memory_system = AgenticMemorySystem(
        model_name='all-MiniLM-L6-v2',
        llm_backend='ollama',
        llm_model='llama3.2'
    )
    print("   ✅ A-mem initialized successfully with Ollama!")

    # Quick test
    print("\n   Running quick test...")
    test_id = memory_system.add_note(
        content="A-mem test with Ollama - setup successful!",
        tags=["test", "setup"]
    )
    print(f"   ✅ Test memory created: {test_id[:12]}...")

    # Search test
    results = memory_system.search_agentic("test", k=1)
    if results and len(results) > 0:
        print(f"   ✅ Search working: found {len(results)} result(s)")

except Exception as e:
    print(f"   ❌ Initialization failed: {e}")
    import traceback
    traceback.print_exc()
    sys.exit(1)

# Step 4: Create environment file
print("\n📝 Step 4: Creating environment configuration...")

env_file = os.path.expanduser("~/memory-layer/.env.amem")
env_content = """# A-mem Configuration with Ollama
export OLLAMA_HOST="http://localhost:11434"
export OLLAMA_MODEL="llama3.2"

# To use these settings, run:
# source ~/.env.amem
"""

try:
    with open(env_file, 'w') as f:
        f.write(env_content)
    print(f"   ✅ Configuration saved to: {env_file}")
    print(f"   Run: source {env_file}")
except Exception as e:
    print(f"   ⚠️  Could not create env file: {e}")

# Summary
print("\n" + "=" * 60)
print("✅ A-mem setup with Ollama completed successfully!")
print("\nNext steps:")
print("  1. Source the environment: source ~/memory-layer/.env.amem")
print("  2. Run test script: python scripts/test_amem.py")
print("  3. Start Rust service: ./scripts/run-amem.sh")
print("\nConfiguration:")
print(f"  • LLM Backend: Ollama")
print(f"  • Model: llama3.2")
print(f"  • Embedding: all-MiniLM-L6-v2")
print(f"  • Ollama URL: http://localhost:11434")
print("=" * 60)
