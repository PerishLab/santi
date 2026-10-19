use std::{
    io::{self, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use santi_provider::openai::{Config, OpenAI};
use santi_provider::{Event, Item, Provider, Request};

use crate::support::body;

pub(crate) fn answered(listener: &TcpListener, payload: &str) {
    let mut stream = accepted(listener, Duration::from_secs(5)).expect("accept fixture request");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("fixture read deadline");
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .expect("fixture write deadline");
    body(&mut stream);
    let body = format!("data: {payload}\n\n");
    let response = format!(
        "HTTP/1.1 200 OK\r\nconnection: close\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .expect("write response");
}

pub(crate) async fn collected(url: String, attempts: Option<u32>) -> Vec<Event> {
    tokio::time::timeout(Duration::from_secs(10), collect(url, attempts))
        .await
        .expect("provider fixture exchange deadline")
}

async fn collect(url: String, attempts: Option<u32>) -> Vec<Event> {
    let provider = OpenAI::new(Config {
        key: "test-key".to_string(),
        model: "gpt-test".to_string(),
        url,
        effort: None,
        summary: None,
        ceiling: None,
        attempts,
        bytes: None,
        maintenance: None,
    });
    let mut stream = provider
        .stream(Request {
            model: provider.metadata().model,
            instructions: None,
            input: vec![Item::Message {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
            tools: None,
            previous: None,
        })
        .await
        .expect("stream response");
    let mut held = Vec::new();
    while let Some(event) = stream.next().await {
        held.push(event.expect("provider event"));
    }
    held
}

fn accepted(listener: &TcpListener, budget: Duration) -> io::Result<TcpStream> {
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + budget;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                return Ok(stream);
            }
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "fixture request deadline",
                ));
            }
            Err(error) => return Err(error),
        }
    }
}

#[test]
fn missing() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let error =
        accepted(&listener, Duration::from_millis(20)).expect_err("missing request is bounded");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[tokio::test]
async fn closes() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let url = format!("http://{}", listener.local_addr().expect("local address"));
    let server = thread::spawn(move || answered(&listener, "probe"));
    let response = tokio::time::timeout(
        Duration::from_secs(10),
        reqwest::Client::new()
            .post(url)
            .json(&serde_json::json!({}))
            .send(),
    )
    .await
    .expect("fixture response deadline")
    .expect("fixture response");
    assert_eq!(response.headers()["connection"], "close");
    server.join().expect("server thread");
}
