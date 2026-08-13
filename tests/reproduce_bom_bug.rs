#[cfg(test)]
mod tests {
    use futures::{StreamExt, stream};
    use kesa::sse::SseStream;

    #[test]
    fn fragmented_utf8_bom_is_stripped_once() {
        let chunks = vec![
            Ok::<_, std::io::Error>(vec![0xEF]),
            Ok(vec![0xBB]),
            Ok([&[0xBF][..], b"data: hello\n\n"].concat()),
        ];
        let mut stream = SseStream::new(stream::iter(chunks));

        futures::executor::block_on(async {
            let event = stream
                .next()
                .await
                .expect("event")
                .expect("valid SSE event");
            assert_eq!(event.data, "hello");
            assert!(stream.next().await.is_none());
        });
    }
}
