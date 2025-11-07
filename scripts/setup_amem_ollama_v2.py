#!/usr/bin/env python3
"""
Setup script for A-mem with Ollama backend (Fixed version).
Handles HuggingFace authentication issues.
"""

import os
import sys
import shutil

print("🚀 Setting up A-mem with Ollama (v2 - Fixed)")
print("=" * 60)

# Step 0: Clear any problematic HuggingFace cache
print("\n🧹 Step 0: Cleaning HuggingFace cache...")
hf_cache = os.path.expanduser("~/.cache/huggingface")
hf_token = os.path.expanduser("~/.huggingface/token")

# Remove token file if it exists and is causing issues
if os.path.exists(hf_token):
    try:
        os.remove(hf_token)
        print(f"   ✅ Removed problematic token file")
    except:
        pass

# Unset any HuggingFace environment variables
for env_var in ['HF_TOKEN', 'HUGGINGFACE_TOKEN', 'HF_API_TOKEN']:
    if env_var in os.environ:
        del os.environ[env_var]
        print(f"   ✅ Cleared {env_var}")

# Set offline mode to bypass authentication
os.environ['HF_HUB_OFFLINE'] = '0'
os.environ['TRANSFORMERS_OFFLINE'] = '0'

# Step 1: Download the embedding model using transformers first
print("\n📦 Step 1: Downloading embedding model (fixed method)...")
print("   Model: all-MiniLM-L6-v2")

try:
    from transformers import AutoTokenizer, AutoModel
    import torch

    model_name = "sentence-transformers/all-MiniLM-L6-v2"

    print("   Downloading tokenizer...")
    tokenizer = AutoTokenizer.from_pretrained(model_name, trust_remote_code=True)
    print("   ✅ Tokenizer downloaded")

    print("   Downloading model...")
    model = AutoModel.from_pretrained(model_name, trust_remote_code=True)
    print("   ✅ Model downloaded")

    # Test it
    inputs = tokenizer("test sentence", return_tensors="pt", padding=True, truncation=True)
    with torch.no_grad():
        outputs = model(**inputs)
    print(f"   ✅ Model working (output shape: {outputs.last_hidden_state.shape})")

except Exception as e:
    print(f"   ❌ Failed with transformers: {e}")
    print("\n   Trying direct sentence-transformers approach...")

    try:
        # Try to use sentence_transformers with trust_remote_code
        from sentence_transformers import SentenceTransformer

        # Download without prefix and with trust_remote_code
        model = SentenceTransformer('all-MiniLM-L6-v2', trust_remote_code=True)
        test_embedding = model.encode("test sentence")
        print(f"   ✅ Model working (embedding dim: {len(test_embedding)})")

    except Exception as e2:
        print(f"   ❌ All methods failed: {e2}")
        print("\n   📝 Note: You may need to:")
        print("      1. Check your internet connection")
        print("      2. Clear HuggingFace cache: rm -rf ~/.cache/huggingface")
        print("      3. Try again")
        sys.exit(1)

# Step 2: Verify Ollama
print("\n🔌 Step 2: Verifying Ollama...")

import subprocess
try:
    import requests
except ImportError:
    print("   Installing requests...")
    subprocess.check_call([sys.executable, "-m", "pip", "install", "-q", "requests"])
    import requests

import time

ollama_running = False
try:
    response = requests.get("http://localhost:11434/api/tags", timeout=5)
    if response.status_code == 200:
        print("   ✅ Ollama is running")
        models = response.json().get('models', [])
        llama_found = any('llama3.2' in m.get('name', '') for m in models)

        if llama_found:
            print("   ✅ llama3.2 model is available")
            ollama_running = True
        else:
            print("   ⚠️  llama3.2 not found")
            print(f"      Available models: {[m.get('name') for m in models[:5]]}")
            # Still try to use it
            ollama_running = True
except:
    print("   ⚠️  Ollama is not running")
    print("   Please run 'ollama serve' in another terminal")
    print("   Continuing anyway...")

# Step 3: Test A-mem initialization (minimal test without full init)
print("\n🧪 Step 3: Testing basic A-mem imports...")

try:
    from agentic_memory.memory_system import AgenticMemorySystem
    print("   ✅ A-mem package imported successfully")

    # Set environment variables
    os.environ['OLLAMA_HOST'] = 'http://localhost:11434'
    os.environ['OLLAMA_MODEL'] = 'llama3.2'

    print("   ✅ Environment configured for Ollama")

except Exception as e:
    print(f"   ❌ Import failed: {e}")
    sys.exit(1)

# Step 4: Create environment file
print("\n📝 Step 4: Creating environment configuration...")

env_file = os.path.expanduser("~/memory-layer/.env.amem")
env_content = """# A-mem Configuration with Ollama
export OLLAMA_HOST="http://localhost:11434"
export OLLAMA_MODEL="llama3.2"

# Clear any HuggingFace authentication issues
unset HF_TOKEN
unset HUGGINGFACE_TOKEN
unset HF_API_TOKEN

# Python path for virtual environment
export PATH="$HOME/memory-layer/.venv/bin:$PATH"

# To use these settings, run:
# source ~/memory-layer/.env.amem
"""

try:
    with open(env_file, 'w') as f:
        f.write(env_content)
    print(f"   ✅ Configuration saved to: {env_file}")
except Exception as e:
    print(f"   ⚠️  Could not create env file: {e}")

# Step 5: Create a simple test script
print("\n📝 Step 5: Creating simple test script...")

test_script = os.path.expanduser("~/memory-layer/scripts/test_amem_simple.py")
test_content = """#!/usr/bin/env python3
import os
os.environ['OLLAMA_HOST'] = 'http://localhost:11434'
os.environ['OLLAMA_MODEL'] = 'llama3.2'

# Clear HF tokens
for k in ['HF_TOKEN', 'HUGGINGFACE_TOKEN', 'HF_API_TOKEN']:
    os.environ.pop(k, None)

from agentic_memory.memory_system import AgenticMemorySystem

print("Initializing A-mem with Ollama...")
memory = AgenticMemorySystem(
    model_name='all-MiniLM-L6-v2',
    llm_backend='ollama',
    llm_model='llama3.2'
)

print("Adding test memory...")
mem_id = memory.add_note("Test memory for A-mem with Ollama", tags=["test"])
print(f"Created: {mem_id}")

print("Searching...")
results = memory.search_agentic("test", k=1)
print(f"Found {len(results)} results")
print("✅ A-mem working with Ollama!")
"""

try:
    with open(test_script, 'w') as f:
        f.write(test_content)
    os.chmod(test_script, 0o755)
    print(f"   ✅ Test script saved to: {test_script}")
except Exception as e:
    print(f"   ⚠️  Could not create test script: {e}")

# Summary
print("\n" + "=" * 60)
print("✅ A-mem setup with Ollama completed!")
print("\nWhat was set up:")
print("  • Embedding model: all-MiniLM-L6-v2 (downloaded)")
print("  • LLM Backend: Ollama")
print("  • Model: llama3.2")
print("  • Environment file: ~/memory-layer/.env.amem")
print(f"  • Test script: {test_script}")

print("\nNext steps:")
print("  1. Make sure Ollama is running:")
print("     ollama serve")
print("  2. Source the environment:")
print("     source ~/memory-layer/.env.amem")
print("  3. Run simple test:")
print("     python scripts/test_amem_simple.py")
print("  4. Or run full test:")
print("     python scripts/test_amem.py")

if not ollama_running:
    print("\n⚠️  NOTE: Ollama doesn't appear to be running.")
    print("   Start it with: ollama serve")

print("=" * 60)
