#[cfg(test)]
mod tests {
    use kode::tools::{EditTool, Tool};
    use serde_json::json;
    use std::fs;
    use tempfile::tempdir;

    fn assert_only_crlf(content: &str) {
        let bytes = content.as_bytes();
        for idx in 0..bytes.len() {
            if bytes[idx] == b'\n' {
                assert!(
                    idx > 0 && bytes[idx - 1] == b'\r',
                    "found LF without preceding CR at byte {idx}",
                );
            }
        }
    }

    #[test]
    fn test_edit_trailing_whitespace_fuzzy() {
        asupersync::test_utils::run_test(|| async {
            let tmp = tempdir().unwrap();
            let file_path = tmp.path().join("fuzzy.txt");

            // File has "foo " (with trailing space)
            fs::write(&file_path, "foo ").unwrap();

            let tool = EditTool::new(tmp.path());

            // Case 1: User tries to replace "foo" (no space) with "bar" (no space).
            // Expectation: Fuzzy match works, but ignores/skips trailing space in file.
            // Result: "bar " (space preserved).
            let output = tool
                .execute(
                    "call1",
                    json!({
                        "path": "fuzzy.txt",
                        "oldText": "foo",
                        "newText": "bar"
                    }),
                    None,
                )
                .await
                .unwrap();

            assert!(!output.is_error);
            let content = fs::read_to_string(&file_path).unwrap();
            assert_eq!(content, "bar ");

            // Case 2: User tries to replace "bar " (with space) with "baz" (no space).
            // Expectation: Exact match works.
            // Result: "baz" (space deleted).
            let output = tool
                .execute(
                    "call2",
                    json!({
                        "path": "fuzzy.txt",
                        "oldText": "bar ",
                        "newText": "baz"
                    }),
                    None,
                )
                .await
                .unwrap();

            assert!(!output.is_error);
            let content = fs::read_to_string(&file_path).unwrap();
            assert_eq!(content, "baz");

            // Case 3: User tries to replace "baz" (no space) with "qux" (no space).
            // But let's say the file has "baz  " (2 spaces).
            fs::write(&file_path, "baz  ").unwrap();

            // User provides "baz " (1 space). This is an exact substring match at the
            // beginning of "baz  ", so edit replacement happens in exact mode first.
            // Result: "qux " (one trailing space remains).

            let output = tool
                .execute(
                    "call3",
                    json!({
                        "path": "fuzzy.txt",
                        "oldText": "baz ",
                        "newText": "qux"
                    }),
                    None,
                )
                .await
                .unwrap();

            assert!(!output.is_error);
            let content = fs::read_to_string(&file_path).unwrap();
            assert_eq!(content, "qux ");
        });
    }

    #[test]
    fn test_edit_crlf_invariance_between_oldtext_line_endings() {
        asupersync::test_utils::run_test(|| async {
            let tmp = tempdir().unwrap();
            let file_path = tmp.path().join("crlf.txt");
            let original = "alpha\r\nbeta\r\ncharlie\r\n";
            fs::write(&file_path, original).unwrap();

            let tool = EditTool::new(tmp.path());

            let output = tool
                .execute(
                    "call_crlf_lf",
                    json!({
                        "path": "crlf.txt",
                        "oldText": "alpha\nbeta\n",
                        "newText": "alpha\nbravo\n"
                    }),
                    None,
                )
                .await
                .unwrap();
            assert!(!output.is_error);
            let content_lf = fs::read_to_string(&file_path).unwrap();
            assert_eq!(content_lf, "alpha\r\nbravo\r\ncharlie\r\n");
            assert_only_crlf(&content_lf);

            fs::write(&file_path, original).unwrap();

            let output = tool
                .execute(
                    "call_crlf_crlf",
                    json!({
                        "path": "crlf.txt",
                        "oldText": "alpha\r\nbeta\r\n",
                        "newText": "alpha\r\nbravo\r\n"
                    }),
                    None,
                )
                .await
                .unwrap();
            assert!(!output.is_error);
            let content_crlf = fs::read_to_string(&file_path).unwrap();
            assert_eq!(content_crlf, "alpha\r\nbravo\r\ncharlie\r\n");
            assert_only_crlf(&content_crlf);

            assert_eq!(content_lf, content_crlf);
        });
    }
}
