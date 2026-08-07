use std::io::Write;
use std::net::TcpListener;

use futures_util::StreamExt;
use santi_provider::openai::{Config, OpenAI};
use santi_provider::{Event, Item, Provider, Request};

use crate::support::body;

pub(crate) fn answered(listener: &TcpListener, payload: &str) {
    let (mut stream, _) = listener.accept().expect("accept request");
    body(&mut stream);
    let body = format!("data: {payload}\n\n");
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .expect("write response");
}

pub(crate) async fn collected(url: String, attempts: Option<u32>) -> Vec<Event> {
    let provider = OpenAI::new(Config {
        key: "test-key".to_string(),
        model: "gpt-test".to_string(),
        url,
        effort: None,
        summary: None,
        ceiling: None,
        attempts,
        bytes: None,
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
