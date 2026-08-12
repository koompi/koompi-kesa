use kode::error::Error;
use kode::session::{SessionEntry, SessionHeader};
use serde_json::json;

#[test]
fn upstream_session_header_and_entries_parse_with_rust_schema() {
    let header: SessionHeader = serde_json::from_value(json!({
        "type": "session",
        "version": 3,
        "id": "legacy-session-1",
        "timestamp": "2026-04-22T00:00:00.000Z",
        "cwd": "/tmp/pi-agent",
        "parentSession": "/tmp/pi-agent/parent.jsonl"
    }))
    .expect("legacy pi-mono header should parse");

    assert!(header.is_valid());
    assert_eq!(
        header.parent_session.as_deref(),
        Some("/tmp/pi-agent/parent.jsonl")
    );

    let entries = [
        json!({
            "type": "message",
            "id": "u1",
            "parentId": null,
            "timestamp": "2026-04-22T00:00:01.000Z",
            "message": {
                "role": "user",
                "content": "hello from pi-mono",
                "timestamp": 1_776_816_001_000_i64
            }
        }),
        json!({
            "type": "model_change",
            "id": "m1",
            "parentId": "u1",
            "timestamp": "2026-04-22T00:00:02.000Z",
            "provider": "openai",
            "modelId": "gpt-5.4"
        }),
        json!({
            "type": "thinking_level_change",
            "id": "t1",
            "parentId": "m1",
            "timestamp": "2026-04-22T00:00:03.000Z",
            "thinkingLevel": "high"
        }),
        json!({
            "type": "compaction",
            "id": "c1",
            "parentId": "t1",
            "timestamp": "2026-04-22T00:00:04.000Z",
            "summary": "compacted",
            "firstKeptEntryId": "u1",
            "tokensBefore": 123
        }),
        json!({
            "type": "branch_summary",
            "id": "b1",
            "parentId": "c1",
            "timestamp": "2026-04-22T00:00:05.000Z",
            "fromId": "u1",
            "summary": "branch summary"
        }),
        json!({
            "type": "custom",
            "id": "x1",
            "parentId": "b1",
            "timestamp": "2026-04-22T00:00:06.000Z",
            "customType": "fixture.state",
            "data": {"ok": true}
        }),
        json!({
            "type": "label",
            "id": "l1",
            "parentId": "x1",
            "timestamp": "2026-04-22T00:00:07.000Z",
            "targetId": "u1",
            "label": "start"
        }),
        json!({
            "type": "session_info",
            "id": "s1",
            "parentId": "l1",
            "timestamp": "2026-04-22T00:00:08.000Z",
            "name": "legacy session"
        }),
    ];

    let parsed = entries
        .into_iter()
        .map(serde_json::from_value::<SessionEntry>)
        .collect::<Result<Vec<_>, _>>()
        .expect("legacy pi-mono entries should parse");

    assert_eq!(parsed.len(), 8);
    assert!(matches!(parsed[0], SessionEntry::Message(_)));
    assert!(matches!(parsed[1], SessionEntry::ModelChange(_)));
    assert!(matches!(parsed[2], SessionEntry::ThinkingLevelChange(_)));
    assert!(matches!(parsed[3], SessionEntry::Compaction(_)));
    assert!(matches!(parsed[4], SessionEntry::BranchSummary(_)));
    assert!(matches!(parsed[5], SessionEntry::Custom(_)));
    assert!(matches!(parsed[6], SessionEntry::Label(_)));
    assert!(matches!(parsed[7], SessionEntry::SessionInfo(_)));
}

#[test]
fn hostcall_error_taxonomy_matches_g08_crosswalk() {
    let cases = [
        (Error::Aborted, "timeout"),
        (Error::auth("missing API key"), "denied"),
        (Error::validation("bad request"), "invalid_request"),
        (
            Error::Io(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "missing",
            ))),
            "io",
        ),
        (Error::session("corrupt jsonl"), "io"),
        (
            Error::SessionNotFound {
                path: "missing.jsonl".to_string(),
            },
            "io",
        ),
        (Error::api("upstream failed"), "internal"),
        (Error::config("bad config"), "internal"),
        (Error::extension("extension failed"), "internal"),
        (Error::provider("openai", "500"), "internal"),
        (Error::tool("bash", "failed"), "internal"),
        (
            serde_json::from_str::<serde_json::Value>("{")
                .map_err(Into::<Error>::into)
                .expect_err("malformed JSON should produce Error::Json"),
            "internal",
        ),
    ];

    for (error, expected_code) in cases {
        assert_eq!(error.hostcall_error_code(), expected_code, "{error:?}");
    }
}
