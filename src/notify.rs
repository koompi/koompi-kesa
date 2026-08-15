//! Turn-end notification, straight to the terminal.
//!
//! A long run used to finish in silence. This writes an OSC 9 desktop
//! notification where the terminal understands one and a bell where it does
//! not, bypassing the renderer: both are terminal-level sequences and neither
//! belongs in a frame.
//!
//! It fires only when the terminal is unfocused. A notification that arrives
//! while the user is watching the screen is noise, and noise gets muted, after
//! which the feature is worse than absent.
//!
//! Focus comes from DEC mode 1004 focus reporting, which only the code holding
//! the terminal's input can see. That code calls [`set_terminal_focus`]. Until
//! something does, focus is [`Focus::Unknown`] and the gate falls back to the
//! `KESA_NOTIFY` setting, which defaults to on.

use std::io::Write;
use std::sync::atomic::{AtomicU8, Ordering};

const BEL: u8 = 0x07;
const ESC: u8 = 0x1b;

/// Longest message put into an escape sequence. A terminal that mishandles a
/// long OSC string should not be handed one.
const MAX_MESSAGE_CHARS: usize = 160;

const FOCUS_UNKNOWN: u8 = 0;
const FOCUS_FOCUSED: u8 = 1;
const FOCUS_UNFOCUSED: u8 = 2;

static FOCUS: AtomicU8 = AtomicU8::new(FOCUS_UNKNOWN);

/// Whether the terminal window holding this session has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// Nothing has reported focus, so the terminal is presumed not to support
    /// focus reporting or nothing has wired it up yet.
    Unknown,
    Focused,
    Unfocused,
}

/// Record a focus change. The caller is whoever reads the terminal's events.
pub fn set_terminal_focus(focused: bool) {
    FOCUS.store(
        if focused {
            FOCUS_FOCUSED
        } else {
            FOCUS_UNFOCUSED
        },
        Ordering::Relaxed,
    );
}

#[must_use]
pub fn terminal_focus() -> Focus {
    match FOCUS.load(Ordering::Relaxed) {
        FOCUS_FOCUSED => Focus::Focused,
        FOCUS_UNFOCUSED => Focus::Unfocused,
        _ => Focus::Unknown,
    }
}

/// Test seam: focus is process-global, so a test that sets it has to put it
/// back.
#[cfg(test)]
fn clear_terminal_focus() {
    FOCUS.store(FOCUS_UNKNOWN, Ordering::Relaxed);
}

/// What [`emit`] did, and when it did nothing, why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Osc9,
    Bell,
    /// The user is looking at the terminal.
    SuppressedFocused,
    /// `KESA_NOTIFY` is off.
    SuppressedDisabled,
    /// Nothing on the other end to notify.
    SuppressedNoTerminal,
}

impl Outcome {
    #[must_use]
    pub const fn emitted(self) -> bool {
        matches!(self, Self::Osc9 | Self::Bell)
    }
}

/// Whether a notification would be emitted now, without emitting one.
///
/// Callers route the `Notification` hook through this same answer, so a user's
/// `notify-send` is silenced by focus exactly like the bell is.
#[must_use]
pub fn should_notify() -> Outcome {
    if !enabled() {
        return Outcome::SuppressedDisabled;
    }
    if terminal_focus() == Focus::Focused {
        return Outcome::SuppressedFocused;
    }
    if supports_osc9() {
        Outcome::Osc9
    } else {
        Outcome::Bell
    }
}

/// Notify the user that something they were waiting for is done.
pub fn emit(message: &str) -> Outcome {
    let intent = should_notify();
    if !intent.emitted() {
        return intent;
    }
    let Some(mut target) = terminal_writer() else {
        return Outcome::SuppressedNoTerminal;
    };
    write_notification(&mut target, message, intent == Outcome::Osc9);
    intent
}

/// `KESA_NOTIFY=0` turns it off. Anything else, including unset, leaves it on:
/// without focus reporting this setting is the only condition on emitting, and
/// a user who does not want a bell needs a way to say so.
#[must_use]
pub fn enabled() -> bool {
    !matches!(
        crate::env::var("NOTIFY").as_deref(),
        Some("0" | "off" | "false" | "no")
    )
}

fn write_notification<W: Write>(out: &mut W, message: &str, osc9: bool) {
    if osc9 {
        let _ = out.write_all(&[ESC, b']']);
        let _ = out.write_all(b"9;");
        let _ = out.write_all(sanitize(message).as_bytes());
        let _ = out.write_all(&[BEL]);
    } else {
        let _ = out.write_all(&[BEL]);
    }
    let _ = out.flush();
}

/// An escape or a bell inside the message would end the sequence early, and a
/// newline would end the line the terminal is drawing.
fn sanitize(message: &str) -> String {
    message
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_MESSAGE_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Terminals known to turn OSC 9 into a desktop notification.
///
/// `KESA_NOTIFY_OSC` overrides the guess in both directions, because this list
/// cannot stay complete and a wrong guess is either a missing notification or a
/// stray title change.
fn supports_osc9() -> bool {
    match crate::env::var("NOTIFY_OSC").as_deref() {
        Some("1" | "on" | "true" | "yes") => return true,
        Some("0" | "off" | "false" | "no") => return false,
        _ => {}
    }

    // tmux and screen swallow OSC 9 unless the pane opts into passthrough, and
    // their own bell handling already flags the window.
    if std::env::var_os("TMUX").is_some() || std::env::var_os("STY").is_some() {
        return false;
    }

    if ["KITTY_WINDOW_ID", "WT_SESSION", "KONSOLE_VERSION"]
        .iter()
        .any(|name| std::env::var_os(name).is_some())
    {
        return true;
    }

    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    if matches!(
        term_program.as_str(),
        "iTerm.app" | "WezTerm" | "ghostty" | "Hyper" | "rio"
    ) {
        return true;
    }

    let term = std::env::var("TERM").unwrap_or_default();
    ["kitty", "wezterm", "ghostty", "foot", "rio"]
        .iter()
        .any(|name| term.contains(name))
}

/// The terminal, not the renderer, and not a redirected stdout.
///
/// A unit test inherits the developer's terminal on its file descriptors, so
/// under `cfg(test)` there is deliberately nothing to write to: a test suite
/// that rings the bell a few hundred times is a defect.
#[cfg(test)]
const fn terminal_writer() -> Option<Box<dyn Write>> {
    None
}

#[cfg(not(test))]
fn terminal_writer() -> Option<Box<dyn Write>> {
    use std::io::IsTerminal as _;

    #[cfg(unix)]
    if let Ok(tty) = std::fs::OpenOptions::new().write(true).open("/dev/tty") {
        return Some(Box::new(tty));
    }
    let stdout = std::io::stdout();
    if stdout.is_terminal() {
        return Some(Box::new(stdout));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(message: &str, osc9: bool) -> Vec<u8> {
        let mut buffer = Vec::new();
        write_notification(&mut buffer, message, osc9);
        buffer
    }

    #[test]
    fn osc_nine_carries_the_message_and_the_fallback_is_a_bare_bell() {
        assert_eq!(
            rendered("turn done", true),
            b"\x1b]9;turn done\x07".to_vec()
        );
        assert_eq!(rendered("turn done", false), vec![0x07]);
    }

    #[test]
    fn a_message_cannot_close_the_sequence_early() {
        let bytes = rendered("done\x07 and \x1b]0;title\x07", true);
        let body = &bytes[3..bytes.len() - 1];

        assert!(!body.contains(&BEL), "message kept a bell: {bytes:?}");
        assert!(!body.contains(&ESC), "message kept an escape: {bytes:?}");
        assert_eq!(bytes.first(), Some(&ESC));
        assert_eq!(bytes.last(), Some(&BEL));
    }

    #[test]
    fn focus_gates_the_notification_and_unknown_focus_does_not() {
        clear_terminal_focus();
        assert_eq!(terminal_focus(), Focus::Unknown);
        assert!(should_notify().emitted());

        set_terminal_focus(true);
        assert_eq!(should_notify(), Outcome::SuppressedFocused);

        set_terminal_focus(false);
        assert!(should_notify().emitted());

        clear_terminal_focus();
    }

    #[test]
    fn a_long_message_is_capped() {
        let long = "x".repeat(MAX_MESSAGE_CHARS * 2);
        assert_eq!(sanitize(&long).chars().count(), MAX_MESSAGE_CHARS);
    }
}
