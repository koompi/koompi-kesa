//! Error hints: mapping from error variants to user-facing remediation suggestions.
//!
//! Each error variant maps to:
//! - A 1-line summary (human readable)
//! - 0-2 actionable hints (commands, env vars, paths)
//! - Contextual fields that should be printed with the error
//!
//! # Design Principles
//! - Hints must be stable for testability
//! - Avoid OS-specific hints unless OS is reliably detectable
//! - Never suggest destructive actions
//! - Prefer specific, actionable guidance over generic messages

use crate::error::Error;
use std::fmt::Write as _;

/// A remediation hint for an error.
#[derive(Debug, Clone)]
pub struct ErrorHint {
    /// Brief 1-line summary of the error category.
    pub summary: &'static str,
    /// Actionable hints for the user (0-2 items).
    pub hints: &'static [&'static str],
    /// Context fields that should be displayed with the error.
    pub context_fields: &'static [&'static str],
    /// The one imperative KESA can execute next: a slash command, a key
    /// sequence, or a shell step followed by one. `<provider>` stands for
    /// the active provider, which only the presenter knows.
    pub action: &'static str,
}

/// The glyph that opens an error row, so a reader can tell an error from a
/// notice before reading a word.
pub const ERROR_GLYPH: &str = "\u{2717}";

/// One error, ready to present: what happened, the detail behind it, and the
/// one imperative to run next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorPresentation {
    pub summary: String,
    pub detail: String,
    pub action: String,
}

/// Present a typed error.
pub fn present_error(error: &Error) -> ErrorPresentation {
    presentation(&error.to_string(), &hints_for_error(error))
}

/// Present a failure that only ever reached us as text, such as a provider's
/// message at the end of a turn. Text that already went through
/// [`format_error_with_hints`] is recognised: its `Error:` prefix and its
/// `Suggestions:` block are dropped, because the action supersedes them.
pub fn present_error_text(message: &str) -> ErrorPresentation {
    presentation(message, &provider_hints(message))
}

/// Present a failed tool result. A failed result is a `ToolEnd` carrying
/// `is_error`, never an `Error::Tool`, so this is the entry the TUI uses to
/// reach the tool hints.
pub fn present_tool_error(tool: &str, message: &str) -> ErrorPresentation {
    presentation(message, &tool_hints(tool, message))
}

fn presentation(message: &str, hint: &ErrorHint) -> ErrorPresentation {
    let body = message.trim();
    let body = body.strip_prefix("Error:").map_or(body, str::trim_start);
    let body = body
        .split("\nSuggestions:")
        .next()
        .unwrap_or(body)
        .trim_end();
    let body = body.strip_suffix(hint.summary).map_or(body, str::trim_end);
    let detail = if body == hint.summary { "" } else { body };
    ErrorPresentation {
        summary: hint.summary.to_string(),
        detail: detail.to_string(),
        action: hint.action.to_string(),
    }
}

/// The text of an error row: glyph and summary first, the detail under it,
/// and the next imperative last. The view styles the first and last lines
/// as errors and the detail as plain text.
pub fn render_error(summary: &str, detail: &str, action: &str) -> String {
    let mut out = format!("{ERROR_GLYPH} {summary}");
    for line in detail.lines().filter(|line| !line.trim().is_empty()) {
        out.push('\n');
        out.push_str(line.trim_end());
    }
    out.push_str("\nNext: ");
    out.push_str(action);
    out
}

/// Get remediation hints for an error variant.
///
/// Returns structured hints that can be rendered in any output mode
/// (interactive, print, RPC).
#[allow(clippy::too_many_lines)]
pub fn hints_for_error(error: &Error) -> ErrorHint {
    match error {
        Error::Config(msg) => config_hints(msg),
        Error::SessionNotFound { .. } | Error::Session(_) => session_hints(error),
        Error::Auth(msg) => auth_hints(msg),
        Error::Provider { message, .. } => provider_hints(message),
        Error::Tool { tool, message } => tool_hints(tool, message),
        Error::Validation(msg) => validation_hints(msg),
        Error::Extension(msg) => extension_hints(msg),
        Error::Io(err) => io_hints(err),
        Error::Json(err) => json_hints(err),
        Error::Sqlite(err) => sqlite_hints(err),
        Error::Aborted => aborted_hints(),
        Error::Api(msg) => api_hints(msg),
    }
}

fn config_hints(msg: &str) -> ErrorHint {
    if msg.contains("cassette") {
        return ErrorHint {
            summary: "VCR cassette missing or invalid",
            action: "Re-record the cassette, or unset the VCR env var and retry",
            hints: &[
                "If running tests, set VCR_MODE=record to create cassettes",
                "Or ensure VCR_CASSETTE_DIR contains the expected cassette file",
            ],
            context_fields: &["file_path"],
        };
    }
    if msg.contains("settings.json") {
        return ErrorHint {
            summary: "Invalid or missing configuration file",
            action: "Fix or remove the settings file, then restart kesa",
            hints: &[
                "Check that ~/.kesa/agent/settings.json exists and is valid JSON",
                "Run 'pi config' to see configuration paths and precedence",
            ],
            context_fields: &["file_path"],
        };
    }
    if msg.contains("models.json") {
        return ErrorHint {
            summary: "Invalid models configuration",
            action: "Fix models.json, then restart kesa",
            hints: &[
                "Verify ~/.kesa/agent/models.json has valid JSON syntax",
                "Check that 'providers' key exists in models.json",
            ],
            context_fields: &["file_path", "parse_error"],
        };
    }
    ErrorHint {
        summary: "Configuration error",
        action: "Fix the setting named above, then restart kesa",
        hints: &["Check configuration file syntax and required fields"],
        context_fields: &[],
    }
}

fn session_hints(error: &Error) -> ErrorHint {
    match error {
        Error::SessionNotFound { .. } => ErrorHint {
            summary: "Session file not found",
            action: "Run kesa without --session, or /resume to pick a session",
            hints: &[
                "Use 'kesa' without --session to start a new session",
                "Use 'pi --resume' to pick from existing sessions",
            ],
            context_fields: &["path"],
        },
        Error::Session(msg) if msg.contains("corrupted") || msg.contains("invalid") => ErrorHint {
            summary: "Session file is corrupted or invalid",
            action: "/new to start a fresh session",
            hints: &[
                "Start a new session with 'kesa'",
                "Session files are JSONL format - check for malformed lines",
            ],
            context_fields: &["path", "line_number"],
        },
        Error::Session(msg) if msg.contains("locked") => ErrorHint {
            summary: "Session file is locked by another process",
            action: "Close the other KESA instance, then retry",
            hints: &["Close other KESA instances using this session"],
            context_fields: &["path"],
        },
        _ => ErrorHint {
            summary: "Session error",
            action: "/new to start a fresh session",
            hints: &["Try starting a new session with 'kesa'"],
            context_fields: &[],
        },
    }
}

fn auth_hints(msg: &str) -> ErrorHint {
    if msg.contains("GitHub Copilot") && msg.contains("client_id") {
        return ErrorHint {
            summary: "GitHub Copilot OAuth client_id not configured",
            action: "Set the client_id in the provider config, then /login github-copilot",
            hints: &[
                "Set GITHUB_COPILOT_CLIENT_ID to your GitHub OAuth App / GitHub App client id",
                "Or run on a workstation with a browser, or use device flow over SSH (set KESA_COPILOT_FORCE_DEVICE_FLOW=1)",
            ],
            context_fields: &["provider"],
        };
    }
    if msg.contains("API key") || msg.contains("api_key") {
        return ErrorHint {
            summary: "API key not configured",
            action: "/login <provider>, or export the provider's API key and restart kesa",
            hints: &[
                "Set ANTHROPIC_API_KEY environment variable",
                "Or add key to ~/.kesa/agent/auth.json",
            ],
            context_fields: &["provider"],
        };
    }
    if msg.contains("401") || msg.contains("unauthorized") {
        return ErrorHint {
            summary: "API key is invalid or expired",
            action: "/login <provider> to replace the key",
            hints: &[
                "Verify your API key is correct and active",
                "Check API key permissions at your provider's console",
            ],
            context_fields: &["provider", "status_code"],
        };
    }
    if msg.contains("OAuth") || msg.contains("refresh") {
        return ErrorHint {
            summary: "OAuth token expired or invalid",
            action: "/login <provider> to sign in again",
            hints: &[
                "Run 'pi login <provider>' to re-authenticate",
                "Or set API key directly via environment variable",
            ],
            context_fields: &["provider"],
        };
    }
    if msg.contains("lock") {
        return ErrorHint {
            summary: "Auth file locked by another process",
            action: "Close the other KESA instance, then retry",
            hints: &["Close other KESA instances that may be using auth.json"],
            context_fields: &["path"],
        };
    }
    ErrorHint {
        summary: "Authentication error",
        action: "/login <provider>",
        hints: &["Check your API credentials"],
        context_fields: &[],
    }
}

fn provider_hints(message: &str) -> ErrorHint {
    // Connect-path errors re-wrapped by providers keep the original message
    // text, so match the WSAENOTCONN signature here too (#106).
    if message.contains("10057") || message.contains("Socket is not connected") {
        return winsock_not_connected_hints();
    }
    if message.contains("429") || message.contains("rate limit") {
        return ErrorHint {
            summary: "Rate limit exceeded",
            action: "wait, then Up then Enter to retry",
            hints: &[
                "Wait a moment and try again",
                "Consider using a different model or reducing request frequency",
            ],
            context_fields: &["provider", "retry_after"],
        };
    }
    if message.contains("500") || message.contains("server error") {
        return ErrorHint {
            summary: "Provider server error",
            action: "Up then Enter to retry; /model to switch if it persists",
            hints: &[
                "This is a temporary issue - try again shortly",
                "Check provider status page for outages",
            ],
            context_fields: &["provider", "status_code"],
        };
    }
    if is_name_resolution_failure(message) {
        return name_resolution_hints();
    }
    if message.contains("connection") || message.contains("network") {
        return ErrorHint {
            summary: "Network connection error",
            action: "Check the connection, then Up then Enter to retry",
            hints: &[
                "Check your internet connection",
                "If using a proxy, verify proxy settings",
            ],
            context_fields: &["provider", "url"],
        };
    }
    if message.contains("timeout") {
        return ErrorHint {
            summary: "Request timed out",
            action: "Up then Enter to retry",
            hints: &[
                "Try again - the provider may be slow",
                "Consider using a smaller context or simpler request",
            ],
            context_fields: &["provider", "timeout_seconds"],
        };
    }
    if message.contains("model") && message.contains("not found") {
        return ErrorHint {
            summary: "Model not found or unavailable",
            action: "/model to pick an available model",
            hints: &[
                "Check that the model ID is correct",
                "Use 'kesa --list-models' to see available models",
            ],
            context_fields: &["provider", "model_id"],
        };
    }
    let lowered = message.to_ascii_lowercase();
    if lowered.contains("401")
        || lowered.contains("token_expired")
        || lowered.contains("unauthorized")
    {
        return ErrorHint {
            summary: "Provider rejected the credentials",
            action: "/login <provider>",
            hints: &[
                "Run /login to sign in again",
                "A token read from another agent's install (~/.codex/auth.json, ~/.claude/.credentials.json) is used as-is and never refreshed; /login stores one KESA can refresh",
            ],
            context_fields: &["provider", "status_code"],
        };
    }
    ErrorHint {
        summary: "Provider API error",
        action: "Up then Enter to retry",
        hints: &["Check provider documentation for this error"],
        context_fields: &["provider", "status_code"],
    }
}

fn tool_hints(tool: &str, message: &str) -> ErrorHint {
    if tool == "read" && message.contains("not found") {
        return ErrorHint {
            summary: "File not found",
            action: "Give the right path, then Up then Enter to retry",
            hints: &[
                "Verify the file path is correct",
                "Use 'ls' or 'find' to locate the file",
            ],
            context_fields: &["path"],
        };
    }
    if tool == "read" && message.contains("permission") {
        return ErrorHint {
            summary: "Permission denied reading file",
            action: "Fix the file permissions, then Up then Enter to retry",
            hints: &["Check file permissions"],
            context_fields: &["path"],
        };
    }
    if tool == "write" && message.contains("permission") {
        return ErrorHint {
            summary: "Permission denied writing file",
            action: "Fix the directory permissions, then Up then Enter to retry",
            hints: &["Check directory permissions"],
            context_fields: &["path"],
        };
    }
    if tool == "edit" && message.contains("not found") {
        return ErrorHint {
            summary: "Text to replace not found in file",
            action: "Up then Enter to retry; the agent re-reads the file",
            hints: &[
                "Verify the old_text exactly matches content in the file",
                "Use 'read' to see the current file content",
            ],
            context_fields: &["path", "old_text_preview"],
        };
    }
    if tool == "edit" && message.contains("ambiguous") {
        return ErrorHint {
            summary: "Multiple matches found for replacement",
            action: "Up then Enter to retry; the agent adds context",
            hints: &["Provide more context in old_text to make it unique"],
            context_fields: &["path", "match_count"],
        };
    }
    if tool == "bash" && message.contains("timeout") {
        return ErrorHint {
            summary: "Command timed out",
            action: "Up then Enter to retry with a longer timeout",
            hints: &[
                "Increase timeout with 'timeout' parameter",
                "Consider breaking into smaller commands",
            ],
            context_fields: &["command", "timeout_seconds"],
        };
    }
    if tool == "bash" && message.contains("exit code") {
        return ErrorHint {
            summary: "Command failed with non-zero exit code",
            action: "Read the output above, then Up then Enter to retry",
            hints: &["Review command output for error details"],
            context_fields: &["command", "exit_code", "stderr"],
        };
    }
    if tool == "grep" && message.contains("pattern") {
        return ErrorHint {
            summary: "Invalid regex pattern",
            action: "Up then Enter to retry with the pattern fixed",
            hints: &["Check regex syntax - special characters may need escaping"],
            context_fields: &["pattern"],
        };
    }
    if tool == "find" && message.contains("fd") {
        return ErrorHint {
            summary: "fd command not found",
            action: "Install fd, then Up then Enter to retry",
            hints: &[
                "Install fd: 'apt install fd-find' or 'brew install fd'",
                "The binary may be named 'fdfind' on some systems",
            ],
            context_fields: &[],
        };
    }
    ErrorHint {
        summary: "Tool execution error",
        action: "Up then Enter to retry",
        hints: &["Review the tool parameters and try again"],
        context_fields: &["tool", "command"],
    }
}

fn validation_hints(msg: &str) -> ErrorHint {
    if msg.contains("required") {
        return ErrorHint {
            summary: "Required field missing",
            action: "Up then Enter to retry",
            hints: &["Provide all required parameters"],
            context_fields: &["field_name"],
        };
    }
    if msg.contains("type") {
        return ErrorHint {
            summary: "Invalid parameter type",
            action: "Up then Enter to retry",
            hints: &["Check parameter types match expected schema"],
            context_fields: &["field_name", "expected_type"],
        };
    }
    ErrorHint {
        summary: "Validation error",
        action: "Fix the input, then Up then Enter to retry",
        hints: &["Check input parameters"],
        context_fields: &[],
    }
}

fn extension_hints(msg: &str) -> ErrorHint {
    if msg.contains("not found") {
        return ErrorHint {
            summary: "Extension not found",
            action: "Fix the extension path in settings, then /reload",
            hints: &[
                "Check extension name is correct",
                "Use 'pi list' to see installed extensions",
            ],
            context_fields: &["extension_name"],
        };
    }
    if msg.contains("manifest") {
        return ErrorHint {
            summary: "Invalid extension manifest",
            action: "Fix the manifest, then /reload",
            hints: &[
                "Check extension manifest.json syntax",
                "Verify required fields are present",
            ],
            context_fields: &["extension_name", "manifest_path"],
        };
    }
    if msg.contains("capability") || msg.contains("permission") {
        return ErrorHint {
            summary: "Extension capability denied",
            action: "Answer the permission prompt, or edit permissions.json, then retry",
            hints: &[
                "Extension requires capabilities not granted by policy",
                "Review extension security settings",
            ],
            context_fields: &["extension_name", "capability"],
        };
    }
    ErrorHint {
        summary: "Extension error",
        action: "Fix the extension, then /reload",
        hints: &["Check extension configuration"],
        context_fields: &["extension_name"],
    }
}

/// Hint for Windows `WSAENOTCONN` (os error 10057) "Socket is not connected"
/// failures during connect/TLS handshake (#106, #66, asupersync#35).
///
/// Layered Winsock providers (VPN clients, antivirus, firewall LSPs) can
/// report an outbound connect as complete while the base provider socket has
/// not finished connecting, so the first send fails with 10057. Pi retries the
/// connect automatically; if the error still surfaces the interference is
/// persistent and needs OS-level remediation.
const fn winsock_not_connected_hints() -> ErrorHint {
    ErrorHint {
        summary: "Socket not connected (Windows WSAENOTCONN 10057) - often VPN, antivirus, or Winsock LSP interference",
        action: "Disable the VPN or LSP, then Up then Enter to retry",
        hints: &[
            "Retry the request; if it persists, temporarily disable VPN/antivirus/firewall software to identify the interfering layer",
            "Inspect layered providers with 'netsh winsock show catalog'; as a last resort run 'netsh winsock reset' from an elevated prompt and reboot",
        ],
        context_fields: &["url"],
    }
}

/// getaddrinfo failures reach us as `Uncategorized`, so the kind says nothing
/// and the text is the only signal.
fn is_name_resolution_failure(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    [
        "failed to lookup address information",
        "name resolution",
        "name or service not known",
        "nodename nor servname",
        "dns error",
        "no such host",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

fn name_resolution_hints() -> ErrorHint {
    ErrorHint {
        summary: "Cannot resolve the provider's hostname",
        action: "Check DNS or the base URL, then Up then Enter to retry",
        hints: &[
            "Check that this machine is online and DNS is working",
            "If you are on a VPN or split-DNS resolver, confirm it is up",
            "Set HTTPS_PROXY if the network requires a proxy",
        ],
        context_fields: &["url"],
    }
}

fn io_hints(err: &std::io::Error) -> ErrorHint {
    // Windows WSAENOTCONN (10057): kind() maps to NotConnected on Windows,
    // but also check the raw OS code in case the error was synthesized with
    // an uncategorized kind (#106).
    if err.kind() == std::io::ErrorKind::NotConnected || err.raw_os_error() == Some(10057) {
        return winsock_not_connected_hints();
    }
    if is_name_resolution_failure(&err.to_string()) {
        return name_resolution_hints();
    }
    match err.kind() {
        std::io::ErrorKind::ConnectionRefused
        | std::io::ErrorKind::ConnectionReset
        | std::io::ErrorKind::ConnectionAborted
        | std::io::ErrorKind::HostUnreachable
        | std::io::ErrorKind::NetworkUnreachable
        | std::io::ErrorKind::NetworkDown => ErrorHint {
            summary: "Cannot reach the provider",
            action: "Check the connection, then Up then Enter to retry",
            hints: &[
                "Check network connectivity and any firewall or proxy",
                "Retry: the endpoint may be briefly unavailable",
            ],
            context_fields: &["url"],
        },
        std::io::ErrorKind::TimedOut => ErrorHint {
            summary: "Network request timed out",
            action: "Up then Enter to retry",
            hints: &[
                "Check connectivity, then retry",
                "A slow proxy or VPN can exceed the request deadline",
            ],
            context_fields: &["url"],
        },
        std::io::ErrorKind::NotFound => ErrorHint {
            summary: "File or directory not found",
            action: "Give the right path, then retry",
            hints: &["Verify the path exists"],
            context_fields: &["path"],
        },
        std::io::ErrorKind::PermissionDenied => ErrorHint {
            summary: "Permission denied",
            action: "Fix the permissions, then retry",
            hints: &["Check file/directory permissions"],
            context_fields: &["path"],
        },
        std::io::ErrorKind::AlreadyExists => ErrorHint {
            summary: "File already exists",
            action: "Pick another path, then retry",
            hints: &["Use a different path or remove existing file first"],
            context_fields: &["path"],
        },
        _ => ErrorHint {
            summary: "I/O error",
            action: "Up then Enter to retry",
            hints: &["Check file system and permissions"],
            context_fields: &["path"],
        },
    }
}

fn json_hints(err: &serde_json::Error) -> ErrorHint {
    if err.is_syntax() {
        return ErrorHint {
            summary: "Invalid JSON syntax",
            action: "Fix the JSON, then retry",
            hints: &[
                "Check for missing commas, brackets, or quotes",
                "Validate JSON at jsonlint.com or similar",
            ],
            context_fields: &["line", "column"],
        };
    }
    if err.is_data() {
        return ErrorHint {
            summary: "JSON data does not match expected structure",
            action: "Fix the JSON, then retry",
            hints: &["Check that JSON fields match expected schema"],
            context_fields: &["field_path"],
        };
    }
    ErrorHint {
        summary: "JSON error",
        action: "Fix the JSON, then retry",
        hints: &["Verify JSON syntax and structure"],
        context_fields: &[],
    }
}

fn sqlite_hints(err: &sqlmodel_core::Error) -> ErrorHint {
    let message = err.to_string();
    if message.contains("locked") {
        return ErrorHint {
            summary: "Database locked",
            action: "Close the other KESA instance, then retry",
            hints: &["Close other KESA instances using this database"],
            context_fields: &["db_path"],
        };
    }
    if message.contains("corrupt") {
        return ErrorHint {
            summary: "Database corrupted",
            action: "Move the database file aside, then restart kesa",
            hints: &[
                "The session index may need to be rebuilt",
                "Delete ~/.kesa/agent/sessions/index.db to rebuild",
            ],
            context_fields: &["db_path"],
        };
    }
    ErrorHint {
        summary: "Database error",
        action: "Restart kesa",
        hints: &["Check database file permissions and integrity"],
        context_fields: &["db_path"],
    }
}

const fn aborted_hints() -> ErrorHint {
    ErrorHint {
        summary: "Operation cancelled by user",
        action: "Up then Enter to send it again",
        hints: &[],
        context_fields: &[],
    }
}

fn api_hints(msg: &str) -> ErrorHint {
    // TLS connect failures arrive flattened into a message string, e.g.
    // "TLS connect failed: I/O error: Socket is not connected. (os error
    // 10057)" (#106).
    if msg.contains("10057") || msg.contains("Socket is not connected") {
        return winsock_not_connected_hints();
    }
    if msg.contains("timed out") || msg.contains("timeout") {
        return ErrorHint {
            summary: "Request timed out",
            action: "Up then Enter to retry",
            hints: &[
                "Raise the timeout: --request-timeout <seconds>, KESA_HTTP_REQUEST_TIMEOUT_SECS=<seconds>, or requestTimeoutSecs in settings.json (0 = no timeout)",
                "Local providers (Ollama/LM Studio): the first request can block while the model loads — ensure the model is pulled (ollama pull <model>) and the server is reachable (ollama list)",
            ],
            context_fields: &["url", "timeout_seconds"],
        };
    }
    if msg.contains("401") {
        return ErrorHint {
            summary: "Unauthorized API request",
            action: "/login <provider>",
            hints: &["Check your API credentials"],
            context_fields: &["url", "status_code"],
        };
    }
    if msg.contains("403") {
        return ErrorHint {
            summary: "Forbidden API request",
            action: "/login <provider>, or check the account's access to this model",
            hints: &["Check API key permissions for this resource"],
            context_fields: &["url", "status_code"],
        };
    }
    if msg.contains("404") {
        return ErrorHint {
            summary: "API resource not found",
            action: "/model to pick another model, or check the base URL",
            hints: &["Check the API endpoint URL"],
            context_fields: &["url"],
        };
    }
    ErrorHint {
        summary: "API error",
        action: "Up then Enter to retry",
        hints: &["Check API documentation"],
        context_fields: &["url", "status_code"],
    }
}

/// Format an error with its hints for display.
///
/// Returns a formatted string suitable for terminal output.
pub fn format_error_with_hints(error: &Error) -> String {
    render_with_hints(&error.to_string(), &hints_for_error(error))
}

/// Same shape as [`format_error_with_hints`], for the turn-failure paths that
/// only ever see the provider's message text and never the typed error.
pub fn format_error_text_with_hints(message: &str) -> String {
    render_with_hints(message, &provider_hints(message))
}

fn render_with_hints(message: &str, hint: &ErrorHint) -> String {
    let mut output = String::new();
    let _ = writeln!(&mut output, "Error: {message}");

    if !message.contains(hint.summary) {
        output.push('\n');
        output.push_str(hint.summary);
        output.push('\n');
    }

    if !hint.hints.is_empty() {
        output.push_str("\nSuggestions:\n");
        for &h in hint.hints {
            let _ = writeln!(&mut output, "  • {h}");
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_429_ends_in_the_retry_imperative() {
        let shown = present_error_text("HTTP 429 Too Many Requests: slow down");
        assert_eq!(shown.summary, "Rate limit exceeded");
        assert_eq!(shown.action, "wait, then Up then Enter to retry");
        let text = render_error(&shown.summary, &shown.detail, &shown.action);
        assert!(text.starts_with("\u{2717} Rate limit exceeded\n"), "{text}");
        assert!(
            text.ends_with("\nNext: wait, then Up then Enter to retry"),
            "{text}"
        );
        assert!(
            text.contains("HTTP 429 Too Many Requests: slow down"),
            "{text}"
        );
    }

    #[test]
    fn text_that_was_already_formatted_loses_its_prefix_and_suggestions() {
        let formatted = format_error_text_with_hints("HTTP 429 Too Many Requests");
        assert!(formatted.contains("Suggestions:"));
        let shown = present_error_text(&formatted);
        assert_eq!(shown.detail, "HTTP 429 Too Many Requests");
        assert!(!shown.detail.contains("Suggestions"));
        let bare = present_error_text("Request failed");
        assert_eq!(bare.summary, "Provider API error");
        assert_eq!(bare.detail, "Request failed");
        let same = present_error_text("Provider API error");
        assert_eq!(
            same.detail, "",
            "a detail that repeats the summary is dropped"
        );
    }

    #[test]
    fn failed_tool_results_reach_the_tool_hints_without_a_typed_error() {
        let shown = present_tool_error("read", "file not found: src/nope.rs");
        assert_eq!(shown.summary, "File not found");
        assert!(shown.action.contains("retry"), "{}", shown.action);
        let typed = present_error(&Error::auth("401 unauthorized"));
        assert!(
            typed.action.starts_with("/login <provider>"),
            "{}",
            typed.action
        );
    }

    #[test]
    fn test_config_error_hints() {
        let error = Error::config("settings.json not found");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("configuration"));
        assert!(!hint.hints.is_empty());
    }

    #[test]
    fn test_auth_error_api_key_hints() {
        let error = Error::auth("API key not set");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("API key"));
        assert!(hint.hints.iter().any(|h| h.contains("ANTHROPIC_API_KEY")));
    }

    #[test]
    fn test_auth_error_401_hints() {
        let error = Error::auth("401 unauthorized");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("invalid") || hint.summary.contains("expired"));
    }

    #[test]
    fn test_provider_expired_token_points_at_login() {
        let error = Error::provider(
            "openai-codex",
            r#"OpenAI API error (HTTP 401): {"code":"token_expired"}"#,
        );
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("credentials"), "{}", hint.summary);
        assert!(hint.hints.iter().any(|h| h.contains("/login")));
    }

    #[test]
    fn test_provider_rate_limit_hints() {
        let error = Error::provider("anthropic", "429 rate limit exceeded");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("Rate limit"));
        assert!(hint.hints.iter().any(|h| h.contains("Wait")));
    }

    #[test]
    fn test_tool_read_not_found_hints() {
        let error = Error::tool("read", "file not found: /path/to/file");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("not found"));
        assert!(hint.context_fields.contains(&"path"));
    }

    #[test]
    fn test_tool_edit_ambiguous_hints() {
        let error = Error::tool("edit", "ambiguous match: found 3 occurrences");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("Multiple"));
        assert!(hint.hints.iter().any(|h| h.contains("context")));
    }

    #[test]
    fn test_tool_fd_not_found_hints() {
        let error = Error::tool("find", "fd command not found");
        let hint = hints_for_error(&error);
        assert!(hint.hints.iter().any(|h| h.contains("apt install")));
    }

    #[test]
    fn test_session_not_found_hints() {
        let error = Error::SessionNotFound {
            path: "/path/to/session.jsonl".to_string(),
        };
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("not found"));
        assert!(hint.hints.iter().any(|h| h.contains("--resume")));
    }

    #[test]
    fn test_json_syntax_error_hints() {
        let json_err = serde_json::from_str::<serde_json::Value>("{ invalid }").unwrap_err();
        let error = Error::Json(Box::new(json_err));
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("JSON") || hint.summary.contains("syntax"));
    }

    #[test]
    fn test_aborted_has_no_hints() {
        let error = Error::Aborted;
        let hint = hints_for_error(&error);
        assert!(hint.hints.is_empty());
    }

    #[test]
    fn test_format_error_with_hints() {
        let error = Error::auth("API key not set");
        let formatted = format_error_with_hints(&error);
        assert!(formatted.contains("Error:"));
        assert!(formatted.contains("Suggestions:"));
    }

    #[test]
    fn test_format_error_with_hints_includes_api_key_suggestion() {
        let error = Error::auth("API key not set");
        let formatted = format_error_with_hints(&error);
        assert!(formatted.contains("ANTHROPIC_API_KEY"));
        assert!(formatted.contains("auth.json"));
    }

    #[test]
    fn test_format_error_with_hints_includes_json_syntax_suggestions() {
        let json_err = serde_json::from_str::<serde_json::Value>("{ invalid }").unwrap_err();
        let error = Error::Json(Box::new(json_err));
        let formatted = format_error_with_hints(&error);
        assert!(formatted.contains("Invalid JSON syntax"));
        assert!(formatted.contains("Validate JSON"));
    }

    #[test]
    fn test_format_error_with_hints_includes_fd_install_hint() {
        let error = Error::tool("find", "fd command not found");
        let formatted = format_error_with_hints(&error);
        assert!(formatted.contains("fd"));
        assert!(formatted.contains("apt install"));
    }

    #[test]
    fn test_format_error_with_hints_includes_read_permission_hint() {
        let error = Error::tool("read", "permission denied: /etc/shadow");
        let formatted = format_error_with_hints(&error);
        assert!(formatted.contains("Permission denied"));
        assert!(formatted.contains("Check file permissions"));
    }

    #[test]
    fn test_format_error_with_hints_includes_vcr_cassette_hint() {
        let error = Error::config("Failed to read cassette /tmp/cassette.json: missing file");
        let formatted = format_error_with_hints(&error);
        assert!(formatted.contains("VCR cassette"));
        assert!(formatted.contains("VCR_MODE=record"));
        assert!(formatted.contains("VCR_CASSETTE_DIR"));
    }

    #[test]
    fn test_extension_capability_denied_hints() {
        let error = Error::extension("capability network not allowed by policy");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("capability") || hint.summary.contains("denied"));
    }

    #[test]
    fn test_provider_timeout_hints() {
        let error = Error::provider("openai", "request timeout after 120s");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("timed out") || hint.summary.contains("timeout"));
    }

    #[test]
    fn test_provider_connection_hints() {
        let error = Error::provider("anthropic", "connection refused");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("Network") || hint.summary.contains("connection"));
    }

    #[test]
    fn test_io_permission_denied_hints() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let error = Error::Io(Box::new(io_err));
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("Permission"));
    }

    #[test]
    fn test_sqlite_locked_hints() {
        // Create a mock sqlite error string
        let error = Error::session("database locked");
        let hint = hints_for_error(&error);
        // Falls back to generic session error since it's not actually a Sqlite variant
        assert!(!hint.hints.is_empty());
    }

    // -----------------------------------------------------------------------
    // config_hints additional branches
    // -----------------------------------------------------------------------

    #[test]
    fn test_config_models_json_hints() {
        let error = Error::config("models.json parse error at line 5");
        let hint = hints_for_error(&error);
        assert_eq!(hint.summary, "Invalid models configuration");
        assert!(hint.context_fields.contains(&"parse_error"));
    }

    #[test]
    fn test_config_generic_fallback() {
        let error = Error::config("some unknown config issue");
        let hint = hints_for_error(&error);
        assert_eq!(hint.summary, "Configuration error");
    }

    // -----------------------------------------------------------------------
    // session_hints additional branches
    // -----------------------------------------------------------------------

    #[test]
    fn test_session_corrupted_hints() {
        let error = Error::session("file corrupted at line 42");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("corrupted"));
        assert!(hint.context_fields.contains(&"line_number"));
    }

    #[test]
    fn test_session_invalid_hints() {
        let error = Error::session("invalid session format");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("corrupted") || hint.summary.contains("invalid"));
    }

    #[test]
    fn test_session_locked_hints() {
        let error = Error::session("session file locked by pid 1234");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("locked"));
        assert!(hint.hints.iter().any(|h| h.contains("Close")));
    }

    #[test]
    fn test_session_generic_fallback() {
        let error = Error::session("something went wrong");
        let hint = hints_for_error(&error);
        assert_eq!(hint.summary, "Session error");
    }

    // -----------------------------------------------------------------------
    // auth_hints additional branches
    // -----------------------------------------------------------------------

    #[test]
    fn test_auth_oauth_hints() {
        let error = Error::auth("OAuth token expired for provider X");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("OAuth"));
        assert!(hint.hints.iter().any(|h| h.contains("pi login")));
    }

    #[test]
    fn test_auth_refresh_hints() {
        let error = Error::auth("failed to refresh token");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("OAuth"));
    }

    #[test]
    fn test_auth_lock_hints() {
        let error = Error::auth("auth file lock contention");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("locked"));
    }

    #[test]
    fn test_auth_generic_fallback() {
        let error = Error::auth("unknown auth issue");
        let hint = hints_for_error(&error);
        assert_eq!(hint.summary, "Authentication error");
    }

    // -----------------------------------------------------------------------
    // provider_hints additional branches
    // -----------------------------------------------------------------------

    #[test]
    fn test_provider_server_error_500_hints() {
        let error = Error::provider("openai", "500 internal server error");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("server error"));
        assert!(hint.hints.iter().any(|h| h.contains("status page")));
    }

    #[test]
    fn test_provider_server_error_text_hints() {
        let error = Error::provider("anthropic", "server error: bad gateway");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("server error"));
    }

    #[test]
    fn test_provider_model_not_found_hints() {
        let error = Error::provider("openai", "model gpt-99 not found");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("Model not found"));
        assert!(hint.hints.iter().any(|h| h.contains("--list-models")));
    }

    #[test]
    fn test_provider_generic_fallback() {
        let error = Error::provider("unknown", "something broke");
        let hint = hints_for_error(&error);
        assert_eq!(hint.summary, "Provider API error");
    }

    // -----------------------------------------------------------------------
    // tool_hints additional branches
    // -----------------------------------------------------------------------

    #[test]
    fn test_tool_write_permission_hints() {
        let error = Error::tool("write", "permission denied: /etc/config");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("Permission denied"));
        assert!(hint.hints.iter().any(|h| h.contains("directory")));
    }

    #[test]
    fn test_tool_edit_not_found_hints() {
        let error = Error::tool("edit", "text not found in file");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("not found"));
        assert!(hint.hints.iter().any(|h| h.contains("old_text")));
    }

    #[test]
    fn test_tool_bash_timeout_hints() {
        let error = Error::tool("bash", "command timeout after 120s");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("timed out"));
        assert!(hint.context_fields.contains(&"timeout_seconds"));
    }

    #[test]
    fn test_tool_bash_exit_code_hints() {
        let error = Error::tool("bash", "exit code 1");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("exit code"));
        assert!(hint.context_fields.contains(&"stderr"));
    }

    #[test]
    fn test_tool_grep_pattern_hints() {
        let error = Error::tool("grep", "invalid regex pattern: [unterminated");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("regex"));
        assert!(hint.hints.iter().any(|h| h.contains("escaping")));
    }

    #[test]
    fn test_tool_generic_fallback() {
        let error = Error::tool("unknown_tool", "something went wrong");
        let hint = hints_for_error(&error);
        assert_eq!(hint.summary, "Tool execution error");
    }

    // -----------------------------------------------------------------------
    // validation_hints branches
    // -----------------------------------------------------------------------

    #[test]
    fn test_validation_required_hints() {
        let error = Error::validation("field 'name' is required");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("Required"));
        assert!(hint.context_fields.contains(&"field_name"));
    }

    #[test]
    fn test_validation_type_hints() {
        let error = Error::validation("expected type string, got number");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("type"));
        assert!(hint.context_fields.contains(&"expected_type"));
    }

    #[test]
    fn test_validation_generic_fallback() {
        let error = Error::validation("value out of range");
        let hint = hints_for_error(&error);
        assert_eq!(hint.summary, "Validation error");
    }

    // -----------------------------------------------------------------------
    // extension_hints additional branches
    // -----------------------------------------------------------------------

    #[test]
    fn test_extension_not_found_hints() {
        let error = Error::extension("extension my-ext not found");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("not found"));
        assert!(hint.hints.iter().any(|h| h.contains("pi list")));
    }

    #[test]
    fn test_extension_manifest_hints() {
        let error = Error::extension("invalid manifest for extension foo");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("manifest"));
        assert!(hint.context_fields.contains(&"manifest_path"));
    }

    #[test]
    fn test_extension_permission_hints() {
        let error = Error::extension("permission denied for exec capability");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("denied"));
    }

    #[test]
    fn test_extension_generic_fallback() {
        let error = Error::extension("runtime crashed");
        let hint = hints_for_error(&error);
        assert_eq!(hint.summary, "Extension error");
    }

    // -----------------------------------------------------------------------
    // io_hints additional branches
    // -----------------------------------------------------------------------

    #[test]
    fn test_io_not_found_hints() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let error = Error::Io(Box::new(io_err));
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("not found"));
    }

    #[test]
    fn test_io_already_exists_hints() {
        let io_err = std::io::Error::new(std::io::ErrorKind::AlreadyExists, "file exists");
        let error = Error::Io(Box::new(io_err));
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("already exists"));
    }

    #[test]
    fn test_io_generic_fallback() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broken");
        let error = Error::Io(Box::new(io_err));
        let hint = hints_for_error(&error);
        assert_eq!(hint.summary, "I/O error");
    }

    #[test]
    fn a_dns_failure_is_not_reported_as_a_filesystem_problem() {
        let io_err = std::io::Error::other(
            "failed to lookup address information: Temporary failure in name resolution",
        );
        let error = Error::Io(Box::new(io_err));
        let hint = hints_for_error(&error);

        assert_eq!(hint.summary, "Cannot resolve the provider's hostname");
        assert!(
            !hint.hints.iter().any(|h| h.contains("file system")),
            "a name resolution failure must not send the user to their filesystem: {:?}",
            hint.hints
        );
        assert!(hint.hints.iter().any(|h| h.contains("DNS")));
    }

    #[test]
    fn an_unreachable_host_points_at_the_network() {
        let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        let error = Error::Io(Box::new(io_err));
        let hint = hints_for_error(&error);
        assert_eq!(hint.summary, "Cannot reach the provider");
    }

    #[test]
    fn test_io_not_connected_hints() {
        let io_err =
            std::io::Error::new(std::io::ErrorKind::NotConnected, "Socket is not connected");
        let error = Error::Io(Box::new(io_err));
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("10057"));
        assert!(hint.hints.iter().any(|h| h.contains("netsh winsock")));
        assert!(hint.hints.iter().any(|h| h.contains("VPN")));
    }

    #[test]
    fn test_io_wsaenotconn_raw_os_error_hints() {
        // Raw os error 10057 must map to the Winsock hint even when the
        // platform does not categorize the kind (non-Windows hosts).
        let io_err = std::io::Error::from_raw_os_error(10057);
        let error = Error::Io(Box::new(io_err));
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("10057"));
        assert!(hint.hints.iter().any(|h| h.contains("netsh winsock")));
    }

    // -----------------------------------------------------------------------
    // json_hints additional branches
    // -----------------------------------------------------------------------

    #[test]
    fn test_json_data_error_hints() {
        // Trigger a data error (wrong type for field)
        let json_err = serde_json::from_str::<Vec<i32>>(r#"{"not": "an array"}"#).unwrap_err();
        let error = Error::Json(Box::new(json_err));
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("data") || hint.summary.contains("structure"));
    }

    #[test]
    fn test_json_eof_fallback() {
        // EOF error is neither syntax nor data
        let json_err = serde_json::from_str::<serde_json::Value>("").unwrap_err();
        let error = Error::Json(Box::new(json_err));
        let hint = hints_for_error(&error);
        // EOF may be classified as syntax or generic depending on serde_json version
        assert!(hint.summary.contains("JSON"));
    }

    // -----------------------------------------------------------------------
    // api_hints branches
    // -----------------------------------------------------------------------

    #[test]
    fn test_api_401_hints() {
        let error = Error::api("401 Unauthorized");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("Unauthorized"));
        assert!(hint.context_fields.contains(&"status_code"));
    }

    #[test]
    fn test_api_403_hints() {
        let error = Error::api("403 Forbidden");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("Forbidden"));
        assert!(hint.hints.iter().any(|h| h.contains("permissions")));
    }

    #[test]
    fn test_api_404_hints() {
        let error = Error::api("404 Not Found");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("not found"));
        assert!(hint.context_fields.contains(&"url"));
    }

    #[test]
    fn test_api_generic_fallback() {
        let error = Error::api("502 Bad Gateway");
        let hint = hints_for_error(&error);
        assert_eq!(hint.summary, "API error");
    }

    #[test]
    fn test_api_tls_connect_10057_hints() {
        let error =
            Error::api("TLS connect failed: I/O error: Socket is not connected. (os error 10057)");
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("10057"));
        assert!(hint.hints.iter().any(|h| h.contains("netsh winsock")));
    }

    #[test]
    fn test_provider_socket_not_connected_hints() {
        let error = Error::provider(
            "anthropic",
            "TLS connect failed: I/O error: Socket is not connected. (os error 10057)",
        );
        let hint = hints_for_error(&error);
        assert!(hint.summary.contains("10057"));
        assert!(hint.hints.iter().any(|h| h.contains("netsh winsock")));
    }

    // -----------------------------------------------------------------------
    // format_error_with_hints additional tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_format_error_aborted_no_suggestions() {
        let error = Error::Aborted;
        let formatted = format_error_with_hints(&error);
        assert!(formatted.contains("Error:"));
        assert!(!formatted.contains("Suggestions:"));
    }

    #[test]
    fn test_format_error_includes_summary_when_different() {
        let error = Error::provider("openai", "429 rate limit exceeded");
        let formatted = format_error_with_hints(&error);
        // Summary "Rate limit exceeded" should appear since error message differs
        assert!(formatted.contains("Rate limit"));
        assert!(formatted.contains("Suggestions:"));
    }

    #[test]
    fn test_format_error_io_not_found() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let error = Error::Io(Box::new(io_err));
        let formatted = format_error_with_hints(&error);
        assert!(formatted.contains("not found"));
        assert!(formatted.contains("Verify the path"));
    }

    // -----------------------------------------------------------------------
    // Property-based tests
    // -----------------------------------------------------------------------

    mod proptest_error_hints {
        use super::*;
        use proptest::prelude::*;

        /// Build an Error from an index + message (avoids Clone requirement).
        fn make_error(variant: usize, msg: &str) -> Error {
            match variant % 9 {
                0 => Error::config(msg),
                1 => Error::session(msg),
                2 => Error::auth(msg),
                3 => Error::validation(msg),
                4 => Error::extension(msg),
                5 => Error::api(msg),
                6 => Error::provider("test", msg),
                7 => Error::tool("test", msg),
                _ => Error::Aborted,
            }
        }

        proptest! {
            /// `hints_for_error` never panics on any error variant.
            #[test]
            fn hints_for_error_never_panics(variant in 0..9usize, msg in "[\\w\\s./]{0,80}") {
                let error = make_error(variant, &msg);
                let hint = hints_for_error(&error);
                assert!(!hint.summary.is_empty());
                assert!(hint.hints.len() <= 2);
                assert!(hint.context_fields.len() <= 3);
            }

            /// `format_error_with_hints` never panics and always starts with "Error:".
            #[test]
            fn format_error_never_panics(variant in 0..9usize, msg in "[\\w\\s./]{0,80}") {
                let error = make_error(variant, &msg);
                let formatted = format_error_with_hints(&error);
                assert!(formatted.starts_with("Error:"));
            }

            /// Summary is always non-empty and contains no control characters.
            #[test]
            fn summary_is_clean(variant in 0..9usize, msg in "[\\w\\s./]{0,80}") {
                let error = make_error(variant, &msg);
                let hint = hints_for_error(&error);
                assert!(!hint.summary.is_empty());
                assert!(!hint.summary.contains('\n'));
                assert!(!hint.summary.contains('\r'));
            }

            /// Each hint line is non-empty.
            #[test]
            fn hints_are_nonempty(variant in 0..9usize, msg in "[\\w\\s./]{0,80}") {
                let error = make_error(variant, &msg);
                let hint = hints_for_error(&error);
                for &h in hint.hints {
                    assert!(!h.is_empty());
                }
            }

            /// Each context field is a valid identifier-like string.
            #[test]
            fn context_fields_are_identifiers(variant in 0..9usize, msg in "[\\w\\s./]{0,80}") {
                let error = make_error(variant, &msg);
                let hint = hints_for_error(&error);
                for &field in hint.context_fields {
                    assert!(!field.is_empty());
                    assert!(field.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
                }
            }

            /// Config error with "cassette" always maps to VCR hint.
            #[test]
            fn config_cassette_keyword_triggers_vcr(prefix in "[a-zA-Z ]{0,30}") {
                let msg = format!("{prefix} cassette missing");
                let error = Error::config(msg);
                let hint = hints_for_error(&error);
                assert_eq!(hint.summary, "VCR cassette missing or invalid");
            }

            /// Config error with "settings.json" always maps to settings hint.
            #[test]
            fn config_settings_keyword_triggers_settings(prefix in "[a-zA-Z ]{0,30}") {
                let msg = format!("{prefix} settings.json not found");
                let error = Error::config(msg);
                let hint = hints_for_error(&error);
                assert!(hint.summary.contains("configuration"));
            }

            /// Config error with "models.json" always maps to models hint.
            #[test]
            fn config_models_keyword_triggers_models(prefix in "[a-zA-Z ]{0,30}") {
                let msg = format!("{prefix} models.json parse error");
                let error = Error::config(msg);
                let hint = hints_for_error(&error);
                assert_eq!(hint.summary, "Invalid models configuration");
            }

            /// Auth error with "API key" always mentions API key setup.
            #[test]
            fn auth_api_key_keyword(suffix in "[a-zA-Z ]{0,30}") {
                let msg = format!("API key {suffix}");
                let error = Error::auth(msg);
                let hint = hints_for_error(&error);
                assert!(hint.summary.contains("API key"));
                assert!(hint.hints.iter().any(|h| h.contains("ANTHROPIC_API_KEY")));
            }

            /// Provider error with "429" always triggers rate limit hint.
            #[test]
            fn provider_429_triggers_rate_limit(provider in "[a-z]{1,10}", suffix in "[a-zA-Z ]{0,30}") {
                let msg = format!("429 {suffix}");
                let error = Error::provider(provider, msg);
                let hint = hints_for_error(&error);
                assert_eq!(hint.summary, "Rate limit exceeded");
            }

            /// Provider error with "timeout" always triggers timeout hint.
            #[test]
            fn provider_timeout_triggers_timeout_hint(provider in "[a-z]{1,10}", prefix in "[a-zA-Z ]{0,30}") {
                let msg = format!("{prefix} timeout");
                let error = Error::provider(provider, msg);
                let hint = hints_for_error(&error);
                assert!(!hint.summary.is_empty());
            }

            /// Aborted error always has empty hints.
            #[test]
            fn aborted_always_empty_hints(_dummy in 0..10u32) {
                let hint = hints_for_error(&Error::Aborted);
                assert!(hint.hints.is_empty());
                assert!(hint.context_fields.is_empty());
                assert_eq!(hint.summary, "Operation cancelled by user");
            }

            /// `format_error_with_hints` includes "Suggestions:" iff hints are non-empty.
            #[test]
            fn format_includes_suggestions_iff_hints(variant in 0..9usize, msg in "[\\w\\s./]{0,80}") {
                let error = make_error(variant, &msg);
                let hint = hints_for_error(&error);
                let formatted = format_error_with_hints(&error);
                if hint.hints.is_empty() {
                    assert!(!formatted.contains("Suggestions:"));
                } else {
                    assert!(formatted.contains("Suggestions:"));
                }
            }

            /// Tool error category detection: "read" + "not found" → File not found.
            #[test]
            fn tool_read_not_found_hint(suffix in "[a-zA-Z /]{0,40}") {
                let msg = format!("not found {suffix}");
                let error = Error::tool("read", msg);
                let hint = hints_for_error(&error);
                assert_eq!(hint.summary, "File not found");
            }

            /// Tool error: unknown tool always gets generic hint.
            #[test]
            fn tool_unknown_gets_generic(tool in "[a-z]{5,10}", msg in "[a-zA-Z ]{0,40}") {
                let error = Error::tool(tool, msg);
                let hint = hints_for_error(&error);
                assert_eq!(hint.summary, "Tool execution error");
            }
        }
    }
}
