//! Network-reaching tools: `web_fetch` and `web_search`.
//!
//! Both run unsandboxed with whatever network the host has, so `web_fetch` resolves
//! every hop of a redirect chain and refuses any address that is not publicly
//! routable. Without that, a model that can be talked into fetching
//! `http://169.254.169.254/` reads the cloud metadata service.

use crate::error::{Error, Result};
use crate::http::client::Client;
use crate::model::{ContentBlock, TextContent};
use crate::tools::{Tool, ToolEffects, ToolOutput, ToolUpdate};
use asupersync::http::h1::ParsedUrl;
use asupersync::http::h1::http_client::Scheme;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Marks a `details` payload as a fetched page.
pub const WEB_FETCH_SCHEMA: &str = "kesa.web_fetch.page.v1";
/// Marks a `details` payload as a search result list.
pub const WEB_SEARCH_SCHEMA: &str = "kesa.web_search.results.v1";

/// Opt-in for loopback / private / link-local targets.
pub const ALLOW_PRIVATE_ENV: &str = "WEB_ALLOW_PRIVATE_HOSTS";
/// Search backend selector: `brave` or `tavily`.
pub const SEARCH_PROVIDER_ENV: &str = "WEB_SEARCH_PROVIDER";
/// Search backend credential.
pub const SEARCH_API_KEY_ENV: &str = "WEB_SEARCH_API_KEY";

const MAX_REDIRECTS: usize = 5;
const DEFAULT_FETCH_BYTES: usize = 5 * 1024 * 1024;
const MAX_FETCH_BYTES: usize = 10 * 1024 * 1024;
const MAX_TEXT_CHARS: usize = 100_000;
const MAX_SEARCH_BYTES: usize = 2 * 1024 * 1024;
const DEFAULT_SEARCH_RESULTS: usize = 10;
const MAX_SEARCH_RESULTS: usize = 20;

// ============================================================================
// Address guard
// ============================================================================

fn blocked_v4(ip: Ipv4Addr) -> Option<&'static str> {
    let [a, b, ..] = ip.octets();
    if ip.is_unspecified() {
        return Some("unspecified address");
    }
    if ip.is_loopback() {
        return Some("loopback address");
    }
    if ip.is_private() {
        return Some("RFC1918 private address");
    }
    if ip.is_link_local() {
        return Some("link-local address");
    }
    if ip.is_broadcast() {
        return Some("broadcast address");
    }
    if ip.is_multicast() {
        return Some("multicast address");
    }
    if a == 100 && (64..128).contains(&b) {
        return Some("CGNAT shared address");
    }
    if a == 198 && (b == 18 || b == 19) {
        return Some("benchmarking address");
    }
    if a >= 240 {
        return Some("reserved address");
    }
    None
}

fn blocked_v6(ip: Ipv6Addr) -> Option<&'static str> {
    if let Some(mapped) = ip.to_ipv4_mapped() {
        return blocked_v4(mapped);
    }
    if ip.is_unspecified() {
        return Some("unspecified address");
    }
    if ip.is_loopback() {
        return Some("loopback address");
    }
    if ip.is_multicast() {
        return Some("multicast address");
    }
    let first = ip.segments()[0];
    if first & 0xfe00 == 0xfc00 {
        return Some("unique-local address");
    }
    if first & 0xffc0 == 0xfe80 {
        return Some("link-local address");
    }
    None
}

/// Why an address may not be fetched, or `None` when it is publicly routable.
#[must_use]
pub fn blocked_reason(ip: IpAddr) -> Option<&'static str> {
    match ip {
        IpAddr::V4(v4) => blocked_v4(v4),
        IpAddr::V6(v6) => blocked_v6(v6),
    }
}

fn strip_brackets(host: &str) -> &str {
    host.strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(host)
}

fn refuse(host: &str, ip: IpAddr, reason: &str) -> Error {
    Error::tool(
        "web_fetch",
        format!(
            "Refusing to fetch {host}: it resolves to {ip}, a {reason}. \
             Set {}=1 to allow private and loopback targets.",
            crate::env::name(ALLOW_PRIVATE_ENV)
        ),
    )
}

fn allow_private() -> bool {
    crate::env::var(ALLOW_PRIVATE_ENV).is_some_and(|v| {
        let v = v.trim();
        !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false")
    })
}

/// Reject a target whose host is, or resolves to, a non-public address.
async fn ensure_public_host(host: &str, port: u16) -> Result<()> {
    if allow_private() {
        return Ok(());
    }
    let bare = strip_brackets(host);

    if let Ok(ip) = bare.parse::<IpAddr>() {
        return blocked_reason(ip).map_or(Ok(()), |reason| Err(refuse(host, ip, reason)));
    }

    let addrs = asupersync::net::lookup_all((bare.to_owned(), port))
        .await
        .map_err(|err| Error::tool("web_fetch", format!("Cannot resolve {host}: {err}")))?;
    if addrs.is_empty() {
        return Err(Error::tool(
            "web_fetch",
            format!("Cannot resolve {host}: no addresses"),
        ));
    }
    for addr in addrs {
        if let Some(reason) = blocked_reason(addr.ip()) {
            return Err(refuse(host, addr.ip(), reason));
        }
    }
    Ok(())
}

// ============================================================================
// URL handling
// ============================================================================

fn parse_target(url: &str) -> Result<ParsedUrl> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(Error::validation("`url` must not be empty"));
    }
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        let scheme = trimmed
            .split_once(':')
            .map_or("(none)", |(scheme, _)| scheme)
            .to_owned();
        return Err(Error::tool(
            "web_fetch",
            format!("Refusing to fetch {trimmed}: scheme `{scheme}` is not http or https"),
        ));
    }
    ParsedUrl::parse(trimmed).map_err(|err| Error::tool("web_fetch", format!("Invalid URL: {err}")))
}

fn scheme_str(scheme: Scheme) -> &'static str {
    match scheme {
        Scheme::Http => "http",
        Scheme::Https => "https",
    }
}

fn origin(parsed: &ParsedUrl) -> String {
    let scheme = scheme_str(parsed.scheme);
    let default_port = if matches!(parsed.scheme, Scheme::Https) {
        443
    } else {
        80
    };
    if parsed.port == default_port {
        format!("{scheme}://{}", parsed.host)
    } else {
        format!("{scheme}://{}:{}", parsed.host, parsed.port)
    }
}

/// Resolve a `Location` header against the URL that produced it.
fn resolve_redirect(base: &str, location: &str) -> Result<String> {
    let location = location.trim();
    if location.is_empty() {
        return Err(Error::tool("web_fetch", "Redirect had an empty Location"));
    }
    let lower = location.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Ok(location.to_owned());
    }
    let parsed = ParsedUrl::parse(base)
        .map_err(|err| Error::tool("web_fetch", format!("Invalid URL: {err}")))?;
    if let Some(rest) = location.strip_prefix("//") {
        return Ok(format!("{}://{rest}", scheme_str(parsed.scheme)));
    }
    let origin = origin(&parsed);
    if location.starts_with('/') {
        return Ok(format!("{origin}{location}"));
    }
    let path = parsed
        .path
        .split(['?', '#'])
        .next()
        .unwrap_or(&parsed.path)
        .to_owned();
    let dir = path.rfind('/').map_or("/", |idx| &path[..=idx]);
    Ok(format!("{origin}{dir}{location}"))
}

const fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

// ============================================================================
// HTML to text
// ============================================================================

fn decode_entities(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_owned();
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(idx) = rest.find('&') {
        out.push_str(&rest[..idx]);
        rest = &rest[idx..];
        let Some(end) = rest[1..].find(';').map(|i| i + 1) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity.to_ascii_lowercase().as_str() {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            "mdash" => Some('-'),
            "ndash" => Some('-'),
            "hellip" => Some('\u{2026}'),
            _ => entity
                .strip_prefix('#')
                .and_then(|num| {
                    num.strip_prefix('x')
                        .or_else(|| num.strip_prefix('X'))
                        .map_or_else(
                            || num.parse::<u32>().ok(),
                            |hex| u32::from_str_radix(hex, 16).ok(),
                        )
                })
                .and_then(char::from_u32),
        };
        match decoded {
            Some(ch) => {
                out.push(ch);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

const SKIPPED_ELEMENTS: &[&str] = &["script", "style", "noscript", "svg", "template"];

const BLOCK_ELEMENTS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "br",
    "div",
    "dd",
    "dl",
    "dt",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "td",
    "th",
    "tr",
    "ul",
];

fn tag_name(tag: &str) -> String {
    tag.trim_start_matches('/')
        .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Collapse an HTML document into the text a reader would see, plus its `<title>`.
#[must_use]
pub fn html_to_text(html: &str) -> (Option<String>, String) {
    let mut title = String::new();
    let mut in_title = false;
    let mut out = String::with_capacity(html.len() / 2);
    let mut skip_until: Option<String> = None;
    let mut rest = html;

    while let Some(idx) = rest.find('<') {
        let text = &rest[..idx];
        if skip_until.is_none() {
            if in_title {
                title.push_str(text);
            } else {
                out.push_str(text);
            }
        }
        rest = &rest[idx..];

        if let Some(after) = rest.strip_prefix("<!--") {
            let end = after.find("-->").map_or(after.len(), |i| i + 3);
            rest = &after[end..];
            continue;
        }
        let Some(close) = rest.find('>') else {
            rest = "";
            break;
        };
        let tag = &rest[1..close];
        rest = &rest[close + 1..];
        let name = tag_name(tag);
        let closing = tag.starts_with('/');

        if let Some(open) = skip_until.as_ref() {
            if closing && &name == open {
                skip_until = None;
            }
            continue;
        }
        if !closing && SKIPPED_ELEMENTS.contains(&name.as_str()) && !tag.ends_with('/') {
            skip_until = Some(name);
            continue;
        }
        if name == "title" {
            in_title = !closing;
            continue;
        }
        if BLOCK_ELEMENTS.contains(&name.as_str()) {
            out.push('\n');
        }
    }
    if skip_until.is_none() {
        out.push_str(rest);
    }

    let title = decode_entities(title.trim());
    (
        (!title.is_empty()).then_some(title),
        normalize_text(&decode_entities(&out)),
    )
}

fn normalize_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut blank_run = 0usize;
    for line in raw.lines() {
        let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            blank_run += 1;
            if blank_run > 1 || out.is_empty() {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(&collapsed);
        out.push('\n');
    }
    out.trim_end().to_owned()
}

fn truncate_chars(text: &str, limit: usize) -> (String, bool) {
    if text.chars().count() <= limit {
        return (text.to_owned(), false);
    }
    (text.chars().take(limit).collect(), true)
}

// ============================================================================
// web_fetch
// ============================================================================

#[derive(Debug)]
struct FetchedPage {
    final_url: String,
    status: u16,
    content_type: String,
    chain: Vec<String>,
    body: String,
}

async fn fetch_page(url: &str, max_bytes: usize) -> Result<FetchedPage> {
    let client = Client::new();
    let mut current = url.trim().to_owned();
    let mut chain = vec![current.clone()];

    for _ in 0..=MAX_REDIRECTS {
        let parsed = parse_target(&current)?;
        ensure_public_host(&parsed.host, parsed.port).await?;

        let response = client
            .get(&current)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,text/plain;q=0.9,*/*;q=0.5",
            )
            .header("Accept-Encoding", "identity")
            .send()
            .await?;
        let status = response.status();
        let headers = response.headers().to_vec();

        if is_redirect(status) {
            let Some(location) = header(&headers, "location") else {
                return Err(Error::tool(
                    "web_fetch",
                    format!("{current} returned {status} without a Location header"),
                ));
            };
            let next = resolve_redirect(&current, location)?;
            current = next;
            chain.push(current.clone());
            continue;
        }

        let content_type = header(&headers, "content-type")
            .unwrap_or("")
            .to_ascii_lowercase();
        let body = response.text_limited(max_bytes).await.map_err(|err| {
            if err.to_string().contains("response body too large") {
                Error::tool(
                    "web_fetch",
                    format!(
                        "{current} body exceeds the {max_bytes}-byte cap. Raise `max_bytes` \
                         (maximum {MAX_FETCH_BYTES}) or fetch a more specific URL."
                    ),
                )
            } else {
                err
            }
        })?;

        return Ok(FetchedPage {
            final_url: current,
            status,
            content_type,
            chain,
            body,
        });
    }

    Err(Error::tool(
        "web_fetch",
        format!(
            "Redirect chain exceeded {MAX_REDIRECTS} hops: {}",
            chain.join(" -> ")
        ),
    ))
}

/// Fetch a URL and return it as readable text.
pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &'static str {
        "web_fetch"
    }

    fn label(&self) -> &'static str {
        "Fetch"
    }

    fn description(&self) -> &'static str {
        "Fetch an http(s) URL and return the page as readable text. HTML is stripped to text; \
         redirects are followed and the final URL is reported. Loopback, link-local and private \
         addresses are refused unless the operator opted in. Use `prompt` to say what you are \
         looking for; the text is returned in full either way."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "required": ["url"],
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Absolute http:// or https:// URL to fetch."
                },
                "prompt": {
                    "type": "string",
                    "description": "What to extract from the page. Recorded with the result; it does not filter the text."
                },
                "max_bytes": {
                    "type": "integer",
                    "description": "Cap on the raw response body in bytes (default 5242880, maximum 10485760)."
                }
            },
            "additionalProperties": false
        })
    }

    fn effects(&self) -> ToolEffects {
        ToolEffects::network()
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        input: Value,
        _on_update: Option<Box<dyn Fn(ToolUpdate) + Send + Sync>>,
    ) -> Result<ToolOutput> {
        let url = input
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::validation("`url` is required"))?;
        let prompt = input.get("prompt").and_then(Value::as_str);
        let max_bytes =
            input
                .get("max_bytes")
                .and_then(Value::as_u64)
                .map_or(DEFAULT_FETCH_BYTES, |n| {
                    usize::try_from(n)
                        .unwrap_or(DEFAULT_FETCH_BYTES)
                        .min(MAX_FETCH_BYTES)
                });
        if max_bytes == 0 {
            return Err(Error::validation("`max_bytes` must be greater than 0"));
        }

        let page = fetch_page(url, max_bytes).await?;
        let is_html = page.content_type.contains("html") || page.content_type.is_empty();
        let (title, text) = if is_html {
            html_to_text(&page.body)
        } else if page.content_type.starts_with("text/")
            || page.content_type.contains("json")
            || page.content_type.contains("xml")
        {
            (None, page.body.clone())
        } else {
            return Err(Error::tool(
                "web_fetch",
                format!(
                    "{} returned {} ({} bytes), which is not text",
                    page.final_url,
                    page.content_type,
                    page.body.len()
                ),
            ));
        };
        let (text, truncated) = truncate_chars(&text, MAX_TEXT_CHARS);

        let mut rendered = String::new();
        if let Some(title) = title.as_ref() {
            rendered.push_str(&format!("# {title}\n\n"));
        }
        rendered.push_str(&format!("Source: {}\n", page.final_url));
        if page.chain.len() > 1 {
            rendered.push_str(&format!("Redirects: {}\n", page.chain.join(" -> ")));
        }
        if let Some(prompt) = prompt {
            rendered.push_str(&format!("Asked for: {prompt}\n"));
        }
        rendered.push('\n');
        rendered.push_str(&text);
        if truncated {
            rendered.push_str(&format!(
                "\n\n[truncated to {MAX_TEXT_CHARS} characters of extracted text]"
            ));
        }

        Ok(ToolOutput {
            content: vec![ContentBlock::Text(TextContent::new(rendered))],
            details: Some(json!({
                "schema": WEB_FETCH_SCHEMA,
                "requestedUrl": url,
                "finalUrl": page.final_url,
                "redirectChain": page.chain,
                "status": page.status,
                "contentType": page.content_type,
                "title": title,
                "bytes": page.body.len(),
                "truncated": truncated,
            })),
            is_error: false,
        })
    }
}

// ============================================================================
// web_search
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchProvider {
    Brave,
    Tavily,
}

impl SearchProvider {
    const fn id(self) -> &'static str {
        match self {
            Self::Brave => "brave",
            Self::Tavily => "tavily",
        }
    }

    const fn key_env(self) -> &'static str {
        match self {
            Self::Brave => "BRAVE_SEARCH_API_KEY",
            Self::Tavily => "TAVILY_API_KEY",
        }
    }
}

struct SearchBackend {
    provider: SearchProvider,
    api_key: String,
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

fn kesa_env_value(suffix: &str) -> Option<String> {
    crate::env::var(suffix)
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

fn no_provider_error() -> Error {
    Error::tool(
        "web_search",
        format!(
            "No search provider is configured. Set {key} to a Brave Search API key \
             (or set {provider}=tavily and {key} to a Tavily key). \
             Provider-specific {} and {} are also read.",
            SearchProvider::Brave.key_env(),
            SearchProvider::Tavily.key_env(),
            key = crate::env::name(SEARCH_API_KEY_ENV),
            provider = crate::env::name(SEARCH_PROVIDER_ENV),
        ),
    )
}

fn resolve_backend() -> Result<SearchBackend> {
    let provider = match kesa_env_value(SEARCH_PROVIDER_ENV)
        .map(|v| v.to_ascii_lowercase())
        .as_deref()
    {
        None | Some("brave") => SearchProvider::Brave,
        Some("tavily") => SearchProvider::Tavily,
        Some(other) => {
            return Err(Error::tool(
                "web_search",
                format!(
                    "Unknown {}=`{other}`; expected `brave` or `tavily`",
                    crate::env::name(SEARCH_PROVIDER_ENV)
                ),
            ));
        }
    };
    let api_key = kesa_env_value(SEARCH_API_KEY_ENV)
        .or_else(|| env_value(provider.key_env()))
        .ok_or_else(no_provider_error)?;
    Ok(SearchBackend { provider, api_key })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

fn host_of(url: &str) -> Option<String> {
    ParsedUrl::parse(url)
        .ok()
        .map(|parsed| strip_brackets(&parsed.host).to_ascii_lowercase())
}

fn host_matches(host: &str, domain: &str) -> bool {
    let domain = domain.trim().trim_start_matches('.').to_ascii_lowercase();
    !domain.is_empty() && (host == domain || host.ends_with(&format!(".{domain}")))
}

fn apply_domain_filters(
    results: Vec<SearchResult>,
    allowed: &[String],
    blocked: &[String],
) -> Vec<SearchResult> {
    results
        .into_iter()
        .filter(|result| {
            let Some(host) = host_of(&result.url) else {
                return allowed.is_empty();
            };
            if !allowed.is_empty() && !allowed.iter().any(|d| host_matches(&host, d)) {
                return false;
            }
            !blocked.iter().any(|d| host_matches(&host, d))
        })
        .collect()
}

fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(*byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn parse_brave(body: &str) -> Result<Vec<SearchResult>> {
    let value: Value = serde_json::from_str(body)
        .map_err(|err| Error::tool("web_search", format!("Brave returned invalid JSON: {err}")))?;
    Ok(value
        .get("web")
        .and_then(|web| web.get("results"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(SearchResult {
                        title: item.get("title").and_then(Value::as_str)?.to_owned(),
                        url: item.get("url").and_then(Value::as_str)?.to_owned(),
                        snippet: item
                            .get("description")
                            .and_then(Value::as_str)
                            .map(|d| normalize_text(&html_to_text(d).1))
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

fn parse_tavily(body: &str) -> Result<Vec<SearchResult>> {
    let value: Value = serde_json::from_str(body)
        .map_err(|err| Error::tool("web_search", format!("Tavily returned invalid JSON: {err}")))?;
    Ok(value
        .get("results")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(SearchResult {
                        title: item.get("title").and_then(Value::as_str)?.to_owned(),
                        url: item.get("url").and_then(Value::as_str)?.to_owned(),
                        snippet: item
                            .get("content")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

async fn run_search(
    backend: &SearchBackend,
    query: &str,
    count: usize,
) -> Result<Vec<SearchResult>> {
    let client = Client::new();
    match backend.provider {
        SearchProvider::Brave => {
            let url = format!(
                "https://api.search.brave.com/res/v1/web/search?q={}&count={count}",
                percent_encode(query)
            );
            let response = client
                .get(&url)
                .header("Accept", "application/json")
                .header("X-Subscription-Token", backend.api_key.clone())
                .send()
                .await?;
            let status = response.status();
            let body = response.text_limited(MAX_SEARCH_BYTES).await?;
            if status != 200 {
                return Err(Error::tool(
                    "web_search",
                    format!("Brave Search returned HTTP {status}: {}", body.trim()),
                ));
            }
            parse_brave(&body)
        }
        SearchProvider::Tavily => {
            let response = client
                .post("https://api.tavily.com/search")
                .header("Authorization", format!("Bearer {}", backend.api_key))
                .json(&json!({
                    "query": query,
                    "max_results": count,
                }))?
                .send()
                .await?;
            let status = response.status();
            let body = response.text_limited(MAX_SEARCH_BYTES).await?;
            if status != 200 {
                return Err(Error::tool(
                    "web_search",
                    format!("Tavily returned HTTP {status}: {}", body.trim()),
                ));
            }
            parse_tavily(&body)
        }
    }
}

fn string_list(input: &Value, key: &str) -> Vec<String> {
    input
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn render_results(query: &str, provider: &str, results: &[SearchResult]) -> String {
    if results.is_empty() {
        return format!("No results for `{query}` from {provider}.");
    }
    let mut out = format!(
        "{} result(s) for `{query}` from {provider}:\n",
        results.len()
    );
    for (index, result) in results.iter().enumerate() {
        out.push_str(&format!(
            "\n{}. {}\n   {}\n",
            index + 1,
            result.title,
            result.url
        ));
        if !result.snippet.is_empty() {
            out.push_str(&format!("   {}\n", result.snippet));
        }
    }
    out
}

/// Search the web through a configured search API.
pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn label(&self) -> &'static str {
        "Search"
    }

    fn description(&self) -> &'static str {
        "Search the web and return ranked results as title, url and snippet. Requires a search API \
         key in the environment; without one the tool reports which key to set rather than \
         guessing. Follow up with web_fetch to read a result."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query."
                },
                "allowed_domains": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Keep only results whose host is, or is under, one of these domains."
                },
                "blocked_domains": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Drop results whose host is, or is under, one of these domains."
                },
                "count": {
                    "type": "integer",
                    "description": "How many results to request (default 10, maximum 20)."
                }
            },
            "additionalProperties": false
        })
    }

    fn effects(&self) -> ToolEffects {
        ToolEffects::network()
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        input: Value,
        _on_update: Option<Box<dyn Fn(ToolUpdate) + Send + Sync>>,
    ) -> Result<ToolOutput> {
        let query = input
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .ok_or_else(|| Error::validation("`query` is required"))?;
        let allowed = string_list(&input, "allowed_domains");
        let blocked = string_list(&input, "blocked_domains");
        let count =
            input
                .get("count")
                .and_then(Value::as_u64)
                .map_or(DEFAULT_SEARCH_RESULTS, |n| {
                    usize::try_from(n)
                        .unwrap_or(DEFAULT_SEARCH_RESULTS)
                        .clamp(1, MAX_SEARCH_RESULTS)
                });

        let backend = resolve_backend()?;
        let raw = run_search(&backend, query, count).await?;
        let total = raw.len();
        let results = apply_domain_filters(raw, &allowed, &blocked);

        Ok(ToolOutput {
            content: vec![ContentBlock::Text(TextContent::new(render_results(
                query,
                backend.provider.id(),
                &results,
            )))],
            details: Some(json!({
                "schema": WEB_SEARCH_SCHEMA,
                "provider": backend.provider.id(),
                "query": query,
                "returned": results.len(),
                "beforeFilters": total,
                "results": results.iter().map(|r| json!({
                    "title": r.title,
                    "url": r.url,
                    "snippet": r.snippet,
                })).collect::<Vec<_>>(),
            })),
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(url: &str) -> SearchResult {
        SearchResult {
            title: "t".into(),
            url: url.into(),
            snippet: String::new(),
        }
    }

    #[test]
    fn non_http_schemes_are_refused() {
        for url in ["file:///etc/passwd", "ftp://example.com/x", "gopher://x"] {
            let err = parse_target(url).unwrap_err().to_string();
            assert!(err.contains("not http or https"), "{url}: {err}");
        }
    }

    #[test]
    fn loopback_and_metadata_addresses_are_blocked() {
        assert_eq!(
            blocked_reason("127.0.0.1".parse().unwrap()),
            Some("loopback address")
        );
        assert_eq!(
            blocked_reason("169.254.169.254".parse().unwrap()),
            Some("link-local address")
        );
        assert_eq!(
            blocked_reason("10.1.2.3".parse().unwrap()),
            Some("RFC1918 private address")
        );
        assert_eq!(
            blocked_reason("192.168.0.1".parse().unwrap()),
            Some("RFC1918 private address")
        );
        assert_eq!(
            blocked_reason("172.16.0.1".parse().unwrap()),
            Some("RFC1918 private address")
        );
        assert_eq!(
            blocked_reason("100.64.0.1".parse().unwrap()),
            Some("CGNAT shared address")
        );
        assert_eq!(
            blocked_reason("::1".parse().unwrap()),
            Some("loopback address")
        );
        assert_eq!(
            blocked_reason("fd00::1".parse().unwrap()),
            Some("unique-local address")
        );
        assert_eq!(
            blocked_reason("fe80::1".parse().unwrap()),
            Some("link-local address")
        );
        assert_eq!(
            blocked_reason("::ffff:127.0.0.1".parse().unwrap()),
            Some("loopback address")
        );
    }

    #[test]
    fn public_addresses_pass() {
        assert_eq!(blocked_reason("93.184.216.34".parse().unwrap()), None);
        assert_eq!(blocked_reason("8.8.8.8".parse().unwrap()), None);
        assert_eq!(blocked_reason("2606:2800:220:1::1".parse().unwrap()), None);
    }

    #[test]
    fn literal_private_urls_are_refused_before_any_connect() {
        asupersync::test_utils::run_test(|| async {
            let err = fetch_page("http://127.0.0.1:1/", 1024).await.unwrap_err();
            let text = err.to_string();
            assert!(text.contains("loopback address"), "{text}");
            assert!(
                text.contains(&crate::env::name(ALLOW_PRIVATE_ENV)),
                "{text}"
            );

            let err = fetch_page("http://169.254.169.254/latest/meta-data/", 1024)
                .await
                .unwrap_err();
            assert!(err.to_string().contains("link-local address"), "{err}");
        });
    }

    #[test]
    fn redirects_resolve_against_the_url_that_issued_them() {
        assert_eq!(
            resolve_redirect("https://a.test/x/y", "https://b.test/z").unwrap(),
            "https://b.test/z"
        );
        assert_eq!(
            resolve_redirect("https://a.test/x/y", "//b.test/z").unwrap(),
            "https://b.test/z"
        );
        assert_eq!(
            resolve_redirect("https://a.test/x/y", "/z").unwrap(),
            "https://a.test/z"
        );
        assert_eq!(
            resolve_redirect("https://a.test/x/y", "z").unwrap(),
            "https://a.test/x/z"
        );
        assert_eq!(
            resolve_redirect("http://a.test:8080/x/y?q=1", "z").unwrap(),
            "http://a.test:8080/x/z"
        );
        assert!(resolve_redirect("https://a.test/", "  ").is_err());
    }

    #[test]
    fn html_becomes_readable_text() {
        let html = "<html><head><title>Example Domain</title>\
                    <style>body{color:red}</style></head>\
                    <body><!-- hi --><h1>Example Domain</h1>\
                    <p>This domain is for use in <b>examples</b>.</p>\
                    <script>var x = '<p>not text</p>';</script>\
                    <p>More&nbsp;info &amp; things &#38; &#x26;.</p></body></html>";
        let (title, text) = html_to_text(html);
        assert_eq!(title.as_deref(), Some("Example Domain"));
        assert_eq!(
            text,
            "Example Domain\n\nThis domain is for use in examples.\n\nMore info & things & &."
        );
        assert!(!text.contains("not text"));
        assert!(!text.contains("color:red"));
    }

    #[test]
    fn text_truncation_reports_itself() {
        let long = "x".repeat(MAX_TEXT_CHARS + 10);
        let (out, truncated) = truncate_chars(&long, MAX_TEXT_CHARS);
        assert!(truncated);
        assert_eq!(out.chars().count(), MAX_TEXT_CHARS);
        let (out, truncated) = truncate_chars("short", MAX_TEXT_CHARS);
        assert!(!truncated);
        assert_eq!(out, "short");
    }

    #[test]
    fn domain_filters_match_on_host_suffix() {
        let results = vec![
            result("https://docs.rs/serde"),
            result("https://rs.evil.test/docs.rs"),
            result("https://blog.example.com/a"),
        ];
        let kept = apply_domain_filters(results.clone(), &["docs.rs".into()], &[]);
        assert_eq!(kept, vec![result("https://docs.rs/serde")]);

        let kept = apply_domain_filters(results, &[], &["example.com".into()]);
        assert_eq!(
            kept,
            vec![
                result("https://docs.rs/serde"),
                result("https://rs.evil.test/docs.rs"),
            ]
        );
    }

    #[test]
    fn missing_credential_names_the_key_to_set() {
        let err = no_provider_error().to_string();
        assert!(err.contains(&crate::env::name(SEARCH_API_KEY_ENV)), "{err}");
        assert!(
            err.contains(&crate::env::name(SEARCH_PROVIDER_ENV)),
            "{err}"
        );
        assert!(err.contains("BRAVE_SEARCH_API_KEY"), "{err}");
    }

    #[test]
    fn brave_payload_becomes_ranked_results() {
        let body = r#"{"web":{"results":[
            {"title":"A","url":"https://a.test/","description":"first <strong>hit</strong>"},
            {"title":"B","url":"https://b.test/"}
        ]}}"#;
        let results = parse_brave(body).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "A");
        assert_eq!(results[0].snippet, "first hit");
        assert_eq!(results[1].snippet, "");
    }

    #[test]
    fn tavily_payload_becomes_ranked_results() {
        let body = r#"{"results":[{"title":"A","url":"https://a.test/","content":"snippet"}]}"#;
        let results = parse_tavily(body).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].snippet, "snippet");
    }

    #[test]
    fn query_is_percent_encoded() {
        assert_eq!(percent_encode("rust web fetch"), "rust%20web%20fetch");
        assert_eq!(percent_encode("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn effects_declare_network() {
        assert!(WebFetchTool.effects().networks());
        assert!(WebSearchTool.effects().networks());
        assert!(!WebFetchTool.effects().writes());
    }
}
