# pi-search-agent extension

## Overview

`pi-search-agent` adds local semantic search to pi. It builds a semantic index of your codebase, runs embedding-based retrieval, and uses a configurable LLM to filter and summarize results via a search subagent.

## First-time setup

On first run you will be prompted (via UI) to:
1. Enter your `OPENAI_API_KEY` (used for embeddings).
2. Choose a search model (recommended: **cerebras / glm-4.7**).

Configuration is stored in:
```
~/.pi/extensions/pi-search-agent/.env
```

You can edit the file manually later. Supported keys:
- `OPENAI_API_KEY`
- `SEARCH_PROVIDER` (e.g. `cerebras`, `openai`)
- `SEARCH_MODEL` (e.g. `glm-4.7`, `gpt-4o-mini`)

## Tools

### `search_agent(query, cwd?, queryExtrapolation?, path?, mode?, logSubagent?)`
Search locally and run a subagent to refine results.

- **query**: natural language search query
- **cwd**: directory to search (defaults to current workspace)
- **queryExtrapolation**: additional queries to run and merge
- **path**: optional filter (file/dir/glob/substring)
- **mode**: currently ignored (defaults to `code`)
- **logSubagent**: writes JSON log to disk when true

Example:
```
search_agent(query: "How do we authenticate API requests?", path: "src", logSubagent: true)
```

### `local_embedding_search(query, cwd?, path?, mode?)`
Run embedding search only (one result per file).

Example:
```
local_embedding_search(query: "retry logic", path: "packages/api")
```

## How it works

1. **File discovery**: streams `find` results and yields to the event loop to avoid blocking the UI.
2. **Chunking**: files are split into overlapping chunks for stable embeddings.
3. **Embeddings**: generated with OpenAI (`text-embedding-3-small`) and cached on disk.
4. **Index storage**: index metadata + chunks are persisted per-cwd.
5. **Search pipeline**:
   - embedding matches → merged per file
   - subagent refines results and provides a concise answer
6. **Summaries / filtering**: uses the configured `SEARCH_PROVIDER` + `SEARCH_MODEL`.

## Data locations

- **Index**: `~/.pi/agent/cache/semantic-search/<hash>/`
- **Embedding cache**: `~/.pi/agent/cache/semantic-search/embeddings/`
- **Subagent logs** (when enabled): `~/.pi/agent/cache/semantic-search/subagent-logs/`

## Legacy tools / UI

Legacy tools are disabled by default. Enable them with:
```
PI_SEMANTIC_LEGACY=1
```
This re-enables:
- `semantic_index`, `semantic_search`
- `/semantic` interactive UI

## Notes

- `mode` is accepted but currently ignored; `code` includes markdown.
- Indexing is per-cwd. If no index exists, it is created automatically.
- The recommended search model is **cerebras / glm-4.7**.
