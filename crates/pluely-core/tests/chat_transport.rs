use pluely_core::{model::LlmConfig, providers};
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tokio_util::sync::CancellationToken;
#[tokio::test]
async fn actual_http_stream_reassembles_chunks_and_auth_header() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = vec![];
        loop {
            let mut part = [0u8; 4096];
            let n = stream.read(&mut part).await.unwrap();
            assert!(n > 0);
            request.extend_from_slice(&part[..n]);
            if let Some(pos) = request.windows(4).position(|x| x == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..pos]).to_lowercase();
                let content = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .unwrap()
                    .trim()
                    .parse::<usize>()
                    .unwrap();
                if request.len() >= pos + 4 + content {
                    assert!(headers.contains("authorization: bearer test-key"));
                    assert!(headers.starts_with("post /v1/chat/completions "));
                    break;
                }
            }
        }
        let body="data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"世界\"}}]}\n\ndata: [DONE]\n\n";
        stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await.unwrap();
        for chunk in body.as_bytes().chunks(3) {
            stream.write_all(chunk).await.unwrap();
        }
        stream.shutdown().await.unwrap();
    });
    let config = LlmConfig {
        base_url: format!("http://{address}/v1"),
        model: "test-model".into(),
        vision: false,
        temperature: 0.3,
    };
    let mut deltas = String::new();
    let text = providers::chat(
        &config,
        Some("test-key"),
        vec![json!({"role":"user","content":"hello"})],
        &CancellationToken::new(),
        |s| deltas.push_str(s),
    )
    .await
    .unwrap();
    server.await.unwrap();
    assert_eq!(text, "Hello 世界");
    assert_eq!(text, deltas);
}
#[tokio::test]
async fn cancel_interrupts_a_request_that_never_returns_headers() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let hold = CancellationToken::new();
    let server_hold = hold.clone();
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        let _ = accepted_tx.send(());
        server_hold.cancelled().await;
    });
    let cancel = CancellationToken::new();
    let c = cancel.clone();
    let config = LlmConfig {
        base_url: format!("http://{address}/v1"),
        model: "test".into(),
        vision: false,
        temperature: 0.3,
    };
    let request =
        tokio::spawn(async move { providers::chat(&config, None, vec![], &c, |_| {}).await });
    accepted_rx.await.unwrap();
    cancel.cancel();
    let result = tokio::time::timeout(std::time::Duration::from_secs(1), request)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(result, Err(pluely_core::model::Error::Cancelled)));
    hold.cancel();
    server.await.unwrap();
}

#[tokio::test]
async fn non_stream_json_works_and_oversized_body_is_rejected() {
    for oversized in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut part = [0u8; 4096];
                let n = stream.read(&mut part).await.unwrap();
                assert!(n > 0);
                request.extend_from_slice(&part[..n]);
                if let Some(pos) = request.windows(4).position(|x| x == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..pos]).to_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|h| h.strip_prefix("content-length:"))
                        .unwrap()
                        .trim()
                        .parse()
                        .unwrap();
                    if request.len() >= pos + 4 + length {
                        assert!(!headers.contains("authorization:"));
                        break;
                    }
                }
            }
            let text = if oversized {
                "x".repeat(1_048_577)
            } else {
                "A real JSON response".to_owned()
            };
            let body = json!({"choices":[{"message":{"content":text}}]}).to_string();
            stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).await.unwrap();
            // An oversized response may be disconnected as soon as the budget is reached.
            let _ = stream.write_all(body.as_bytes()).await;
        });
        let config = LlmConfig {
            base_url: format!("http://{address}/v1"),
            model: "local-test".into(),
            vision: false,
            temperature: 0.3,
        };
        let result =
            providers::chat(&config, None, vec![], &CancellationToken::new(), |_| {}).await;
        if oversized {
            assert!(result.unwrap_err().to_string().contains("1 MB budget"));
        } else {
            assert_eq!(result.unwrap(), "A real JSON response");
        }
        server.await.unwrap();
    }
}
