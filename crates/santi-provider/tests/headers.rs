use futures_util::StreamExt;
use santi_provider::{Event, Headers, Provider, Request, chat::completions, openai};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    thread,
    time::Duration,
};

fn configured() -> Headers {
    Headers::new(
        &BTreeMap::from([
            ("User-Agent".into(), "{{meta.user_agent}}".into()),
            ("x-opencode-session".into(), "santi-{{strand.id}}".into()),
            ("x-literal".into(), "literal".into()),
        ]),
        "santi-api/v1.2.3",
    )
    .unwrap()
}

#[test]
fn scopes() {
    let headers = configured();
    for strand in ["ss_first", "ss_second", "ss_first"] {
        let held = headers.render(Some(strand)).unwrap();
        assert_eq!(held["user-agent"], "santi-api/v1.2.3");
        assert_eq!(held["x-opencode-session"], format!("santi-{strand}"));
    }
    assert!(headers.render(None).is_err());
    assert!(headers.render(Some("")).is_err());
    assert!(headers.render(Some("line\r\nbreak")).is_err());
    assert!(Headers::default().render(None).unwrap().is_empty());
    let literal = Headers::new(&BTreeMap::from([("x-static".into(), "ok".into())]), "").unwrap();
    assert_eq!(literal.render(None).unwrap()["x-static"], "ok");
}

#[test]
fn refusal() {
    for (name, value) in [
        ("x-test", "{{absent}}"),
        ("x-test", "{{strand.id}"),
        ("x-test", "line\r\nbreak"),
        ("bad name", "ok"),
        ("Authorization", "secret"),
        ("Host", "elsewhere"),
        ("content-length", "0"),
        ("content-type", "text/plain"),
        ("content-encoding", "gzip"),
        ("transfer-encoding", "chunked"),
        ("Connection", "x-test"),
    ] {
        assert!(Headers::new(&BTreeMap::from([(name.into(), value.into())]), "agent/v1").is_err());
    }
    let duplicate = BTreeMap::from([
        ("X-Test".into(), "one".into()),
        ("x-test".into(), "two".into()),
    ]);
    assert!(Headers::new(&duplicate, "agent/v1").is_err());
    let private = Headers::new(
        &BTreeMap::from([("x-private".into(), "private-value".into())]),
        "agent/v1",
    )
    .unwrap();
    assert!(!format!("{private:?}").contains("private-value"));
    let error = private.render(Some("invalid\r\nvalue"));
    assert!(error.is_ok());
}

#[tokio::test]
async fn transports() {
    for chat in [true, false] {
        for strand in ["ss_first", "ss_second"] {
            exchange(chat, strand, false).await;
        }
    }
    exchange(false, "ss_retry", true).await;
}

async fn exchange(chat: bool, strand: &str, retry: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let session = format!("x-opencode-session: santi-{strand}\r\n");
    let server = thread::spawn(move || {
        for attempt in 0..=usize::from(retry) {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                head.push(byte[0]);
            }
            let head = String::from_utf8(head).unwrap().to_ascii_lowercase();
            assert!(head.contains("user-agent: santi-api/v1.2.3\r\n"));
            assert!(head.contains(&session));
            assert!(head.contains("x-literal: literal\r\n"));
            assert!(head.contains("authorization: bearer test-key\r\n"));
            let length = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .unwrap()
                .parse()
                .unwrap();
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            assert!(
                !String::from_utf8(body)
                    .unwrap()
                    .contains("x-opencode-session")
            );
            let event = if retry && attempt == 0 {
                r#"{"type":"error","error":{"message":"test failure"}}"#
            } else if chat {
                r#"{"id":"chat-test","choices":[{"delta":{},"finish_reason":"stop"}]}"#
            } else {
                r#"{"type":"response.completed","response":{"id":"resp-test"}}"#
            };
            let body = format!("data: {event}\n\n");
            let response = format!(
                "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).unwrap();
        }
    });
    let provider: Arc<dyn Provider> = if chat {
        Arc::new(completions::Chat::new(completions::Config {
            outgoing: configured(),
            provider: "fixture".into(),
            key: "test-key".into(),
            model: "fixture".into(),
            url,
            thinking: None,
            replay: Default::default(),
            effort: None,
            ceiling: None,
            bytes: None,
            maintenance: None,
            headers: Vec::new(),
        }))
    } else {
        Arc::new(openai::OpenAI::new(openai::Config {
            outgoing: configured(),
            key: "test-key".into(),
            model: "fixture".into(),
            url,
            effort: None,
            summary: None,
            ceiling: None,
            bytes: None,
            maintenance: None,
            attempts: Some(2),
            headers: Vec::new(),
        }))
    };
    let request = Request {
        strand: Some(strand.into()),
        model: "fixture".into(),
        instructions: None,
        input: Vec::new(),
        tools: None,
        previous: None,
    };
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut stream = provider.stream(request).await.unwrap();
        let mut completed = false;
        while let Some(event) = stream.next().await {
            if matches!(event.unwrap(), Event::Completed { .. }) {
                completed = true;
            }
        }
        assert!(completed);
    })
    .await
    .unwrap();
    server.join().unwrap();
}
