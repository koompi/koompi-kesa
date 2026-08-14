# brave-search (extension)

Token-efficient Brave web search as a Pi extension, with optional content extraction/clipping.

## Goals / design

- **Token efficiency**: search results are shown to the user, but filtered out of the LLM context
- **Predictable “search + clip” workflow**: fetch full page content to disk, return only a preview in chat/tool output
- **Robust tool args**: `freshness`/`format` are sanitized defensively to reduce schema-validation failures

## Setup

**Brave Search API keys:**

- Set `BRAVE_API_KEY` in your environment (Brave Search `/web/search`)
  - Get a key: https://api-dashboard.search.brave.com/app/keys
  - This is assumed to be a free-tier key

- Optional: set `BRAVE_API_KEY_PAID` for automatic fallback
  - If `BRAVE_API_KEY` fails due to quota limits (HTTP 429) or auth errors (401/403), the extension will automatically retry with `BRAVE_API_KEY_PAID`
  - Useful if you have both free and paid tier keys and want to maximize free tier usage while having paid as a backup

**Brave AI Grounding (optional):**

- Set `BRAVE_API_KEY_AI_GROUNDING` (Brave AI Grounding `/chat/completions`)
  - Used by the `brave_grounding` tool

- Install dependencies
  - If you installed this extension via `pi install ...` and the package has a root `package.json`, pi will run `npm install` automatically
  - If you installed by copying the folder manually, run:

```bash
cd ~/.pi/agent/extensions/brave-search
npm install
```

## Usage

### Manual search (no LLM turn)

```text
/ws <query> [-n N] [--country US] [--freshness pd|pw|pm|py] [--content]

Note: Avoid wrapping the whole query in quotes. Brave can return empty results for quoted queries.
```

- Results are shown in the chat but filtered out of LLM context
- `--content`:
  - Fetches and extracts readable content (slow)
  - Caps the number of results fetched to **3**
  - Saves full extracted markdown to: `~/.pi/agent/extensions/brave-search/.clips/`
  - Includes a short preview + a `Saved: ...` file path in the output

### Tools for the agent

- `brave_search({ query, count, country, freshness, fetchContent, format })`
  - `format`: `one_line | short | raw_json` (default `short`)
  - `fetchContent=true` performs the same extraction/clipping behavior as `/ws --content`
  - If `query` is a direct URL (including `raw.githubusercontent.com/...`) and `fetchContent=true`, the tool will fetch and clip that URL directly (no search step)

- `brave_grounding({ question, enableResearch, enableCitations, enableEntities, maxAnswerChars })`
  - Returns a grounded answer (Brave AI Grounding) and extracts markdown links as citations

## Content extraction strategy (high-level)

- If the response is already plain text/markdown, keep it as-is
- Otherwise: HTML → Readability → Turndown (GFM) → markdown
- GitHub heuristics:
  - `github.com/.../blob/...` is rewritten to `raw.githubusercontent.com/...`
  - `https://github.com/<owner>/<repo>` (repo root) tries to fetch a raw README from `raw.githubusercontent.com` (HEAD → main/master; common README filenames), then falls back to HTML extraction
  - If an HTML GitHub page contains rendered markdown (`.markdown-body`), extract that instead of Readability

## Optional marker mode (off by default)

If you set:

- `BRAVE_SEARCH_MARKERS=1` (also supports legacy `BRAVE_SEARCH_LITE_MARKERS=1`)

…and the assistant emits `[[ws: some query]]`, the extension will perform a small search and inject results via `sendUserMessage()`.
