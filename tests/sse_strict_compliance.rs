use kesa::sse::SseParser;

#[test]
fn test_sse_bom_stripping() {
    let mut parser = SseParser::new();
    // UTF-8 BOM is \u{FEFF}
    let input = "\u{FEFF}data: hello\n\n";
    let events = parser.feed(input);
    assert_eq!(events.len(), 1, "Should parse one event despite BOM");
    assert_eq!(events[0].data, "hello", "Data should be 'hello'");
}

#[test]
fn test_sse_bare_cr_handling() {
    let mut parser = SseParser::new();
    // CR as line terminator: "data: line1\rdata: line2\n\n"
    let input = "data: line1\rdata: line2\n\n";
    let events = parser.feed(input);

    // Per SSE spec, bare CR is a line terminator:
    // 1. "data: line1" (terminated by CR)
    // 2. "data: line2" (terminated by LF)
    // 3. "" (blank line from second LF) dispatches the event.
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "line1\nline2");
}

#[test]
fn test_sse_retry_accepts_digits_only() {
    let mut parser = SseParser::new();
    let events = parser.feed("retry: +3000\ndata: first\n\nretry: 3000\ndata: second\n\n");

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].retry, None);
    assert_eq!(events[1].retry, Some(3000));
}
