use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

use futures_util::StreamExt;
use santi_provider::chat::completions::{self, Chat};
use santi_provider::openai::{self, OpenAI};
use santi_provider::{Event, Item, Provider, Receipt, Report, Request, Trace, Usage};

const HEADERS: &str = "x-request-id: req_1\r\nx-teamo-actual-model: gpt-5.5-real\r\nx-trace-id: trace_1\r\nset-cookie: session=secret\r\n";

#[tokio::test]
async fn responses() {
    let url = served(&[
        r#"{"type":"response.created","response":{"id":"resp_1"}}"#,
        r#"{"type":"response.incomplete","response":{"id":"resp_1","status":"incomplete","incomplete_details":{"reason":"max_output_tokens"},"model":"gpt-5.5","usage":{"input_tokens":120,"input_tokens_details":{"cached_tokens":100},"output_tokens":30,"output_tokens_details":{"reasoning_tokens":20}}}}"#,
        r#"{"type":"response.completed","response":{"id":"resp_1","status":"completed","model":"gpt-5.5","usage":{"input_tokens":120,"input_tokens_details":{"cached_tokens":100},"output_tokens":30,"output_tokens_details":{"reasoning_tokens":20}}}}"#,
    ]);
    let provider = OpenAI::new(openai::Config {
        key: "test-key".to_string(),
        model: "gpt-5.5".to_string(),
        url,
        effort: None,
        summary: None,
        ceiling: None,
        bytes: None,
        maintenance: None,
        attempts: None,
        headers: allowed(),
    });
    let (receipts, reports) = traced(&provider).await;
    assert_eq!(receipted(receipts), vec![kept()]);
    let usage = Some(Usage {
        input: Some(120),
        cached: Some(100),
        output: Some(30),
        reasoning: Some(20),
    });
    assert_eq!(
        reports,
        vec![
            Report {
                status: Some("incomplete".to_string()),
                incomplete: Some("max_output_tokens".to_string()),
                model: Some("gpt-5.5".to_string()),
                usage: usage.clone(),
            },
            Report {
                status: Some("completed".to_string()),
                incomplete: None,
                model: Some("gpt-5.5".to_string()),
                usage,
            },
        ]
    );
}

#[tokio::test]
async fn completions() {
    let url = served(&[
        r#"{"id":"chat_1","model":"deepseek-v4-pro","choices":[{"delta":{"content":"hi"}}]}"#,
        r#"{"id":"chat_1","model":"deepseek-v4-pro","choices":[{"delta":{},"finish_reason":"length"}],"usage":{"prompt_tokens":50,"prompt_tokens_details":{"cached_tokens":40},"completion_tokens":8,"completion_tokens_details":{"reasoning_tokens":5}}}"#,
    ]);
    let provider = Chat::new(completions::Config {
        provider: "deepseek".to_string(),
        key: "test-key".to_string(),
        model: "deepseek-v4-pro".to_string(),
        url,
        thinking: None,
        effort: None,
        ceiling: None,
        bytes: None,
        maintenance: None,
        headers: allowed(),
    });
    let (receipts, reports) = traced(&provider).await;
    assert_eq!(receipted(receipts), vec![kept()]);
    assert_eq!(
        reports,
        vec![Report {
            status: Some("length".to_string()),
            incomplete: None,
            model: Some("deepseek-v4-pro".to_string()),
            usage: Some(Usage {
                input: Some(50),
                cached: Some(40),
                output: Some(8),
                reasoning: Some(5),
            }),
        }]
    );
}

#[tokio::test]
async fn unlisted() {
    let url = served(&[r#"{"type":"response.completed","response":{"id":"resp_1"}}"#]);
    let provider = OpenAI::new(openai::Config {
        key: "test-key".to_string(),
        model: "gpt-5.5".to_string(),
        url,
        effort: None,
        summary: None,
        ceiling: None,
        bytes: None,
        maintenance: None,
        attempts: None,
        headers: Vec::new(),
    });
    let (receipts, _) = traced(&provider).await;
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].status, 200);
    assert!(receipts[0].headers.is_empty());
}

fn allowed() -> Vec<String> {
    [
        "X-Request-Id",
        "x-teamo-actual-model",
        "set-cookie",
        "x-absent",
    ]
    .map(str::to_string)
    .to_vec()
}

fn kept() -> Vec<(String, String)> {
    vec![
        ("x-request-id".to_string(), "req_1".to_string()),
        (
            "x-teamo-actual-model".to_string(),
            "gpt-5.5-real".to_string(),
        ),
    ]
}

fn receipted(receipts: Vec<Receipt>) -> Vec<Vec<(String, String)>> {
    receipts
        .into_iter()
        .inspect(|receipt| assert_eq!(receipt.status, 200))
        .map(|receipt| receipt.headers)
        .collect()
}

async fn traced(provider: &dyn Provider) -> (Vec<Receipt>, Vec<Report>) {
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
    let (mut receipts, mut reports) = (Vec::new(), Vec::new());
    while let Some(event) = stream.next().await {
        match event.expect("provider event") {
            Event::Traced(Trace::Received(receipt)) => receipts.push(receipt),
            Event::Traced(Trace::Reported(report)) => reports.push(report),
            _ => {}
        }
    }
    (receipts, reports)
}

fn served(frames: &[&str]) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let url = format!("http://{}", listener.local_addr().expect("local address"));
    let body: String = frames
        .iter()
        .map(|frame| format!("data: {frame}\n\n"))
        .collect();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        drained(&mut stream);
        let response = format!(
            "HTTP/1.1 200 OK\r\nconnection: close\r\ncontent-type: text/event-stream\r\n{HEADERS}content-length: {}\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });
    url
}

fn drained(stream: &mut impl Read) {
    let mut request = Vec::new();
    let mut buffer = [0; 1024];
    let length = loop {
        let read = stream.read(&mut buffer).expect("read request");
        assert!(read > 0, "connection closed early");
        request.extend_from_slice(&buffer[..read]);
        let Some(split) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..split]).to_ascii_lowercase();
        let length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .expect("content length");
        break split + 4 + length;
    };
    while request.len() < length {
        let read = stream.read(&mut buffer).expect("read body");
        assert!(read > 0, "connection closed before body");
        request.extend_from_slice(&buffer[..read]);
    }
}
