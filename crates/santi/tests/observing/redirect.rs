use std::net::SocketAddr;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::{io::AsyncWriteExt, net::TcpStream};

use super::Wire;

pub(crate) async fn origin(
    stream: TcpStream,
    posts: Arc<AtomicUsize>,
    events: Arc<AtomicUsize>,
    location: SocketAddr,
) {
    let mut wire = Wire(stream);
    let (method, path) = wire.read().await.expect("read origin request");
    if method == "GET" && path == "/api/v1/strands/ss_cli/events" {
        events.fetch_add(1, Ordering::SeqCst);
        wire.0
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\r\n")
            .await
            .expect("event headers");
        std::future::pending::<()>().await;
    } else if method == "POST" && path == "/api/v1/strands/ss_cli/send" {
        posts.fetch_add(1, Ordering::SeqCst);
        let response = format!(
            "HTTP/1.1 307 Temporary Redirect\r\nlocation: http://{location}/replayed\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
        );
        wire.0
            .write_all(response.as_bytes())
            .await
            .expect("redirect");
    }
}
