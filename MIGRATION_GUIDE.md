# Memory Layer - Hierarchical Migration Guide

## Overview

This guide explains how to migrate your existing flat topic-based memories to the new hierarchical structure with 4 levels:

**Workspace → Project → Area → Topic → Memory**

The migration tool automatically organizes your memories into this structure using either:
- **LLM-powered suggestions** (Ollama) - intelligent, context-aware hierarchy
- **Heuristic-based suggestions** - fast, pattern-matching approach

## Prerequisites

### For Heuristic Migration (Fast)
- No additional setup required
- Uses pattern matching to classify memories

### For LLM Migration (Intelligent)
1. Install Ollama: `brew install ollama`
2. Start Ollama: `ollama serve`
3. Pull a model: `ollama pull llama3.2:3b`
4. Set environment variables:
   ```bash
   export OLLAMA_HOST=http://localhost:11434
   export OLLAMA_MODEL=llama3.2:3b
   ```

## Running the Migration

### Option 1: Heuristic Migration (Fast, No LLM)

```bash
cd ~/memory-layer
cargo run --bin migrate
```

This will:
- Analyze all existing topics in your database
- Use pattern matching to suggest hierarchical organization
- Create the hierarchy structure
- Update all memories to point to new hierarchical topics
- Generate index notes for each topic

### Option 2: LLM Migration (Intelligent, Requires Ollama)

```bash
cd ~/memory-layer
cargo run --bin migrate -- --use-llm
```

This will:
- Use Ollama to intelligently suggest hierarchy based on context
- Provide more accurate and context-aware organization
- Takes longer but produces better results

### Option 3: Custom Database Path

```bash
cargo run --bin migrate -- --db-path /path/to/your/memory.db
```

### Option 4: LLM + Custom Path

```bash
cargo run --bin migrate -- --use-llm --db-path /path/to/your/memory.db
```

## What the Migration Does

### 1. Analysis Phase
- Scans all distinct topics in the memories table
- Counts how many memories belong to each topic

### 2. Hierarchy Suggestion Phase
For each topic, the system suggests:
- **Workspace**: High-level category (e.g., "Work", "Personal", "Learning")
- **Project**: Specific initiative (e.g., "Memory Layer", "WaterBuddy App")
- **Area**: Domain or component (e.g., "Database", "iOS Development")
- **Topic**: Specific concept (e.g., "Schema Design", "SwiftUI Views")

### 3. Creation Phase
- Creates workspaces (if they don't exist)
- Creates projects within workspaces
- Creates areas within projects
- Creates topics within areas

### 4. Update Phase
- Updates all memories with the old flat topic string
- Points them to the new hierarchical topic_id
- Preserves the original topic string for reference

### 5. Index Note Generation
- Creates Zettelkasten-style index notes for each topic
- Summarizes the contents and memory count
- Serves as a hub for navigating related memories

## Example Migration Output

```
Memory Layer - Hierarchy Migration Tool v0.1.0
Database: /Users/ashvinarora/Library/Application Support/MemoryLayer/memory.db
Using heuristic-based hierarchy suggestions (faster but less accurate)
Opening database...
Found 127 total memories in database
Starting migration...
─────────────────────────────────────────────────
Found 23 distinct topics to migrate
Migrating topic 'Work: Memory Layer Database Schema' (15 memories)
  → Workspace: 'Work', Project: 'Memory Layer', Area: 'Database', Topic: 'Database Schema'
  ✓ Migrated 15 memories to new hierarchy
Migrating topic 'Personal: WaterBuddy Hydration Model' (8 memories)
  → Workspace: 'Personal', Project: 'WaterBuddy', Area: 'iOS Development', Topic: 'Hydration Model'
  ✓ Migrated 8 memories to new hierarchy
...
Generating index notes for all topics...
─────────────────────────────────────────────────
Migration complete!

Statistics:
  Total topics:        23
  Total memories:      127
  Workspaces created:  3
  Projects created:    8
  Areas created:       12
  Topics created:      23
  Memories updated:    127

✓ All memories successfully migrated to hierarchical structure!
```

## Heuristic Rules

The heuristic-based migration uses these patterns:

### Workspace Classification
- **Work**: Contains "work", "project", "code", "development"
- **Learning**: Contains "learn", "study", "research"
- **Personal**: Contains "personal", "health", "home"
- **General**: Default fallback

### Project Detection
- **Memory Layer**: Contains "memory", "amem"
- **WaterBuddy**: Contains "water", "hydration"
- **Social Radar**: Contains "linkedin", "radar"
- **Miscellaneous**: Default fallback

### Area Detection
- **Database**: Contains "database", "schema", "sql"
- **API**: Contains "api", "endpoint"
- **User Interface**: Contains "ui", "interface"
- **iOS Development**: Contains "swift", "ios"
- **General**: Default fallback

## Post-Migration

After migration, your memories will have:
1. **topic_id**: Points to hierarchical topic
2. **topic** (old field): Preserved for reference
3. **importance**: Set to "normal" (can be updated later)
4. **status**: Set to "fleeting" (can be updated to permanent/archived)
5. **version**: Set to 1 (for future versioning)

## Querying the Hierarchy

### Get All Workspaces
```sql
SELECT * FROM workspaces;
```

### Get Projects in a Workspace
```sql
SELECT * FROM projects WHERE workspace_id = 'ws_...';
```

### Get Areas in a Project
```sql
SELECT * FROM areas WHERE project_id = 'proj_...';
```

### Get Topics in an Area
```sql
SELECT * FROM topics WHERE area_id = 'area_...';
```

### Get Memories in a Topic
```sql
SELECT * FROM memories WHERE topic_id = 'topic_...';
```

### Full Hierarchy View
```sql
SELECT
  w.name as workspace,
  p.name as project,
  a.name as area,
  t.name as topic,
  m.text as memory
FROM memories m
JOIN topics t ON m.topic_id = t.id
JOIN areas a ON t.area_id = a.id
JOIN projects p ON a.project_id = p.id
JOIN workspaces w ON p.workspace_id = w.id
ORDER BY w.name, p.name, a.name, t.name;
```

## Troubleshooting

### Migration Fails Midway
- Migration is transactional per-topic
- Partially migrated topics are fine
- Re-run the migration - it will skip existing hierarchies

### Want to Re-migrate
1. Back up your database
2. Delete hierarchy tables:
   ```sql
   DELETE FROM memory_relations;
   DELETE FROM memory_versions;
   DELETE FROM topics;
   DELETE FROM areas;
   DELETE FROM projects;
   DELETE FROM workspaces;
   UPDATE memories SET topic_id = NULL;
   ```
3. Re-run migration

### LLM Timeout
- Increase timeout in migration.rs (currently 30 seconds)
- Use smaller/faster model (e.g., `llama3.2:1b`)
- Fall back to heuristic mode

## Next Steps

After migration, you can:
1. Review the suggested hierarchy
2. Manually adjust workspace/project/area/topic names in the database
3. Update memory importance levels
4. Promote fleeting memories to permanent status
5. Create memory relationships (supersedes, implements, etc.)
6. Use the new hierarchy API endpoints (coming in Phase 5)

## Support

For issues or questions:
- Check logs during migration for error details
- Verify database permissions
- Ensure Ollama is running (for LLM mode)
- File an issue with migration output
