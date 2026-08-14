use super::*;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Transcript gutter glyphs. One column each, so every speaker's text starts in
/// the same place and the transcript reads as a single column.
const USER_PROMPT_GLYPH: &str = "\u{203a}";
const TOOL_RESULT_GLYPH: &str = "\u{23bf}";

const APP_LABEL: &str = "KESA";

/// Prompt shown on the first row inside the input box.
const INPUT_PROMPT_GLYPH: &str = ">";

/// Smallest box we will draw before giving up on fitting the terminal.
const MIN_BOX_WIDTH: usize = 16;

/// The autocomplete list stays readable rather than stretching to the terminal.
const DROPDOWN_MAX_WIDTH: usize = 64;

/// Outer width of every framed box, leaving the two-column left gutter the rest
/// of the view uses plus a matching margin on the right.
pub(super) const fn box_width(term_width: usize) -> usize {
    let width = term_width.saturating_sub(4);
    if width < MIN_BOX_WIDTH {
        MIN_BOX_WIDTH
    } else {
        width
    }
}

/// Editor columns that fit inside the input box, after its frame, its own
/// padding and the `>` prompt column.
pub(super) const fn editor_width(term_width: usize, editor_padding_x: usize) -> usize {
    let inner = box_width(term_width) - 4;
    let width = inner.saturating_sub(2 + editor_padding_x);
    if width < 4 { 4 } else { width }
}

/// Ensure the view output fits within `term_height` terminal rows.
///
/// The output must contain at most `term_height - 1` newline characters so
/// that the cursor never advances past the last visible row, which would
/// trigger terminal scrolling in the alternate-screen buffer.
pub(super) fn clamp_to_terminal_height(mut output: String, term_height: usize) -> String {
    if term_height == 0 {
        output.clear();
        return output;
    }
    let max_newlines = term_height.saturating_sub(1);

    // Single-pass: use memchr to jump directly to each newline position.
    // Finds the (max_newlines+1)-th newline and truncates there, or returns
    // early if the output has fewer newlines than the limit.
    let bytes = output.as_bytes();
    let mut pos = 0;
    for _ in 0..max_newlines {
        match memchr::memchr(b'\n', &bytes[pos..]) {
            Some(offset) => pos += offset + 1,
            None => return output, // Fewer newlines than limit — fits.
        }
    }
    // `pos` is now past the max_newlines-th newline.  If there's another
    // newline at or after `pos`, the output exceeds the limit — truncate
    // just before that next newline.
    if let Some(offset) = memchr::memchr(b'\n', &bytes[pos..]) {
        output.truncate(pos + offset);
    }
    output
}

/// Terminal rows a frame block occupies. Every row is newline-terminated.
fn block_rows(block: &str) -> usize {
    memchr::memchr_iter(b'\n', block.as_bytes()).count()
}

/// Append `count` empty rows, pushing what follows down the screen.
fn pad_rows(output: &mut String, count: usize) {
    output.reserve(count);
    for _ in 0..count {
        output.push('\n');
    }
}

pub(super) fn normalize_raw_terminal_newlines(input: String) -> String {
    if !input.contains('\n') {
        return input;
    }

    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len() + 16);
    let mut cursor = 0usize;

    // Byte-scan with memchr avoids UTF-8 decode on the hot view() path.
    for newline_idx in memchr::memchr_iter(b'\n', bytes) {
        out.push_str(&input[cursor..newline_idx]);
        if newline_idx == 0 || bytes[newline_idx - 1] != b'\r' {
            out.push('\r');
        }
        out.push('\n');
        cursor = newline_idx + 1;
    }

    out.push_str(&input[cursor..]);
    out
}

/// Draw a rounded border around `content`, one output line per row.
///
/// `width` is the full outer width in cells, borders included; content is
/// padded or truncated to `width - 4` so the frame stays rectangular even when
/// a row already carries styling escapes. Only the border glyphs take `style`,
/// so pre-styled content passes through untouched.
pub(super) fn bordered_box<'a>(
    content: impl IntoIterator<Item = &'a str>,
    width: usize,
    style: &lipgloss::Style,
) -> Vec<String> {
    let width = width.max(4);
    let inner = width - 4;
    let rule = "\u{2500}".repeat(width - 2);

    let mut lines = vec![style.render(&format!("\u{256d}{rule}\u{256e}"))];
    let left = style.render("\u{2502}");
    let right = style.render("\u{2502}");
    for row in content {
        let row = fit_to_width(row, inner);
        lines.push(format!("{left} {row} {right}"));
    }
    lines.push(style.render(&format!("\u{2570}{rule}\u{256f}")));
    lines
}

/// Pad with spaces or truncate `line` so it occupies exactly `width` cells,
/// counting display width and ignoring ANSI escapes.
fn fit_to_width(line: &str, width: usize) -> String {
    let visible = lipgloss::width(line);
    if visible == width {
        return line.to_string();
    }
    if visible < width {
        let mut out = line.to_string();
        out.push_str(&" ".repeat(width - visible));
        return out;
    }

    let mut out = String::with_capacity(line.len());
    let mut taken = 0usize;
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            out.push(ch);
            for esc in chars.by_ref() {
                out.push(esc);
                if esc.is_ascii_alphabetic() || esc == '\u{7}' {
                    break;
                }
            }
            continue;
        }
        let cell = UnicodeWidthChar::width(ch).unwrap_or(0);
        if taken + cell > width {
            break;
        }
        out.push(ch);
        taken += cell;
    }
    if out.contains('\u{1b}') {
        // truncated mid-sequence would bleed colour into the border
        out.push_str("\u{1b}[0m");
    }
    if taken < width {
        out.push_str(&" ".repeat(width - taken));
    }
    out
}

pub(super) fn progress_suffix(elapsed: Option<std::time::Duration>, tokens: u64) -> String {
    let mut parts = Vec::new();
    if let Some(elapsed) = elapsed {
        parts.push(format!("{}s", elapsed.as_secs()));
    }
    if tokens > 0 {
        parts.push(format!("{} tokens", format_count(tokens as usize)));
    }
    parts.push("esc to interrupt".to_string());
    format!("({})", parts.join(" \u{00b7} "))
}

/// Append one plain-text line with hard wrapping to `max_width` display cells.
///
/// We do explicit wrapping here instead of relying on terminal auto-wrap so the
/// renderer's logical rows stay aligned with physical rows in alt-screen mode.
/// Percent of a context window still free.
///
/// `None` when the registry has no window for the model, or when no turn has
/// reported usage yet: no number beats a guessed one.
fn context_left_percent(window: u32, used: u64) -> Option<u64> {
    let window = u64::from(window);
    if window == 0 || used == 0 {
        return None;
    }
    Some(window.saturating_sub(used.min(window)) * 100 / window)
}

/// Drop the `**` emphasis markers reasoning summaries arrive wrapped in, and
/// put each summary on its own line.
///
/// The block is rendered as muted italic plain text, not markdown, so the
/// markers reach the screen literally. Consecutive summaries are concatenated
/// with no separator at all, which shows up as `**one****two**`; the empty
/// stretch between one run closing and the next opening is the seam.
fn plain_thinking(text: &str) -> String {
    let parts: Vec<&str> = text.split("**").collect();
    let mut out = String::with_capacity(text.len());
    for (idx, part) in parts.iter().enumerate() {
        if part.is_empty() {
            let seam = idx > 0 && idx + 1 < parts.len();
            if seam && !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            continue;
        }
        out.push_str(part);
    }
    out
}

/// Terminal rows `text` occupies at `width`, counting the wrap the terminal
/// does for a line longer than the screen.
pub(super) fn wrapped_rows(text: &str, width: usize) -> usize {
    let width = width.max(1);
    text.lines()
        .map(|line| line.width().div_ceil(width).max(1))
        .sum::<usize>()
        .max(1)
}

/// Join `hints` with two spaces, dropping them from the left until the line
/// fits. The last hint is the most useful one, so it is the last to go.
fn fit_hints(hints: &[String], max_width: usize) -> String {
    for start in 0..hints.len() {
        let line = hints[start..].join("  ");
        if line.width() <= max_width {
            return line;
        }
    }
    truncate(hints.last().map_or("", String::as_str), max_width)
}

/// Visual rows the input box is allowed to occupy before it scrolls inside
/// itself: about a third of the terminal, so the transcript keeps the rest.
pub(super) const fn input_max_rows(term_height: usize) -> usize {
    let rows = term_height / 3;
    if rows == 0 { 1 } else { rows }
}

/// One editor value laid out as visual rows of at most `width` cells.
///
/// `rows` are byte ranges into the value and cover it end to end, so a byte
/// offset maps to a row and back without consulting the text again.
pub(super) struct InputLayout {
    pub rows: Vec<std::ops::Range<usize>>,
    pub cursor_row: usize,
    pub cursor_col: usize,
}

pub(super) fn layout_input(value: &str, cursor_byte: usize, width: usize) -> InputLayout {
    let width = width.max(1);
    let mut rows = Vec::new();
    let mut base = 0usize;
    for line in value.split('\n') {
        wrap_input_line(line, width, base, &mut rows);
        base += line.len() + 1;
    }

    let cursor_row = rows
        .iter()
        .rposition(|row| row.start <= cursor_byte)
        .unwrap_or(0);
    let row = rows.get(cursor_row).cloned().unwrap_or(0..0);
    let cursor_col = value
        .get(row.start..cursor_byte.min(row.end))
        .map_or(0, UnicodeWidthStr::width);

    InputLayout {
        rows,
        cursor_row,
        cursor_col,
    }
}

/// Byte offset of display column `col` within `row`, clamped to the row's end.
pub(super) fn input_byte_at_column(value: &str, row: &std::ops::Range<usize>, col: usize) -> usize {
    let mut width = 0usize;
    for (idx, ch) in value[row.clone()].char_indices() {
        if width >= col {
            return row.start + idx;
        }
        width += UnicodeWidthChar::width(ch).unwrap_or(0);
    }
    row.end
}

/// Break one logical line into rows of at most `width` cells, preferring a
/// break after a space and keeping that space on the row it ends.
fn wrap_input_line(line: &str, width: usize, base: usize, rows: &mut Vec<std::ops::Range<usize>>) {
    if line.is_empty() {
        rows.push(base..base);
        return;
    }

    let mut start = 0usize;
    while start < line.len() {
        let rest = &line[start..];
        let mut used = 0usize;
        let mut overflow = None;
        let mut after_space = None;
        for (idx, ch) in rest.char_indices() {
            let cell = UnicodeWidthChar::width(ch).unwrap_or(0);
            if used + cell > width {
                overflow = Some(idx);
                break;
            }
            used += cell;
            if ch == ' ' {
                after_space = Some(idx + ch.len_utf8());
            }
        }
        match overflow {
            Some(cut) => {
                let cut = after_space.filter(|space| *space < cut).unwrap_or(cut);
                rows.push(base + start..base + start + cut);
                start += cut;
            }
            None => {
                rows.push(base + start..base + line.len());
                // A row filled to the last cell leaves the cursor nowhere to
                // sit, so the next cell gets its own row.
                if used == width {
                    rows.push(base + line.len()..base + line.len());
                }
                return;
            }
        }
    }
}

fn wrapped_line_segments(line: &str, max_width: usize) -> Vec<&str> {
    if max_width == 0 || line.is_empty() {
        return vec![line];
    }

    let mut segments = Vec::new();
    let mut segment_start = 0usize;
    let mut segment_width = 0usize;

    for (idx, ch) in line.char_indices() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if segment_width + ch_width > max_width && idx > segment_start {
            segments.push(&line[segment_start..idx]);
            segment_start = idx;
            segment_width = 0;
        }
        segment_width += ch_width;
    }

    segments.push(&line[segment_start..]);
    segments
}

#[inline]
const fn starts_with_unordered_list_marker(trimmed: &str) -> bool {
    let bytes = trimmed.as_bytes();
    bytes.len() >= 2 && matches!(bytes[0], b'-' | b'+' | b'*') && bytes[1].is_ascii_whitespace()
}

#[inline]
const fn starts_with_ordered_list_marker(trimmed: &str) -> bool {
    let bytes = trimmed.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }

    idx > 0
        && idx <= 9
        && (idx + 1) < bytes.len()
        && matches!(bytes[idx], b'.' | b')')
        && bytes[idx + 1].is_ascii_whitespace()
}

#[inline]
fn is_repeated_marker_line(trimmed: &str, marker: u8) -> bool {
    let mut marker_count = 0usize;
    for byte in trimmed.bytes() {
        if byte == marker {
            marker_count += 1;
        } else if !byte.is_ascii_whitespace() {
            return false;
        }
    }
    marker_count >= 3
}

#[inline]
fn has_potential_underscore_emphasis(markdown: &str) -> bool {
    let bytes = markdown.as_bytes();
    for (idx, byte) in bytes.iter().enumerate() {
        if *byte != b'_' {
            continue;
        }
        let prev_alnum = idx
            .checked_sub(1)
            .and_then(|i| bytes.get(i))
            .is_some_and(u8::is_ascii_alphanumeric);
        let next_alnum = bytes.get(idx + 1).is_some_and(u8::is_ascii_alphanumeric);
        if !(prev_alnum && next_alnum) {
            return true;
        }
    }
    false
}

fn streaming_needs_markdown_renderer(markdown: &str) -> bool {
    // Inline syntax that can change visible formatting mid-stream.
    if markdown.as_bytes().iter().any(|byte| {
        matches!(
            *byte,
            b'`' | b'*' | b'[' | b']' | b'<' | b'>' | b'|' | b'!' | b'~' | b'\t'
        )
    }) {
        return true;
    }
    if has_potential_underscore_emphasis(markdown) {
        return true;
    }

    // Block-level syntax that only needs quick line-prefix checks.
    for line in markdown.lines() {
        if line.starts_with("    ") || parse_fence_line(line).is_some() {
            return true;
        }

        let trimmed = line.trim_start_matches(' ');
        let leading_spaces = line.len().saturating_sub(trimmed.len());
        if leading_spaces > 3 || trimmed.is_empty() {
            if leading_spaces > 3 {
                return true;
            }
            continue;
        }

        let first = trimmed.as_bytes()[0];
        if first == b'#'
            || first == b'>'
            || starts_with_unordered_list_marker(trimmed)
            || starts_with_ordered_list_marker(trimmed)
            || is_repeated_marker_line(trimmed, b'-')
            || is_repeated_marker_line(trimmed, b'*')
            || is_repeated_marker_line(trimmed, b'=')
        {
            return true;
        }
    }

    false
}

fn append_wrapped_plain_line_to_output(output: &mut String, line: &str, max_width: usize) {
    if max_width == 0 || line.is_empty() {
        let _ = writeln!(output, "  {line}");
        return;
    }

    let mut segment_start = 0usize;
    let mut segment_width = 0usize;
    for (idx, ch) in line.char_indices() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if segment_width + ch_width > max_width && idx > segment_start {
            let _ = writeln!(output, "  {}", &line[segment_start..idx]);
            segment_start = idx;
            segment_width = 0;
        }
        segment_width += ch_width;
    }

    let _ = writeln!(output, "  {}", &line[segment_start..]);
}

fn append_streaming_plaintext_to_output(output: &mut String, markdown: &str, max_width: usize) {
    for line in markdown.split_terminator('\n') {
        append_wrapped_plain_line_to_output(output, line, max_width);
    }
}

fn render_streaming_markdown_with_glamour(
    markdown: &str,
    markdown_style: &GlamourStyleConfig,
    max_width: usize,
) -> String {
    let stabilized_markdown = stabilize_streaming_markdown(markdown);
    glamour::Renderer::new()
        .with_style_config(markdown_style.clone())
        .with_word_wrap(max_width)
        .render(stabilized_markdown.as_ref())
}

fn parse_fence_line(line: &str) -> Option<(char, usize, &str)> {
    let trimmed_line = line.trim_end_matches(['\r', '\n']);
    let leading_spaces = trimmed_line.chars().take_while(|ch| *ch == ' ').count();
    if leading_spaces > 3 {
        return None;
    }

    let trimmed = trimmed_line.get(leading_spaces..)?;
    let marker = trimmed.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }

    let mut marker_len = 0usize;
    for ch in trimmed.chars() {
        if ch == marker {
            marker_len += 1;
        } else {
            break;
        }
    }

    if marker_len >= 3 {
        Some((
            marker,
            marker_len,
            trimmed.get(marker_len..).unwrap_or_default(),
        ))
    } else {
        None
    }
}

fn streaming_unclosed_fence(markdown: &str) -> Option<(char, usize)> {
    let mut open_fence: Option<(char, usize)> = None;

    for line in markdown.lines() {
        let Some((marker, marker_len, tail)) = parse_fence_line(line) else {
            continue;
        };

        if let Some((open_marker, open_len)) = open_fence {
            if marker == open_marker && marker_len >= open_len && tail.trim().is_empty() {
                open_fence = None;
            }
        } else {
            // CommonMark: backtick fence info strings may not contain backticks.
            if marker == '`' && tail.contains('`') {
                continue;
            }
            open_fence = Some((marker, marker_len));
        }
    }

    open_fence
}

fn stabilize_streaming_markdown(markdown: &str) -> std::borrow::Cow<'_, str> {
    let Some((marker, marker_len)) = streaming_unclosed_fence(markdown) else {
        return std::borrow::Cow::Borrowed(markdown);
    };

    // Close an unterminated fence in the transient streaming view so partial
    // markdown renders predictably while tokens are still arriving.
    let mut stabilized = String::with_capacity(markdown.len() + marker_len + 1);
    stabilized.push_str(markdown);
    if !stabilized.ends_with('\n') {
        stabilized.push('\n');
    }
    for _ in 0..marker_len {
        stabilized.push(marker);
    }
    std::borrow::Cow::Owned(stabilized)
}

fn format_path_label(path: &std::path::Path) -> String {
    let display = dirs::home_dir()
        .and_then(|home| {
            path.strip_prefix(&home)
                .ok()
                .map(|tail| (home, tail.to_path_buf()))
        })
        .map_or_else(
            || path.to_string_lossy().to_string(),
            |(_, tail)| {
                if tail.as_os_str().is_empty() {
                    "~".to_string()
                } else {
                    format!("~/{}", tail.to_string_lossy())
                }
            },
        );
    truncate(&display, 32)
}

const FOOTER_SEPARATOR: &str = "  |  ";

// Drop order at narrow widths, lowest first. Same whole-segment rule the header
// uses for its hint line: lose a field outright rather than truncate the tail
// and leave the reader half a word.
const FOOTER_PRIORITY_MODE_HINT: u8 = 0;
const FOOTER_PRIORITY_HELP: u8 = 1;
const FOOTER_PRIORITY_PERSISTENCE: u8 = 2;
const FOOTER_PRIORITY_BRANCH: u8 = 3;
const FOOTER_PRIORITY_TOKENS: u8 = 4;
const FOOTER_PRIORITY_QUIT: u8 = 5;
const FOOTER_PRIORITY_CWD: u8 = 6;
/// Only present while scrolled back, and the one thing that explains why the
/// conversation is not showing its tail.
const FOOTER_PRIORITY_SCROLL: u8 = 6;
/// Context outranks everything: it is the only field that warns the session is
/// about to run out of room, so it is never dropped.
const FOOTER_PRIORITY_CONTEXT: u8 = 7;

struct FooterSegment {
    long: String,
    /// Empty means the segment is absent from the compact form.
    short: String,
    priority: u8,
}

impl FooterSegment {
    fn pair(text: String, priority: u8) -> Self {
        Self {
            long: text.clone(),
            short: text,
            priority,
        }
    }
}

fn fit_footer(segments: &[FooterSegment], max_width: usize) -> String {
    let join = |live: &[&FooterSegment], compact: bool| {
        live.iter()
            .map(|seg| if compact { &seg.short } else { &seg.long })
            .filter(|text| !text.is_empty())
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(FOOTER_SEPARATOR)
    };

    // The long form only ever gives up a segment the compact form does not
    // carry at all, so full labels survive as long as they fit.
    let mut live: Vec<&FooterSegment> = segments.iter().collect();
    loop {
        let candidate = join(&live, false);
        if candidate.width() <= max_width {
            return candidate;
        }
        let Some(pos) = live.iter().position(|seg| seg.short.is_empty()) else {
            break;
        };
        live.remove(pos);
    }

    let mut live: Vec<&FooterSegment> = segments
        .iter()
        .filter(|seg| !seg.short.is_empty())
        .collect();
    loop {
        let candidate = join(&live, true);
        if candidate.width() <= max_width {
            return candidate;
        }
        let Some(pos) = live
            .iter()
            .enumerate()
            .filter(|(_, seg)| seg.priority < FOOTER_PRIORITY_CONTEXT)
            .min_by_key(|(_, seg)| seg.priority)
            .map(|(pos, _)| pos)
        else {
            return truncate(&candidate, max_width);
        };
        live.remove(pos);
    }
}

fn format_persistence_footer_segment(
    mode: crate::session::AutosaveDurabilityMode,
    metrics: crate::session::AutosaveQueueMetrics,
) -> String {
    let mut details = Vec::new();
    if metrics.pending_mutations > 0 {
        details.push(format!(
            "pending {}/{}",
            metrics.pending_mutations, metrics.max_pending_mutations
        ));
    }
    if metrics.flush_failed > 0 {
        details.push(format!("flush-fail {}", metrics.flush_failed));
    }
    if metrics.max_pending_mutations > 0
        && metrics.pending_mutations >= metrics.max_pending_mutations
    {
        details.push("backpressure".to_string());
    }

    if details.is_empty() {
        format!("Persist: {}", mode.as_str())
    } else {
        format!("Persist: {} ({})", mode.as_str(), details.join(", "))
    }
}

impl PiApp {
    fn header_binding_hint(&self, action: AppAction, fallback: &str) -> String {
        self.keybindings
            .get_bindings(action)
            .first()
            .map_or_else(|| fallback.to_string(), std::string::ToString::to_string)
    }

    fn render_startup_placeholder(&self) -> String {
        let mut output = String::new();
        let plain_width = self.term_width.saturating_sub(4).max(1);

        if !self.startup_welcome.trim().is_empty() {
            let width = box_width(self.term_width);
            let mut rows = Vec::new();
            for line in self.startup_welcome.trim_end().lines() {
                let line = line.trim_start();
                if line.is_empty() {
                    rows.push(String::new());
                    continue;
                }
                for segment in wrapped_line_segments(line, width.saturating_sub(4)) {
                    rows.push(self.styles.muted_italic.render(segment));
                }
            }
            rows.push(
                self.styles
                    .accent_bold
                    .render("Ask for a change, paste an error, or drag files here."),
            );
            for line in bordered_box(rows.iter().map(String::as_str), width, &self.styles.border) {
                let _ = writeln!(output, "  {line}");
            }
        }

        match &self.startup_changelog {
            Some(StartupChangelog::Condensed { latest_version }) => {
                if !output.is_empty() && !output.ends_with('\n') {
                    output.push('\n');
                }
                let message =
                    format!("Updated to v{latest_version}. Use /changelog to view full changelog.");
                for segment in wrapped_line_segments(&message, plain_width) {
                    let _ = writeln!(output, "  {}", self.styles.warning.render(segment));
                }
            }
            Some(StartupChangelog::Full { markdown }) => {
                if !output.is_empty() && !output.ends_with('\n') {
                    output.push('\n');
                }
                let _ = writeln!(output, "  {}", self.styles.accent_bold.render("What's New"));
                output.push('\n');
                let rendered = glamour::Renderer::new()
                    .with_style_config(self.markdown_style.clone())
                    .with_word_wrap(self.term_width.saturating_sub(6).max(40))
                    .render(markdown);
                for line in rendered.lines() {
                    let _ = writeln!(output, "  {line}");
                }
            }
            None => {}
        }

        output.trim_end().to_string()
    }

    /// Render the view.
    #[allow(clippy::too_many_lines)]
    pub(super) fn view(&self) -> String {
        let view_start = if self.frame_timing.enabled {
            Some(std::time::Instant::now())
        } else {
            None
        };

        // PERF-7: Pre-allocate view output with capacity from the previous
        // frame, avoiding incremental String grows during assembly.
        let mut output = String::with_capacity(self.render_buffers.view_capacity_hint());

        // Header — PERF-7: render directly into output, no intermediate String.
        self.render_header_into(&mut output);
        output.push('\n');
        let header_rows = block_rows(&output);

        // Modal overlays (e.g. /tree) take over the main view.
        if let Some(tree_ui) = &self.tree_ui {
            let tree = view_tree_ui(tree_ui, &self.styles);
            let body_rows = self.term_height.saturating_sub(header_rows + 1);
            output.push_str(&tree);
            pad_rows(&mut output, body_rows.saturating_sub(block_rows(&tree)));
            self.render_footer_into(&mut output, None);
            return output;
        }

        // Build conversation content for viewport.
        // Trim trailing whitespace so the viewport line count matches
        // what refresh_conversation_viewport() stored — this keeps the
        // y_offset from goto_bottom() aligned with the visible lines.
        let conversation_content = {
            let content_start = if self.frame_timing.enabled {
                Some(std::time::Instant::now())
            } else {
                None
            };
            let mut raw = self.build_conversation_content();
            if let Some(start) = content_start {
                self.frame_timing
                    .record_content_build(micros_as_u64(start.elapsed().as_micros()));
            }
            // PERF-7: Truncate in place instead of trim_end().to_string()
            // which would allocate a second copy of the entire content.
            let trimmed_len = raw.trim_end().len();
            raw.truncate(trimmed_len);
            raw
        };

        // Everything under the conversation is rendered first and measured,
        // so the conversation can be padded to fill exactly the rows left
        // over. Estimating that chrome instead let the input box float up
        // the screen whenever the estimate ran high.
        let bottom = self.render_below_conversation();

        // Render conversation area (scrollable), filling the gap between the
        // header and the measured bottom block. One row is reserved for the
        // footer, which is written last without a trailing newline.
        let effective_vp = self
            .term_height
            .saturating_sub(header_rows + block_rows(&bottom) + 1)
            .max(1);
        let mut scroll_percent = None;
        {
            // PERF-7: Use Cow to avoid consuming conversation_content so
            // the reusable buffer is always returned regardless of path.
            use std::borrow::Cow;
            let viewport_content: Cow<'_, str> = if conversation_content.is_empty() {
                Cow::Owned(self.render_startup_placeholder())
            } else {
                Cow::Borrowed(&conversation_content)
            };

            // PERF: Count total lines with memchr (O(n) byte scan, no alloc)
            // instead of collecting all lines into a Vec.  For a 10K-line
            // conversation this avoids a ~80KB Vec<&str> allocation per frame.
            let total_lines = memchr::memchr_iter(b'\n', viewport_content.as_bytes()).count() + 1;
            let start = if self.follow_stream_tail {
                total_lines.saturating_sub(effective_vp)
            } else {
                self.conversation_viewport
                    .y_offset()
                    .min(total_lines.saturating_sub(1))
            };
            let end = (start + effective_vp).min(total_lines);

            // Skip `start` lines, then take `end - start` lines — no Vec
            // allocation needed.
            for line in viewport_content.lines().skip(start).take(end - start) {
                output.push_str(line);
                output.push('\n');
            }
            pad_rows(&mut output, effective_vp.saturating_sub(end - start));

            // Scroll position moved into the footer: a dedicated row for it
            // cost one conversation line on every frame, scrolled or not.
            if total_lines > effective_vp {
                let total = total_lines.saturating_sub(effective_vp);
                scroll_percent = Some((start * 100).checked_div(total).map_or(100, |p| p.min(100)));
            }
        }
        // PERF-7: Return the conversation buffer for reuse next frame.
        // Always returned (even when empty) to preserve heap capacity.
        self.render_buffers
            .return_conversation_buffer(conversation_content);

        output.push_str(&bottom);

        // Footer with usage stats — PERF-7: render directly into output.
        self.render_footer_into(&mut output, scroll_percent);

        // Clamp the output to `term_height` rows so the terminal never
        // scrolls in the alternate-screen buffer.
        let output = clamp_to_terminal_height(output, self.term_height);
        let output = normalize_raw_terminal_newlines(output);

        // PERF-7: Remember this frame's output size so the next frame can
        // pre-allocate with the right capacity.
        self.render_buffers.set_view_capacity_hint(output.len());

        if let Some(start) = view_start {
            self.frame_timing
                .record_frame(micros_as_u64(start.elapsed().as_micros()));
            self.tui_pressure_frame_p99_us
                .store(self.frame_timing.frame_p99_us(), Ordering::Relaxed);
        }

        output
    }

    /// Status rows, overlays and the editor: everything between the
    /// conversation and the footer, rendered as one measurable block.
    #[allow(clippy::too_many_lines)]
    fn render_below_conversation(&self) -> String {
        let mut output = String::new();

        // Tool status
        if let Some(tool) = &self.current_tool {
            let progress_str = self.tool_progress.as_ref().map_or_else(String::new, |p| {
                let secs = p.elapsed_ms / 1000;
                if secs < 1 {
                    return String::new();
                }
                let mut parts = vec![format!("{secs}s")];
                if p.line_count > 0 {
                    parts.push(format!("{} lines", format_count(p.line_count)));
                } else if p.byte_count > 0 {
                    parts.push(format!("{} bytes", format_count(p.byte_count)));
                }
                if let Some(timeout_ms) = p.timeout_ms {
                    let timeout_s = timeout_ms / 1000;
                    if timeout_s > 0 {
                        parts.push(format!("timeout {timeout_s}s"));
                    }
                }
                format!(" ({})", parts.join(" \u{2022} "))
            });
            let _ = writeln!(
                output,
                "  {} {}{} ...",
                self.spinner.view(),
                self.styles.warning_bold.render(&format!("Running {tool}")),
                self.styles.muted.render(&progress_str),
            );
        }

        // Status message (slash command feedback)
        if let Some(status) = &self.status_message {
            let status_style = self.styles.accent.clone().italic();
            let _ = writeln!(output, "  {}", status_style.render(status));
        }

        // Session picker overlay (if open)
        if let Some(ref picker) = self.session_picker {
            output.push_str(&self.render_session_picker(picker));
        }

        // Settings overlay (if open)
        if let Some(ref settings_ui) = self.settings_ui {
            output.push_str(&self.render_settings_ui(settings_ui));
        }

        // Theme picker overlay (if open)
        if let Some(ref picker) = self.theme_picker {
            output.push_str(&self.render_theme_picker(picker));
        }

        // Capability prompt overlay (if open)
        if let Some(ref prompt) = self.capability_prompt {
            output.push_str(&self.render_capability_prompt(prompt));
        }

        // Tool approval overlay (if open)
        if let Some(ref prompt) = self.tool_approval {
            output.push_str(&self.render_tool_approval(prompt));
        }

        // Extension custom overlay (if open)
        if let Some(ref overlay) = self.extension_custom_overlay {
            output.push_str(&self.render_extension_custom_overlay(overlay));
        }

        // Branch picker overlay (if open)
        if let Some(ref picker) = self.branch_picker {
            output.push_str(&self.render_branch_picker(picker));
        }

        // Model selector overlay (if open)
        if let Some(ref selector) = self.model_selector {
            output.push_str(&self.render_model_selector(selector));
        }

        // Progress rows sit above the editor, which stays live through a turn.
        if self.agent_state != AgentState::Idle {
            if self.show_processing_status_spinner() {
                // Show spinner while waiting on provider/tool activity, before
                // we have visible streaming deltas.
                let _ = writeln!(
                    output,
                    "  {} {} {}",
                    self.spinner.view(),
                    self.styles.accent.render("Working"),
                    self.styles.muted.render(&self.render_progress_suffix()),
                );
            }

            if let Some(pending_queue) = self.render_pending_message_queue() {
                output.push_str(&pending_queue);
            }
        }

        // Input area (hidden only while an overlay owns the keyboard)
        if self.editor_input_is_available() {
            output.push_str(&self.render_input());

            // Autocomplete dropdown (if open)
            if self.autocomplete.open && !self.autocomplete.items.is_empty() {
                output.push_str(&self.render_autocomplete_dropdown());
            }
        }

        output
    }

    /// PERF-7: Render the header directly into `output`, avoiding an
    /// intermediate `String` allocation on the hot path.
    fn render_header_into(&self, output: &mut String) {
        let model_label = format!("({})", self.model);
        let cwd_label = format_path_label(&self.cwd);
        let mode_label = format!("mode: {}", self.permission_mode);

        // Branch indicator: show "Branch N/M" when session has multiple leaves.
        let branch_indicator = self
            .session
            .try_lock()
            .ok()
            .and_then(|guard| {
                let info = guard.branch_summary();
                if info.leaf_count <= 1 {
                    return None;
                }
                let current_idx = info
                    .current_leaf
                    .as_ref()
                    .and_then(|leaf| info.leaves.iter().position(|l| l == leaf))
                    .map_or(1, |i| i + 1);
                Some(format!(" [branch {current_idx}/{}]", info.leaf_count))
            })
            .unwrap_or_default();

        let model_key = self.header_binding_hint(AppAction::SelectModel, "ctrl+l");
        let tools_key = self.header_binding_hint(AppAction::ExpandTools, "ctrl+o");
        let mode_key = self.header_binding_hint(AppAction::CyclePermissionMode, "shift+tab");
        let max_width = self.term_width.saturating_sub(2);
        let title_line = truncate(
            &format!("{APP_LABEL} {model_label}  {cwd_label}{branch_indicator}  {mode_label}"),
            max_width,
        );

        // The four bindings a new user needs. Narrow terminals lose whole
        // hints from the left rather than a cut word on the right.
        let hints = [
            format!("{model_key}: model"),
            format!("{tools_key}: tools"),
            format!("{mode_key}: mode"),
            "/help".to_string(),
        ];
        let hints_line = fit_hints(&hints, max_width);

        let _ = writeln!(output, "  {}", self.styles.title.render(&title_line));
        if !self.header_hints_are_earned() {
            return;
        }
        let _ = writeln!(output, "  {}", self.styles.muted.render(&hints_line));
        if let Some(resources_line) = self.resources_line() {
            let _ = writeln!(
                output,
                "  {}",
                self.styles
                    .muted
                    .render(&truncate(&resources_line, max_width)),
            );
        }
    }

    /// Key hints and the resource inventory are onboarding, not state: they
    /// stay until the first message, then hand their rows to the conversation.
    fn header_hints_are_earned(&self) -> bool {
        self.messages.is_empty()
    }

    /// What the resource loader actually found, or nothing at all. A row of
    /// zeroes tells the user less than an absent row does.
    fn resources_line(&self) -> Option<String> {
        let counts = [
            (self.resources.skills().len(), "skills"),
            (self.resources.prompts().len(), "prompts"),
            (self.resources.themes().len(), "themes"),
            (self.resources.extensions().len(), "extensions"),
        ];
        let parts: Vec<String> = counts
            .iter()
            .filter(|(count, _)| *count > 0)
            .map(|(count, label)| format!("{count} {label}"))
            .collect();
        if parts.is_empty() {
            return None;
        }
        Some(parts.join("  "))
    }

    /// Rows `render_header_into` writes, including its trailing spacer.
    pub(super) fn header_rows(&self) -> usize {
        if !self.header_hints_are_earned() {
            return 2;
        }
        if self.resources_line().is_some() {
            4
        } else {
            3
        }
    }

    #[cfg(test)]
    pub(super) fn render_header(&self) -> String {
        let mut buf = String::new();
        self.render_header_into(&mut buf);
        buf
    }

    fn render_progress_suffix(&self) -> String {
        progress_suffix(
            self.busy_since.map(|started| started.elapsed()),
            self.total_usage.input + self.total_usage.output,
        )
    }

    pub(super) fn render_input(&self) -> String {
        let mut output = String::new();

        let thinking_level = self
            .session
            .try_lock()
            .ok()
            .and_then(|guard| guard.header.thinking_level.clone())
            .and_then(|level| level.parse::<ThinkingLevel>().ok())
            .or_else(|| {
                self.config
                    .default_thinking_level
                    .as_deref()
                    .and_then(|level| level.parse::<ThinkingLevel>().ok())
            })
            .unwrap_or(ThinkingLevel::Off);

        let input_text = self.input.value();
        let is_bash_mode = parse_bash_command(&input_text).is_some();

        let (thinking_label, thinking_border_style) = match thinking_level {
            ThinkingLevel::Off => ("off", self.styles.border.clone()),
            ThinkingLevel::Minimal => ("minimal", self.styles.accent.clone()),
            ThinkingLevel::Low => ("low", self.styles.accent.clone()),
            ThinkingLevel::Medium => ("medium", self.styles.accent.clone()),
            ThinkingLevel::High => ("high", self.styles.warning.clone()),
            ThinkingLevel::XHigh => ("xhigh", self.styles.error_bold.clone()),
            ThinkingLevel::Max => ("max", self.styles.error_bold.clone()),
        };

        let border_style = if is_bash_mode {
            self.styles.warning_bold.clone()
        } else {
            thinking_border_style
        };

        let mode_text = if self.term_width >= 76 {
            "Enter send · \\+Enter newline · @file context"
        } else {
            "Enter send · \\+Enter newline"
        };
        let width = box_width(self.term_width);
        let middle_width = width.saturating_sub(2);

        let mut segments: Vec<&str> = Vec::new();
        if is_bash_mode {
            segments.push("bash");
        }
        segments.extend(self.permission_mode.indicator());
        let thinking_segment = format!("think {thinking_label}");
        segments.push(&thinking_segment);
        segments.push(mode_text);

        let mut label = String::new();
        for segment in segments {
            let candidate = if label.is_empty() {
                format!(" {segment} ")
            } else {
                format!("{label}· {segment} ")
            };
            if lipgloss::width(&candidate) <= middle_width {
                label = candidate;
            }
        }
        if lipgloss::width(&label) > middle_width {
            label = truncate(&label, middle_width);
        }
        let label_width = lipgloss::width(&label).min(middle_width);
        let top_middle = format!("{label}{}", "─".repeat(middle_width - label_width));
        let rule = "─".repeat(width.saturating_sub(2));
        let _ = writeln!(
            output,
            "  {}",
            border_style.render(&format!("╭{top_middle}╮"))
        );

        let left = border_style.render("│");
        let right = border_style.render("│");
        let padding = " ".repeat(self.editor_padding_x);
        let prompt = self.styles.accent_bold.render(INPUT_PROMPT_GLYPH);
        for (idx, line) in self.editor_rows().into_iter().enumerate() {
            let row = if idx == 0 {
                format!("{padding}{prompt} {line}")
            } else {
                format!("{padding}  {line}")
            };
            let row = fit_to_width(&row, width.saturating_sub(4));
            let _ = writeln!(output, "  {left} {row} {right}");
        }
        let _ = writeln!(output, "  {}", border_style.render(&format!("╰{rule}╯")));

        output
    }

    /// The editor's visible rows, wrapped to the box and scrolled so the
    /// cursor is always on one of them.
    fn editor_rows(&self) -> Vec<String> {
        let value = self.input.value();
        let width = editor_width(self.term_width, self.editor_padding_x);
        if value.is_empty() && !self.input.placeholder.is_empty() {
            return vec![
                self.styles
                    .muted
                    .render(&truncate(&self.input.placeholder, width)),
            ];
        }

        let layout = layout_input(&value, self.input.cursor_byte_offset(), width);
        let visible = self.input.height().clamp(1, layout.rows.len());
        let offset = layout
            .cursor_row
            .saturating_sub(visible - 1)
            .min(layout.rows.len() - visible);
        let focused = self.input.focused();

        layout.rows[offset..offset + visible]
            .iter()
            .enumerate()
            .map(|(idx, row)| {
                let text = &value[row.clone()];
                if !focused || offset + idx != layout.cursor_row {
                    return text.to_string();
                }
                let split = input_byte_at_column(&value, row, layout.cursor_col) - row.start;
                let (before, rest) = text.split_at(split);
                let under = rest.chars().next();
                let mut cursor = self.input.cursor.clone();
                cursor.set_char(&under.map_or_else(|| " ".to_string(), |ch| ch.to_string()));
                let after = under.map_or("", |ch| &rest[ch.len_utf8()..]);
                format!("{before}{}{after}", cursor.view())
            })
            .collect()
    }

    /// Percent of the active model's context window still free.
    fn context_left_percent(&self) -> Option<u64> {
        context_left_percent(
            self.model_entry.model.context_window,
            self.session_context_tokens(),
        )
    }

    /// Tokens the session currently occupies, from the newest assistant turn
    /// that reported usage. `0` means no turn has reported any yet.
    fn session_context_tokens(&self) -> u64 {
        let key = (self.messages.len(), self.total_usage.total_tokens);
        if let Some((cached_key, tokens)) = self.context_tokens_cache.get()
            && cached_key == key
        {
            return tokens;
        }

        let Ok(session) = self.session.try_lock() else {
            return 0;
        };
        let tokens = session
            .entries_for_current_path()
            .iter()
            .rev()
            .find_map(|entry| {
                let SessionEntry::Message(entry) = entry else {
                    return None;
                };
                let SessionMessage::Assistant { message } = &entry.message else {
                    return None;
                };
                let usage = &message.usage;
                let tokens = if usage.total_tokens > 0 {
                    usage.total_tokens
                } else {
                    usage.input.saturating_add(usage.output)
                };
                (tokens > 0).then_some(tokens)
            })
            .unwrap_or(0);
        self.context_tokens_cache.set(Some((key, tokens)));
        tokens
    }

    /// PERF-7: Render the footer directly into `output`, avoiding an
    /// intermediate `String` allocation on the hot path.
    fn render_footer_into(&self, output: &mut String, scroll_percent: Option<usize>) {
        let total_cost = self.total_usage.cost.total;
        let cost_str = if total_cost > 0.0 {
            format!(" (${total_cost:.4})")
        } else {
            String::new()
        };

        let input = self.total_usage.input;
        let output_tokens = self.total_usage.output;
        let persistence_str = self.session.try_lock().ok().map_or_else(
            || "Persist: unavailable".to_string(),
            |session| {
                format_persistence_footer_segment(
                    session.autosave_durability_mode(),
                    session.autosave_metrics(),
                )
            },
        );
        let branch_str = self.vcs_info.clone();
        let percent = self.context_left_percent();

        // No cwd here: the header already prints it. No key hints either: the
        // input box border carries the ones that apply to what you are typing.
        let mut segments = Vec::new();
        if let Some(percent) = scroll_percent {
            segments.push(FooterSegment {
                long: format!("Scroll {percent}% (PgUp/PgDn)"),
                short: format!("Scroll {percent}%"),
                priority: FOOTER_PRIORITY_SCROLL,
            });
        }
        if let Some(branch) = branch_str {
            segments.push(FooterSegment::pair(branch, FOOTER_PRIORITY_BRANCH));
        }
        segments.push(FooterSegment {
            long: format!("Tokens: {input} in / {output_tokens} out{cost_str}"),
            short: format!("{input}/{output_tokens} tokens{cost_str}"),
            priority: FOOTER_PRIORITY_TOKENS,
        });
        if let Some(percent) = percent {
            segments.push(FooterSegment {
                long: format!("Context: {percent}% left"),
                short: format!("{percent}% ctx"),
                priority: FOOTER_PRIORITY_CONTEXT,
            });
        }
        segments.push(FooterSegment::pair(
            persistence_str,
            FOOTER_PRIORITY_PERSISTENCE,
        ));
        segments.push(FooterSegment::pair(
            "Ctrl+C quit".to_string(),
            FOOTER_PRIORITY_QUIT,
        ));

        let footer = fit_footer(&segments, self.term_width.saturating_sub(2));
        // Last row of the frame: no trailing newline, or the terminal scrolls.
        let _ = write!(output, "  {}", self.styles.muted.render(&footer));
    }

    /// Render a single conversation message to a string (uncached path).
    fn render_single_message(&self, msg: &ConversationMessage) -> String {
        let mut output = String::new();
        match msg.role {
            MessageRole::User => {
                let _ = writeln!(output);
                for line in msg.content.lines() {
                    let _ = writeln!(
                        output,
                        "  {} {line}",
                        self.styles.accent_bold.render(USER_PROMPT_GLYPH)
                    );
                }
            }
            MessageRole::Assistant => {
                let _ = writeln!(output);

                // Render thinking if present
                if self.thinking_visible
                    && let Some(thinking) = &msg.thinking
                {
                    self.render_thinking_into(thinking, &mut output);
                }

                // Render markdown content
                let rendered = glamour::Renderer::new()
                    .with_style_config(self.markdown_style.clone())
                    .with_word_wrap(self.term_width.saturating_sub(6).max(40))
                    .render(&msg.content);
                for line in rendered.lines() {
                    let _ = writeln!(output, "    {line}");
                }
            }
            MessageRole::Tool => {
                // Per-message collapse: global toggle overrides, then per-message.
                let show_expanded = self.tools_expanded && !msg.collapsed;
                if show_expanded {
                    let rendered = render_tool_message(&msg.content, &self.styles);
                    let gutter = self.styles.muted.render(TOOL_RESULT_GLYPH);
                    let _ = writeln!(output);
                    for (idx, line) in rendered.lines().enumerate() {
                        if idx == 0 {
                            let _ = writeln!(output, "  {gutter} {line}");
                        } else {
                            let _ = writeln!(output, "    {line}");
                        }
                    }
                } else {
                    let header = msg.content.lines().next().unwrap_or("Tool output");
                    let line_count = memchr::memchr_iter(b'\n', msg.content.as_bytes()).count() + 1;
                    let summary = format!(
                        "{TOOL_RESULT_GLYPH} {} (+{line_count} lines, ctrl+o to expand)",
                        header.trim_end()
                    );
                    let _ = write!(
                        output,
                        "\n  {}\n",
                        self.styles.muted_italic.render(&summary)
                    );
                    // Show preview when per-message collapsed (not global).
                    if self.tools_expanded && msg.collapsed {
                        for (i, line) in msg.content.lines().skip(1).enumerate() {
                            if i >= TOOL_COLLAPSE_PREVIEW_LINES {
                                let remaining = line_count
                                    .saturating_sub(1)
                                    .saturating_sub(TOOL_COLLAPSE_PREVIEW_LINES);
                                let _ = writeln!(
                                    output,
                                    "  {}",
                                    self.styles
                                        .muted
                                        .render(&format!("  ... {remaining} more lines"))
                                );
                                break;
                            }
                            let _ = writeln!(
                                output,
                                "  {}",
                                self.styles.muted.render(&format!("  {line}"))
                            );
                        }
                    }
                }
            }
            MessageRole::System => {
                let _ = write!(output, "\n  {}\n", self.styles.warning.render(&msg.content));
            }
        }
        output
    }

    /// Build the conversation content string for the viewport.
    ///
    /// Uses `MessageRenderCache` (PERF-1) to avoid re-rendering unchanged
    /// messages and a conversation prefix cache (PERF-2) to skip iterating
    /// all messages during streaming. Streaming content (current_response)
    /// always renders fresh.
    pub fn build_conversation_content(&self) -> String {
        let has_streaming_state =
            !self.current_response.is_empty() || !self.current_thinking.is_empty();
        let has_visible_streaming_tail = !self.current_response.is_empty()
            || (self.thinking_visible && !self.current_thinking.is_empty());

        // PERF-7: Reuse the pre-allocated conversation buffer from the
        // previous frame. `take_conversation_buffer()` clears the buffer
        // but preserves its heap capacity, avoiding a fresh allocation.
        let mut output = self.render_buffers.take_conversation_buffer();

        // PERF-2 fast path: during streaming, reuse the cached prefix
        // (all finalized messages) and only rebuild the streaming tail.
        if has_streaming_state && self.message_render_cache.prefix_valid(self.messages.len()) {
            // PERF-7: Append prefix directly into the reusable buffer
            // instead of cloning via prefix_get().
            self.message_render_cache.prefix_append_to(&mut output);
            if has_visible_streaming_tail {
                self.append_streaming_tail(&mut output);
            }
            return output;
        }

        // Full rebuild: iterate all messages with per-message cache (PERF-1).
        for (index, msg) in self.messages.iter().enumerate() {
            let key =
                MessageRenderCache::compute_key(msg, self.thinking_visible, self.tools_expanded);

            if self
                .message_render_cache
                .append_cached(&mut output, index, &key)
            {
                continue;
            }
            let rendered = self.render_single_message(msg);
            // PERF: push_str first, then move into cache — avoids cloning
            // the rendered String (which can be several KB for tool output).
            output.push_str(&rendered);
            self.message_render_cache.put(index, key, rendered);
        }

        // Snapshot the prefix for future streaming frames (PERF-2).
        self.message_render_cache
            .prefix_set(&output, self.messages.len());

        // Append streaming content if active.
        if has_visible_streaming_tail {
            self.append_streaming_tail(&mut output);
        }

        output
    }

    /// Render a thinking block in full, word-wrapped to the viewport.
    ///
    /// Long blocks collapse to [`THINKING_COLLAPSED_MAX_LINES`] rows behind the
    /// same `ctrl+o to expand` affordance tool output uses, so a verbose model
    /// cannot swamp the transcript.
    fn render_thinking_into(&self, thinking: &str, output: &mut String) {
        let content_width = self.term_width.saturating_sub(4).max(20);
        let labelled = format!("Thinking: {}", plain_thinking(thinking.trim_end()));
        let mut lines = Vec::new();
        for line in labelled.lines() {
            if line.trim().is_empty() {
                lines.push(String::new());
                continue;
            }
            for segment in textwrap::wrap(line, content_width) {
                lines.push(segment.into_owned());
            }
        }

        let hidden = if self.tools_expanded {
            0
        } else {
            lines.len().saturating_sub(THINKING_COLLAPSED_MAX_LINES)
        };
        for line in lines.iter().take(lines.len() - hidden) {
            let _ = writeln!(output, "  {}", self.styles.muted_italic.render(line));
        }
        if hidden > 0 {
            let _ = writeln!(
                output,
                "  {}",
                self.styles.muted_italic.render(&format!(
                    "{TOOL_RESULT_GLYPH} ... (+{hidden} lines, ctrl+o to expand)"
                ))
            );
        }
    }

    /// Render the current streaming response / thinking into `output`.
    /// Always renders fresh — never cached.
    fn append_streaming_tail(&self, output: &mut String) {
        let _ = write!(
            output,
            "\n  {}\n",
            self.styles.success_bold.render("Assistant:")
        );

        // Show thinking if present
        if self.thinking_visible && !self.current_thinking.is_empty() {
            self.render_thinking_into(&self.current_thinking, output);
        }

        // Render partial markdown on every stream update so headings/lists/code
        // format as they arrive instead of showing raw markers.
        if !self.current_response.is_empty() {
            let markdown_width = self.term_width.saturating_sub(6).max(40);
            if streaming_needs_markdown_renderer(&self.current_response) {
                let rendered = render_streaming_markdown_with_glamour(
                    &self.current_response,
                    &self.markdown_style,
                    markdown_width,
                );
                for line in rendered.lines() {
                    let _ = writeln!(output, "  {line}");
                }
            } else {
                append_streaming_plaintext_to_output(
                    output,
                    &self.current_response,
                    markdown_width,
                );
            }
        }
    }

    pub(super) fn render_pending_message_queue(&self) -> Option<String> {
        if self.agent_state == AgentState::Idle {
            return None;
        }

        let Ok(queue) = self.message_queue.lock() else {
            return None;
        };

        let steering_len = queue.steering_len();
        let follow_len = queue.follow_up_len();
        if steering_len == 0 && follow_len == 0 {
            return None;
        }

        let max_preview = self.term_width.saturating_sub(24).max(20);

        let mut out = String::new();
        out.push_str("  ");
        out.push_str(&self.styles.muted_bold.render("Pending:"));
        out.push(' ');
        out.push_str(
            &self
                .styles
                .accent_bold
                .render(&format!("{steering_len} steering")),
        );
        out.push_str(&self.styles.muted.render(", "));
        out.push_str(&self.styles.muted.render(&format!("{follow_len} follow-up")));
        out.push('\n');

        if let Some(text) = queue.steering_front() {
            let preview = queued_message_preview(text, max_preview);
            out.push_str("  ");
            out.push_str(&self.styles.accent_bold.render("steering →"));
            out.push(' ');
            out.push_str(&preview);
            out.push('\n');
        }

        if let Some(text) = queue.follow_up_front() {
            let preview = queued_message_preview(text, max_preview);
            out.push_str("  ");
            out.push_str(&self.styles.muted_bold.render("follow-up →"));
            out.push(' ');
            out.push_str(&self.styles.muted.render(&preview));
            out.push('\n');
        }

        Some(out)
    }

    pub(super) fn render_autocomplete_dropdown(&self) -> String {
        let mut output = String::new();

        let offset = self.autocomplete.scroll_offset();
        // Constrain visible items to available terminal space.
        // Dropdown chrome uses ~5 rows (borders, help, pagination, description).
        let max_dropdown_rows = self.term_height.saturating_sub(
            // header + min conversation(1) + scroll indicator(1)
            // + input(4 + height) + footer(2) + dropdown chrome(5)
            self.header_rows() + 1 + 1 + 4 + self.input.height() + 2 + 5,
        );
        let visible_count = self
            .autocomplete
            .max_visible
            .min(self.autocomplete.items.len())
            .min(max_dropdown_rows.max(1));
        let end = (offset + visible_count).min(self.autocomplete.items.len());

        let selected_style = &self.styles.selection;
        let kind_style = &self.styles.warning;
        let desc_style = &self.styles.muted_italic;

        // Aligns under the input box, never wider than it.
        let width = box_width(self.term_width).min(DROPDOWN_MAX_WIDTH);
        let inner = width.saturating_sub(4);

        let mut rows = Vec::with_capacity(visible_count + 2);
        for (idx, item) in self.autocomplete.items[offset..end].iter().enumerate() {
            let global_idx = offset + idx;
            let is_selected = self.autocomplete.selected == Some(global_idx);

            let kind_icon = match item.kind {
                AutocompleteItemKind::SlashCommand => "⚡",
                AutocompleteItemKind::ExtensionCommand => "🧩",
                AutocompleteItemKind::PromptTemplate => "📄",
                AutocompleteItemKind::Skill => "🔧",
                AutocompleteItemKind::Model => "🤖",
                AutocompleteItemKind::File => "📁",
                AutocompleteItemKind::Path => "📂",
            };

            let plain = fit_to_width(&format!("{kind_icon} {}", item.label), inner);
            rows.push(match plain.strip_prefix(kind_icon) {
                Some(rest) if !is_selected => format!("{}{rest}", kind_style.render(kind_icon)),
                _ if is_selected => selected_style.render(&plain),
                _ => plain,
            });

            if is_selected && let Some(desc) = &item.description {
                let desc = fit_to_width(&format!("  {}", desc.replace('\n', " ")), inner);
                rows.push(desc_style.render(&desc));
            }
        }

        if self.autocomplete.items.len() > visible_count {
            let shown = format!(
                "{}-{} of {}",
                offset + 1,
                end,
                self.autocomplete.items.len()
            );
            rows.push(format!("{shown:^inner$}"));
        }

        output.push('\n');
        for line in bordered_box(rows.iter().map(String::as_str), width, &self.styles.border) {
            let _ = writeln!(output, "  {line}");
        }

        let _ = write!(
            output,
            "  {}",
            self.styles
                .muted_italic
                .render("↑/↓ navigate  Enter/Tab accept  Esc cancel")
        );

        output
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn render_session_picker(&self, picker: &SessionPickerOverlay) -> String {
        let mut output = String::new();

        let _ = writeln!(
            output,
            "\n  {}\n",
            self.styles.title.render("Select a session to resume")
        );

        let query = picker.query();
        let search_line = if query.is_empty() {
            "  > (type to filter sessions)".to_string()
        } else {
            format!("  > {query}")
        };
        let _ = writeln!(output, "{}", self.styles.muted.render(&search_line));
        let _ = writeln!(
            output,
            "  {}",
            self.styles.muted.render("─".repeat(50).as_str())
        );
        output.push('\n');

        if picker.sessions.is_empty() {
            let message = if picker.has_query() {
                "No sessions match the current filter."
            } else {
                "No sessions found for this project."
            };
            let _ = writeln!(output, "  {}", self.styles.muted.render(message));
        } else {
            let _ = writeln!(
                output,
                "  {:<20}  {:<30}  {:<8}  {}",
                self.styles.muted_bold.render("Time"),
                self.styles.muted_bold.render("Name"),
                self.styles.muted_bold.render("Messages"),
                self.styles.muted_bold.render("Session ID")
            );
            output.push_str("  ");
            output.push_str(&"-".repeat(78));
            output.push('\n');

            let offset = picker.scroll_offset();
            let visible_count = picker.max_visible.min(picker.sessions.len());
            let end = (offset + visible_count).min(picker.sessions.len());

            for (idx, session) in picker.sessions[offset..end].iter().enumerate() {
                let global_idx = offset + idx;
                let is_selected = global_idx == picker.selected;

                let prefix = if is_selected { ">" } else { " " };
                let time = crate::session_picker::format_time(&session.timestamp);
                let name = session
                    .name
                    .as_deref()
                    .unwrap_or("-")
                    .chars()
                    .take(28)
                    .collect::<String>();
                let messages = session.message_count.to_string();
                let id = crate::session_picker::truncate_session_id(&session.id, 8);

                let row = format!(" {time:<20}  {name:<30}  {messages:<8}  {id}");
                let rendered = if is_selected {
                    self.styles.selection.render(&row)
                } else {
                    row
                };

                let _ = writeln!(output, "{prefix} {rendered}");
            }

            if picker.sessions.len() > visible_count {
                let _ = writeln!(
                    output,
                    "  {}",
                    self.styles.muted.render(&format!(
                        "({}-{} of {})",
                        offset + 1,
                        end,
                        picker.sessions.len()
                    ))
                );
            }
        }

        output.push('\n');
        if picker.confirm_delete {
            let _ = writeln!(
                output,
                "  {}",
                self.styles.warning_bold.render(
                    picker
                        .status_message
                        .as_deref()
                        .unwrap_or("Delete session? Press y/n to confirm."),
                )
            );
        } else {
            let _ = writeln!(
                output,
                "  {}",
                self.styles.muted_italic.render(
                    "Type: filter  Backspace: clear  ↑/↓/j/k/PgUp/PgDn: navigate  Enter: select  Ctrl+D: delete  Esc/q: cancel",
                )
            );
            if let Some(message) = &picker.status_message {
                let _ = writeln!(output, "  {}", self.styles.warning_bold.render(message));
            }
        }

        output
    }

    pub(super) fn render_settings_ui(&self, settings_ui: &SettingsUiState) -> String {
        let mut output = String::new();

        let _ = writeln!(output, "\n  {}\n", self.styles.title.render("Settings"));

        if settings_ui.entries.is_empty() {
            let _ = writeln!(
                output,
                "  {}",
                self.styles.muted.render("No settings available.")
            );
        } else {
            let offset = settings_ui.scroll_offset();
            let visible_count = settings_ui.max_visible.min(settings_ui.entries.len());
            let end = (offset + visible_count).min(settings_ui.entries.len());

            for (idx, entry) in settings_ui.entries[offset..end].iter().enumerate() {
                let global_idx = offset + idx;
                let is_selected = global_idx == settings_ui.selected;

                let prefix = if is_selected { ">" } else { " " };
                let label = match *entry {
                    SettingsUiEntry::Summary => "Summary".to_string(),
                    SettingsUiEntry::Theme => "Theme".to_string(),
                    SettingsUiEntry::SteeringMode => format!(
                        "steeringMode: {}",
                        self.config.steering_queue_mode().as_str()
                    ),
                    SettingsUiEntry::FollowUpMode => format!(
                        "followUpMode: {}",
                        self.config.follow_up_queue_mode().as_str()
                    ),
                    SettingsUiEntry::DefaultPermissive => format!(
                        "extensionPolicy.defaultPermissive: {}{}",
                        bool_label(self.effective_default_permissive()),
                        if self.default_permissive_changes_require_extension_restart() {
                            " (restart required)"
                        } else {
                            ""
                        }
                    ),
                    SettingsUiEntry::QuietStartup => format!(
                        "quietStartup: {}",
                        bool_label(self.config.quiet_startup.unwrap_or(false))
                    ),
                    SettingsUiEntry::CollapseChangelog => format!(
                        "collapseChangelog: {}",
                        bool_label(self.config.collapse_changelog.unwrap_or(false))
                    ),
                    SettingsUiEntry::HideThinkingBlock => format!(
                        "hideThinkingBlock: {}",
                        bool_label(self.config.hide_thinking_block.unwrap_or(false))
                    ),
                    SettingsUiEntry::ShowHardwareCursor => format!(
                        "showHardwareCursor: {}",
                        bool_label(self.effective_show_hardware_cursor())
                    ),
                    SettingsUiEntry::DoubleEscapeAction => format!(
                        "doubleEscapeAction: {}",
                        self.config
                            .double_escape_action
                            .as_deref()
                            .unwrap_or("tree")
                    ),
                    SettingsUiEntry::EditorPaddingX => {
                        format!("editorPaddingX: {}", self.editor_padding_x)
                    }
                    SettingsUiEntry::AutocompleteMaxVisible => {
                        format!("autocompleteMaxVisible: {}", self.autocomplete.max_visible)
                    }
                };
                let row = format!(" {label}");
                let rendered = if is_selected {
                    self.styles.selection.render(&row)
                } else {
                    row
                };

                let _ = writeln!(output, "{prefix} {rendered}");
            }

            if settings_ui.entries.len() > visible_count {
                let _ = writeln!(
                    output,
                    "  {}",
                    self.styles.muted.render(&format!(
                        "({}-{} of {})",
                        offset + 1,
                        end,
                        settings_ui.entries.len()
                    ))
                );
            }
        }

        output.push('\n');
        let _ = writeln!(
            output,
            "  {}",
            self.styles
                .muted_italic
                .render("↑/↓/j/k/PgUp/PgDn: navigate  Enter: select  Esc/q: cancel")
        );

        output
    }

    pub(super) fn render_theme_picker(&self, picker: &ThemePickerOverlay) -> String {
        let mut output = String::new();

        let _ = writeln!(output, "\n  {}\n", self.styles.title.render("Select Theme"));

        if picker.items.is_empty() {
            let _ = writeln!(output, "  {}", self.styles.muted.render("No themes found."));
        } else {
            let offset = picker.scroll_offset();
            let visible_count = picker.max_visible.min(picker.items.len());
            let end = (offset + visible_count).min(picker.items.len());

            for (idx, item) in picker.items[offset..end].iter().enumerate() {
                let global_idx = offset + idx;
                let is_selected = global_idx == picker.selected;

                let prefix = if is_selected { ">" } else { " " };
                let (name, label) = match item {
                    ThemePickerItem::BuiltIn(name) => {
                        (name.to_string(), format!("{name} (built-in)"))
                    }
                    ThemePickerItem::File { name, .. } => {
                        (name.clone(), format!("{name} (custom)"))
                    }
                };

                let active = name.eq_ignore_ascii_case(&self.theme.name);
                let marker = if active { " *" } else { "" };

                let row = format!(" {label}{marker}");
                let rendered = if is_selected {
                    self.styles.selection.render(&row)
                } else {
                    row
                };

                let _ = writeln!(output, "{prefix} {rendered}");
            }

            if picker.items.len() > visible_count {
                let _ = writeln!(
                    output,
                    "  {}",
                    self.styles.muted.render(&format!(
                        "({}-{} of {})",
                        offset + 1,
                        end,
                        picker.items.len()
                    ))
                );
            }
        }

        output.push('\n');
        let _ = writeln!(
            output,
            "  {}",
            self.styles
                .muted_italic
                .render("↑/↓/j/k/PgUp/PgDn: navigate  Enter: select  Esc/q: back")
        );

        output
    }

    pub(super) fn render_capability_prompt(&self, prompt: &CapabilityPromptOverlay) -> String {
        let mut output = String::new();

        // Title line.
        let _ = writeln!(
            output,
            "\n  {}",
            self.styles.title.render("Extension Permission Request")
        );

        // Extension and capability info.
        let _ = writeln!(
            output,
            "  {} requests {}",
            self.styles.accent_bold.render(&prompt.extension_id),
            self.styles.warning_bold.render(&prompt.capability),
        );

        // Description.
        if !prompt.description.is_empty() {
            let _ = writeln!(
                output,
                "\n  {}",
                self.styles.muted.render(&prompt.description),
            );
        }

        // Button row.
        output.push('\n');
        output.push_str("  ");
        for (idx, action) in CapabilityAction::ALL.iter().enumerate() {
            let label = action.label();
            let rendered = if idx == prompt.focused {
                self.styles.selection.render(&format!("[{label}]"))
            } else {
                self.styles.muted.render(&format!(" {label} "))
            };
            output.push_str(&rendered);
            output.push_str("  ");
        }
        output.push('\n');

        // Auto-deny timer.
        if let Some(secs) = prompt.auto_deny_secs {
            let _ = writeln!(
                output,
                "  {}",
                self.styles
                    .muted_italic
                    .render(&format!("Auto-deny in {secs}s")),
            );
        }

        // Help text.
        let _ = writeln!(
            output,
            "  {}",
            self.styles
                .muted_italic
                .render("←/→/Tab: navigate  Enter: confirm  Esc: deny")
        );

        output
    }

    pub(super) fn render_tool_approval(&self, prompt: &ToolApprovalOverlay) -> String {
        let width = box_width(self.term_width);
        let title = format!("{} wants to run", self.styles.accent_bold.render("KESA"));
        let summary = self.styles.warning_bold.render(&prompt.summary);
        let tool = self.styles.muted.render(&format!("{}:", prompt.tool_name));

        let mut rows = vec![title, String::new(), tool, summary, String::new()];
        for (idx, action) in ApprovalAction::ALL.iter().enumerate() {
            let label = format!("{}. {}", idx + 1, prompt.label(*action));
            rows.push(if idx == prompt.focused {
                self.styles.selection.render(&format!("\u{276f} {label}"))
            } else {
                self.styles.muted.render(&format!("  {label}"))
            });
        }

        let mut output = String::from("\n");
        for line in bordered_box(rows.iter().map(String::as_str), width, &self.styles.border) {
            let _ = writeln!(output, "  {line}");
        }
        let _ = writeln!(
            output,
            "  {}",
            self.styles
                .muted_italic
                .render("1-3 choose  \u{2191}/\u{2193} navigate  Enter confirm  Esc reject")
        );
        output
    }

    pub(super) fn render_extension_custom_overlay(
        &self,
        overlay: &ExtensionCustomOverlay,
    ) -> String {
        let mut output = String::new();
        let title = overlay.title.as_deref().unwrap_or("Extension Overlay");
        let source = overlay.extension_id.as_deref().unwrap_or("extension");

        let _ = writeln!(output, "\n  {}", self.styles.title.render(title));
        let _ = writeln!(
            output,
            "  {}",
            self.styles
                .muted
                .render(&format!("[{source}] custom UI active"))
        );

        let max_lines = self.term_height.saturating_sub(12).max(4);
        if overlay.lines.is_empty() {
            let _ = writeln!(
                output,
                "  {}",
                self.styles
                    .muted_italic
                    .render("Waiting for extension frame...")
            );
        } else {
            for line in overlay
                .lines
                .iter()
                .skip(overlay.lines.len().saturating_sub(max_lines))
            {
                let _ = writeln!(output, "  {line}");
            }
        }
        let _ = writeln!(
            output,
            "  {}",
            self.styles
                .muted_italic
                .render("Press q to exit extension overlays that support quit")
        );

        output
    }

    pub(super) fn render_branch_picker(&self, picker: &BranchPickerOverlay) -> String {
        let mut output = String::new();

        let _ = writeln!(
            output,
            "\n  {}",
            self.styles.title.render("Select a branch")
        );
        let _ = writeln!(
            output,
            "  {}",
            self.styles
                .muted
                .render("-------------------------------------------")
        );

        if picker.branches.is_empty() {
            let _ = writeln!(
                output,
                "  {}",
                self.styles.muted_italic.render("No branches found.")
            );
        } else {
            let offset = picker.scroll_offset();
            let visible_count = picker.max_visible.min(picker.branches.len());
            let end = (offset + visible_count).min(picker.branches.len());

            for (idx, branch) in picker.branches[offset..end].iter().enumerate() {
                let global_idx = offset + idx;
                let is_selected = global_idx == picker.selected;
                let prefix = if is_selected { ">" } else { " " };

                let current_marker = if branch.is_current { " *" } else { "" };
                let msg_count = format!("({} msgs)", branch.message_count);
                let preview = if branch.preview.chars().count() > 40 {
                    let truncated: String = branch.preview.chars().take(37).collect();
                    format!("{truncated}...")
                } else {
                    branch.preview.clone()
                };

                let row = format!("{prefix} {preview:<42} {msg_count:>10}{current_marker}");
                let rendered = if is_selected {
                    self.styles.accent_bold.render(&row)
                } else if branch.is_current {
                    self.styles.accent.render(&row)
                } else {
                    self.styles.muted.render(&row)
                };
                let _ = writeln!(output, "  {rendered}");
            }
        }

        let _ = writeln!(
            output,
            "\n  {}",
            self.styles
                .muted_italic
                .render("↑/↓/j/k/PgUp/PgDn: navigate  Enter: switch  Esc: cancel  * = current")
        );
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{AutosaveDurabilityMode, AutosaveQueueMetrics};

    #[test]
    fn context_left_percent_reports_remaining_window() {
        assert_eq!(context_left_percent(200_000, 45_500), Some(77));
        assert_eq!(context_left_percent(4_096, 4_096), Some(0));
        assert_eq!(context_left_percent(4_096, 9_999), Some(0));
    }

    #[test]
    fn context_left_percent_omits_unknown_window_or_unused_context() {
        assert_eq!(context_left_percent(0, 45_500), None);
        assert_eq!(context_left_percent(200_000, 0), None);
    }

    fn demo_footer_segments() -> Vec<FooterSegment> {
        vec![
            FooterSegment::pair(
                "~/workspace/koompi-code-cli".to_string(),
                FOOTER_PRIORITY_CWD,
            ),
            FooterSegment::pair("main".to_string(), FOOTER_PRIORITY_BRANCH),
            FooterSegment {
                long: "Tokens: 45000 in / 500 out ($1.2345)".to_string(),
                short: "45000/500 tokens ($1.2345)".to_string(),
                priority: FOOTER_PRIORITY_TOKENS,
            },
            FooterSegment {
                long: "Context: 77% left".to_string(),
                short: "77% ctx".to_string(),
                priority: FOOTER_PRIORITY_CONTEXT,
            },
            FooterSegment::pair("Persist: balanced".to_string(), FOOTER_PRIORITY_PERSISTENCE),
            FooterSegment {
                long: "Shift+Enter: newline  |  Alt+Enter: multi-line".to_string(),
                short: String::new(),
                priority: FOOTER_PRIORITY_MODE_HINT,
            },
            FooterSegment::pair("/help".to_string(), FOOTER_PRIORITY_HELP),
            FooterSegment::pair("Ctrl+C quit".to_string(), FOOTER_PRIORITY_QUIT),
        ]
    }

    #[test]
    fn footer_keeps_context_at_every_width() {
        let segments = demo_footer_segments();
        for width in 20..=200_usize {
            let footer = fit_footer(&segments, width);
            assert!(
                footer.width() <= width,
                "footer overflows {width} columns: {footer:?}"
            );
            assert!(
                footer.contains("77%"),
                "context must survive at {width} columns: {footer:?}"
            );
        }
    }

    #[test]
    fn footer_drops_whole_segments_by_priority() {
        let segments = demo_footer_segments();

        assert_eq!(
            fit_footer(&segments, 198),
            "~/workspace/koompi-code-cli  |  main  |  Tokens: 45000 in / 500 out ($1.2345)  |  \
             Context: 77% left  |  Persist: balanced  |  \
             Shift+Enter: newline  |  Alt+Enter: multi-line  |  /help  |  Ctrl+C quit"
        );
        // 167 columns: the mode hint is the first whole segment to go.
        assert_eq!(
            fit_footer(&segments, 167 - 2),
            "~/workspace/koompi-code-cli  |  main  |  Tokens: 45000 in / 500 out ($1.2345)  |  \
             Context: 77% left  |  Persist: balanced  |  /help  |  Ctrl+C quit"
        );
        assert_eq!(
            fit_footer(&segments, 100 - 2),
            "~/workspace/koompi-code-cli  |  main  |  45000/500 tokens ($1.2345)  |  77% ctx  |  \
             Ctrl+C quit"
        );
        assert_eq!(
            fit_footer(&segments, 60 - 2),
            "~/workspace/koompi-code-cli  |  77% ctx  |  Ctrl+C quit"
        );
    }

    #[test]
    fn progress_suffix_shows_elapsed_tokens_and_interrupt_hint() {
        let suffix = progress_suffix(Some(std::time::Duration::from_secs(12)), 3_400);
        assert_eq!(
            suffix,
            "(12s \u{00b7} 3.4K tokens \u{00b7} esc to interrupt)"
        );
    }

    #[test]
    fn progress_suffix_omits_missing_elapsed_and_zero_tokens() {
        assert_eq!(progress_suffix(None, 0), "(esc to interrupt)");
    }

    #[test]
    fn normalize_raw_terminal_newlines_inserts_crlf() {
        let normalized = normalize_raw_terminal_newlines("hello\nworld\n".to_string());
        assert_eq!(normalized, "hello\r\nworld\r\n");
    }

    #[test]
    fn normalize_raw_terminal_newlines_preserves_existing_crlf() {
        let normalized = normalize_raw_terminal_newlines("hello\r\nworld\r\n".to_string());
        assert_eq!(normalized, "hello\r\nworld\r\n");
    }

    #[test]
    fn normalize_raw_terminal_newlines_handles_mixed_newlines() {
        let normalized = normalize_raw_terminal_newlines("a\r\nb\nc\r\nd\n".to_string());
        assert_eq!(normalized, "a\r\nb\r\nc\r\nd\r\n");
    }

    #[test]
    fn normalize_raw_terminal_newlines_preserves_utf8_content() {
        let normalized = normalize_raw_terminal_newlines("αβ\nγ\r\nδ\n".to_string());
        assert_eq!(normalized, "αβ\r\nγ\r\nδ\r\n");
    }

    #[test]
    fn clamp_to_terminal_height_noop_when_fits() {
        let input = "line1\nline2\nline3".to_string();
        // 2 newlines => 3 rows; term_height=4 allows 3 newlines => fits.
        assert_eq!(clamp_to_terminal_height(input.clone(), 4), input);
    }

    #[test]
    fn clamp_to_terminal_height_truncates_excess() {
        let input = "a\nb\nc\nd\ne\n".to_string(); // 5 newlines = 6 rows
        // term_height=4 => max 3 newlines => keeps "a\nb\nc\nd"
        let clamped = clamp_to_terminal_height(input, 4);
        assert_eq!(clamped, "a\nb\nc\nd");
    }

    #[test]
    fn clamp_to_terminal_height_zero_height() {
        let clamped = clamp_to_terminal_height("hello\nworld".to_string(), 0);
        assert_eq!(clamped, "");
    }

    #[test]
    fn clamp_to_terminal_height_exact_fit() {
        // term_height=3 => max 2 newlines. Input has exactly 2 => fits.
        let input = "a\nb\nc".to_string();
        assert_eq!(clamp_to_terminal_height(input.clone(), 3), input);
    }

    #[test]
    fn clamp_to_terminal_height_trailing_newline() {
        // "a\nb\n" = 2 newlines, 3 rows (last row empty).
        // term_height=2 => max 1 newline => "a\nb"
        let clamped = clamp_to_terminal_height("a\nb\n".to_string(), 2);
        assert_eq!(clamped, "a\nb");
    }

    #[test]
    fn persistence_footer_segment_healthy() {
        let metrics = AutosaveQueueMetrics {
            pending_mutations: 0,
            max_pending_mutations: 256,
            coalesced_mutations: 0,
            backpressure_events: 0,
            flush_started: 0,
            flush_succeeded: 0,
            flush_failed: 0,
            last_flush_batch_size: 0,
            last_flush_duration_ms: None,
            last_flush_trigger: None,
        };
        assert_eq!(
            format_persistence_footer_segment(AutosaveDurabilityMode::Balanced, metrics),
            "Persist: balanced"
        );
    }

    #[test]
    fn persistence_footer_segment_includes_backlog_and_failures() {
        let metrics = AutosaveQueueMetrics {
            pending_mutations: 256,
            max_pending_mutations: 256,
            coalesced_mutations: 99,
            backpressure_events: 4,
            flush_started: 5,
            flush_succeeded: 3,
            flush_failed: 2,
            last_flush_batch_size: 64,
            last_flush_duration_ms: Some(42),
            last_flush_trigger: Some(crate::session::AutosaveFlushTrigger::Periodic),
        };
        let rendered =
            format_persistence_footer_segment(AutosaveDurabilityMode::Throughput, metrics);
        assert!(rendered.contains("Persist: throughput"));
        assert!(rendered.contains("pending 256/256"));
        assert!(rendered.contains("flush-fail 2"));
        assert!(rendered.contains("backpressure"));
    }

    #[test]
    fn wrapped_plain_line_no_wrap_when_under_width() {
        let segments = wrapped_line_segments("hello", 10);
        assert_eq!(segments, vec!["hello"]);
    }

    #[test]
    fn wrapped_plain_line_wraps_when_over_width() {
        let segments = wrapped_line_segments("abcdef", 4);
        assert_eq!(segments, vec!["abcd", "ef"]);
    }

    #[test]
    fn wrapped_plain_line_preserves_empty_line() {
        let segments = wrapped_line_segments("", 8);
        assert_eq!(segments, vec![""]);
    }

    #[test]
    fn parse_fence_line_detects_backtick_and_tilde_fences() {
        assert_eq!(parse_fence_line("```rust"), Some(('`', 3, "rust")));
        assert_eq!(parse_fence_line("   ~~~~~"), Some(('~', 5, "")));
        assert_eq!(parse_fence_line("`not-a-fence"), None);
    }

    #[test]
    fn parse_fence_line_rejects_four_space_indent() {
        assert_eq!(parse_fence_line("    ```rust"), None);
    }

    #[test]
    fn streaming_unclosed_fence_none_when_balanced() {
        let markdown = "```rust\nfn main() {}\n```\n";
        assert_eq!(streaming_unclosed_fence(markdown), None);
    }

    #[test]
    fn streaming_unclosed_fence_detects_open_backtick_block() {
        let markdown = "Heading\n\n```rust\nfn main() {\n    println!(\"hi\");";
        assert_eq!(streaming_unclosed_fence(markdown), Some(('`', 3)));
    }

    #[test]
    fn streaming_unclosed_fence_does_not_close_on_trailing_text() {
        let markdown = "```rust\nfn main() {}\n``` trailing";
        assert_eq!(streaming_unclosed_fence(markdown), Some(('`', 3)));
    }

    #[test]
    fn streaming_unclosed_fence_closes_on_whitespace_only_suffix() {
        let markdown = "```rust\nfn main() {}\n```   \n";
        assert_eq!(streaming_unclosed_fence(markdown), None);
    }

    #[test]
    fn streaming_unclosed_fence_ignores_invalid_backtick_info() {
        let markdown = "```a`b\ncontent\n";
        assert_eq!(streaming_unclosed_fence(markdown), None);
    }

    #[test]
    fn stabilize_streaming_markdown_closes_unterminated_fence() {
        let markdown = "```python\nprint('hello')";
        let stabilized = stabilize_streaming_markdown(markdown);
        assert_eq!(stabilized.as_ref(), "```python\nprint('hello')\n```");
    }

    #[test]
    fn stabilize_streaming_markdown_preserves_balanced_input() {
        let markdown = "# Title\n\n- item\n";
        let stabilized = stabilize_streaming_markdown(markdown);
        assert_eq!(stabilized.as_ref(), markdown);
    }

    #[test]
    fn streaming_needs_markdown_renderer_false_for_plain_text() {
        let markdown = "Starting response... token_1 token_2";
        assert!(!streaming_needs_markdown_renderer(markdown));
    }

    #[test]
    fn streaming_needs_markdown_renderer_true_for_heading() {
        let markdown = "# Heading";
        assert!(streaming_needs_markdown_renderer(markdown));
    }

    #[test]
    fn streaming_needs_markdown_renderer_true_for_underscore_emphasis() {
        let markdown = "This is _important_.";
        assert!(streaming_needs_markdown_renderer(markdown));
    }

    #[test]
    fn append_streaming_plaintext_to_output_wraps_without_trailing_blank() {
        let mut out = String::new();
        append_streaming_plaintext_to_output(&mut out, "abcdef\n", 4);
        assert_eq!(out, "  abcd\n  ef\n");
    }
}
