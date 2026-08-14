//! Differential extension conformance tests: compare TS oracle (Bun + jiti)
//! output against Rust `QuickJS` runtime output for the SAME extension source.
//!
//! Each test:
//! 1. Loads an extension .ts file through the Rust swc+`QuickJS` pipeline
//! 2. Runs the TS oracle harness (Bun + jiti) on the same file
//! 3. Compares registration snapshots (tools, commands, flags, shortcuts, etc.)
//!
//! Behaviour is not compared. `hasExecute`/`hasHandler` booleans are as far as
//! the oracle goes, and `messageRenderers` is collected and never diffed at all.
//!
//! The suite needs the `ext-conformance` feature. Without it the body compiles
//! to nothing, so the announcement below stands in for it rather than letting an
//! empty run read as a green one.

#[cfg(not(feature = "ext-conformance"))]
#[test]
#[ignore = "NOT RUN: the ext-conformance feature is off, so this suite measured nothing. Rerun with --features ext-conformance"]
fn ext_conformance_diff_suite_did_not_run() {}

#[cfg(feature = "ext-conformance")]
mod common;

#[cfg(feature = "ext-conformance")]
include!("ext_conformance/diff_suite.rs");
