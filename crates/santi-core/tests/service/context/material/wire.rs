use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::test]
async fn chat() {
    transmitted(true).await;
}

#[tokio::test]
async fn responses() {
    transmitted(false).await;
}

async fn transmitted(chat: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { capture(listener, chat).await });
    let provider = provider(url, chat);
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    source(&temp, "wire-marker");
    let service = Service::open(config(&temp), provider)
        .await
        .unwrap()
        .catalogued(sources(&temp))
        .unwrap();
    let strand = service.weave().await.unwrap().strand.id;
    service
        .load(Scope::Soul, "soul_default", command("shared"))
        .await
        .unwrap();
    service
        .load(Scope::Strand, &strand, command("local"))
        .await
        .unwrap();
    let prompt = service
        .material(
            &strand,
            material::Request {
                kind: material::Kind::SystemPrompt,
            },
        )
        .await
        .unwrap()
        .text;
    let audit = service.audit(&strand).await.unwrap().unwrap();
    assert_eq!(audit.materials.len(), 1);
    assert_eq!(audit.materials[0].origins.len(), 2);
    assert!(audit.materials[0].bytes > "wire-marker".len());
    if chat {
        assert_eq!(audit.estimate.instructions, 0);
        assert!(audit.estimate.total >= audit.estimate.input + audit.estimate.tools);
    } else {
        assert_eq!(audit.estimate.instructions, prompt.len() as i64);
    }
    let posted = service
        .send(
            &strand,
            strand::Post {
                content: vec![santi_core::message::Part::Text {
                    text: "wire check".into(),
                }],
            },
        )
        .await
        .unwrap();
    let turn = accepted_turn(&service, &posted).await;
    let runtime = Probe::new(&service).completed_turn(&strand, &turn.id).await;
    let body = server.await.unwrap();
    let text = if chat {
        body["messages"][0]["content"].as_str().unwrap()
    } else {
        body["instructions"].as_str().unwrap()
    };
    assert_eq!(text, prompt);
    assert_eq!(text.matches("wire-marker").count(), 1);
    assert_eq!(runtime.contexts.len(), 1);
    assert!(runtime.contexts[0].thinking.is_some());
    assert_eq!(
        runtime.contexts[0].materials[0].bytes,
        audit.materials[0].bytes
    );
}

fn provider(url: String, chat: bool) -> Arc<dyn Provider> {
    if chat {
        Arc::new(santi_provider::chat::completions::Chat::new(
            santi_provider::chat::completions::Config {
                provider: "deepseek".into(),
                key: "test".into(),
                model: "test".into(),
                url,
                thinking: Some("enabled".into()),
                replay: santi_provider::chat::completions::Replay::History,
                effort: None,
                ceiling: None,
                bytes: None,
                maintenance: None,
                headers: vec![],
            },
        ))
    } else {
        Arc::new(santi_provider::openai::OpenAI::new(
            santi_provider::openai::Config {
                key: "test".into(),
                model: "test".into(),
                url,
                effort: None,
                summary: None,
                ceiling: None,
                bytes: None,
                maintenance: None,
                attempts: Some(1),
                headers: vec![],
            },
        ))
    }
}

async fn capture(listener: TcpListener, chat: bool) -> serde_json::Value {
    let (mut stream, _) = listener.accept().await.unwrap();
    let request = request(&mut stream).await;
    let event = if chat {
        r#"data: {"id":"chatcmpl_test","choices":[{"delta":{"content":"done"},"finish_reason":"stop"}]}"#
    } else {
        r#"data: {"type":"response.completed","response":{"id":"resp_test"}}"#
    };
    let body = format!("{event}\n\n");
    stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    request
}

async fn request(stream: &mut TcpStream) -> serde_json::Value {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0; 4096];
        let count = stream.read(&mut chunk).await.unwrap();
        assert!(count > 0 && bytes.len() < 100_000);
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
            let header = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
            let length: usize = header
                .lines()
                .find_map(|l| l.strip_prefix("content-length: "))
                .unwrap()
                .parse()
                .unwrap();
            if bytes.len() >= end + 4 + length {
                return serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
            }
        }
    }
}
