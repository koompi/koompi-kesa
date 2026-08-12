//! Permission rules for built-in and extension tool calls.
//!
//! Distinct from [`crate::permissions`], which persists per-extension capability
//! grants. This module decides whether a single tool call may run at all.

#[cfg(test)]
mod tests {
    #[test]
    fn module_is_wired_into_the_crate() {}
}
