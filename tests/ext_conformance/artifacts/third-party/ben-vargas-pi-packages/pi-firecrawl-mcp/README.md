# @benvargas/pi-firecrawl-mcp

Firecrawl MCP tools for [pi](https://github.com/badlogic/pi-mono), the AI coding agent.

Provides web scraping, crawling, and data extraction via [Firecrawl's](https://firecrawl.dev) Model Context Protocol (MCP) over HTTP.

## Features

- **Scrape** (`firecrawl_scrape`) — Scrape a single URL; best for known pages
- **Batch Scrape** (`firecrawl_batch_scrape`) — Scrape many URLs in one job
- **Batch Status** (`firecrawl_check_batch_status`) — Check batch scrape status
- **Map** (`firecrawl_map`) — Discover URLs on a site before scraping
- **Search** (`firecrawl_search`) — Search the web and optionally scrape results
- **Crawl** (`firecrawl_crawl`) — Crawl a site for broad coverage
- **Crawl Status** (`firecrawl_check_crawl_status`) — Check crawl status
- **Extract** (`firecrawl_extract`) — Extract structured data from URLs
- **Configurable limits** — Client-side truncation with configurable max bytes/lines
- **Multiple config sources** — JSON config, environment variables, or CLI flags
- **MCP protocol** — Full JSON-RPC 2.0 and SSE support for streaming responses

## Installation

```bash
pi install npm:@benvargas/pi-firecrawl-mcp
```

Or try without installing:

```bash
pi -e npm:@benvargas/pi-firecrawl-mcp
```

## Setup

No API key is required for basic usage. For higher rate limits, get an API key from [firecrawl.dev](https://firecrawl.dev) and configure it using one of these methods:

### Option 1: Environment Variable

```bash
export FIRECRAWL_API_KEY="your_firecrawl_api_key"
pi
```

### Option 2: JSON Config

Create `~/.pi/agent/extensions/firecrawl-mcp.json`:

```json
{
  "url": "https://mcp.firecrawl.dev/v2/mcp",
  "apiKey": "your_firecrawl_api_key",
  "tools": ["firecrawl_scrape", "firecrawl_map"],
  "timeoutMs": 30000,
  "maxBytes": 51200,
  "maxLines": 2000
}
```

### Option 3: CLI Flags

```bash
pi --firecrawl-mcp-api-key=your_firecrawl_api_key
```

## Usage

Once installed, the AI will automatically use these tools when appropriate:

```
"Scrape https://example.com and get the main content"
"Crawl docs.example.com to find all API reference pages"
"Search for React tutorials and scrape the top 3 results"
"Extract product prices from these URLs: ..."
```

### Tools

#### firecrawl_scrape

Scrape a single URL; best for known pages.

| Parameter | Description | Default |
|-----------|-------------|---------|
| `url` | Target URL (required) | — |
| `formats` | Output formats array | — |
| `onlyMainContent` | Extract only main content | — |
| `waitFor` | Wait time in milliseconds | — |
| `timeout` | Request timeout | — |
| `mobile` | Use mobile viewport | — |
| `includeTags` | Tags to include | — |
| `excludeTags` | Tags to exclude | — |
| `skipTlsVerification` | Skip TLS verification | — |
| `piMaxBytes` | Client-side max bytes override | — |
| `piMaxLines` | Client-side max lines override | — |

#### firecrawl_batch_scrape

Scrape many URLs in one job.

| Parameter | Description | Default |
|-----------|-------------|---------|
| `urls` | URLs to scrape (required) | — |
| `options` | Additional scrape options | — |
| `piMaxBytes` | Client-side max bytes override | — |
| `piMaxLines` | Client-side max lines override | — |

#### firecrawl_check_batch_status

Check batch scrape status/results.

| Parameter | Description | Default |
|-----------|-------------|---------|
| `id` | Batch job id (required) | — |
| `piMaxBytes` | Client-side max bytes override | — |
| `piMaxLines` | Client-side max lines override | — |

#### firecrawl_map

Discover URLs on a site; use before scraping many pages.

| Parameter | Description | Default |
|-----------|-------------|---------|
| `url` | Base URL to map (required) | — |
| `search` | Search query to filter URLs | — |
| `sitemap` | Sitemap handling mode (`include`, `skip`, `only`) | — |
| `includeSubdomains` | Include subdomains | — |
| `limit` | Maximum URLs to return | — |
| `ignoreQueryParameters` | Ignore query parameters | — |
| `piMaxBytes` | Client-side max bytes override | — |
| `piMaxLines` | Client-side max lines override | — |

#### firecrawl_search

Search the web and optionally scrape results.

| Parameter | Description | Default |
|-----------|-------------|---------|
| `query` | Search query (required) | — |
| `limit` | Maximum results | — |
| `lang` | Language code | — |
| `country` | Country code | — |
| `scrapeOptions` | Scrape options for results | — |
| `piMaxBytes` | Client-side max bytes override | — |
| `piMaxLines` | Client-side max lines override | — |

#### firecrawl_crawl

Crawl a site for broad coverage.

| Parameter | Description | Default |
|-----------|-------------|---------|
| `url` | Base URL to crawl (required) | — |
| `maxDepth` | Maximum crawl depth | — |
| `limit` | Maximum pages to crawl | — |
| `allowExternalLinks` | Allow external links | — |
| `deduplicateSimilarURLs` | Deduplicate similar URLs | — |
| `piMaxBytes` | Client-side max bytes override | — |
| `piMaxLines` | Client-side max lines override | — |

#### firecrawl_check_crawl_status

Check crawl status/results.

| Parameter | Description | Default |
|-----------|-------------|---------|
| `id` | Crawl job id (required) | — |
| `piMaxBytes` | Client-side max bytes override | — |
| `piMaxLines` | Client-side max lines override | — |

#### firecrawl_extract

Extract structured data from URLs.

| Parameter | Description | Default |
|-----------|-------------|---------|
| `urls` | URLs to extract (required) | — |
| `prompt` | Extraction prompt | — |
| `systemPrompt` | System prompt for extraction | — |
| `schema` | JSON schema for structured output | — |
| `allowExternalLinks` | Allow external links | — |
| `enableWebSearch` | Enable web search for context | — |
| `includeSubdomains` | Include subdomains | — |
| `piMaxBytes` | Client-side max bytes override | — |
| `piMaxLines` | Client-side max lines override | — |

## Configuration

### Config File Locations

Config files are loaded in order (first match wins):

1. Path from `--firecrawl-mcp-config` flag
2. Path from `FIRECRAWL_MCP_CONFIG` environment variable
3. `./.pi/extensions/firecrawl-mcp.json` (project-level)
4. `~/.pi/agent/extensions/firecrawl-mcp.json` (global)

If none exist, the extension writes a default config to the global path with `apiKey` set to `null`.

### Config File Format

```json
{
  "url": "https://mcp.firecrawl.dev/v2/mcp",
  "apiKey": null,
  "tools": ["firecrawl_scrape", "firecrawl_map"],
  "headers": null,
  "timeoutMs": 30000,
  "protocolVersion": "2025-06-18",
  "maxBytes": 51200,
  "maxLines": 2000
}
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `FIRECRAWL_MCP_URL` | MCP endpoint URL | `https://mcp.firecrawl.dev/v2/mcp` |
| `FIRECRAWL_API_KEY` | Firecrawl API key (Bearer token) | — |
| `FIRECRAWL_MCP_TIMEOUT_MS` | Request timeout (ms) | `30000` |
| `FIRECRAWL_MCP_PROTOCOL_VERSION` | MCP protocol version | `2025-06-18` |
| `FIRECRAWL_MCP_TOOLS` | Comma-separated tool list | — |
| `FIRECRAWL_MCP_MAX_BYTES` | Max bytes to keep from output | `51200` |
| `FIRECRAWL_MCP_MAX_LINES` | Max lines to keep from output | `2000` |
| `FIRECRAWL_MCP_CONFIG` | Path to JSON config file | — |

### CLI Flags

| Flag | Description |
|------|-------------|
| `--firecrawl-mcp-url` | Override the Firecrawl MCP endpoint |
| `--firecrawl-mcp-api-key` | Firecrawl API key |
| `--firecrawl-mcp-timeout-ms` | HTTP timeout (milliseconds) |
| `--firecrawl-mcp-protocol` | MCP protocol version |
| `--firecrawl-mcp-config` | Path to JSON config file |
| `--firecrawl-mcp-tools` | Comma-separated tool list |
| `--firecrawl-mcp-max-bytes` | Max bytes to keep from output |
| `--firecrawl-mcp-max-lines` | Max lines to keep from output |

## Output Truncation

Tool output is automatically truncated to prevent context window overflow:

- **Default limits**: 51200 bytes and 2000 lines
- **Client overrides**: Models can request higher limits via `piMaxBytes`/`piMaxLines` (clamped to configured max)
- **Temp files**: When truncated, full output is saved to a temp file with the path included in the response

## Tool Selection Guide

| Task | Recommended Tool | Avoid When |
|------|------------------|------------|
| Single known page | `firecrawl_scrape` | Many URLs (use `batch_scrape`) |
| Many known URLs | `firecrawl_batch_scrape` | Discovery needed (use `map`/`search`/`crawl`) |
| Discover site structure | `firecrawl_map` | You need content (use `scrape`/`batch`) |
| Find relevant pages | `firecrawl_search` | You already know URLs (use `scrape`/`batch`) |
| Broad site coverage | `firecrawl_crawl` | Single pages or size concerns (use `scrape`/`map+batch`) |
| Structured data extraction | `firecrawl_extract` | Full-page text needed (use `scrape`) |

## Requirements

- pi v0.51.0 or later
- Optional: Firecrawl API key from [firecrawl.dev](https://firecrawl.dev) for higher rate limits

## Uninstall

```bash
pi remove npm:@benvargas/pi-firecrawl-mcp
```

## License

MIT
