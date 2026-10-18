//! The venue's end of the feed's socket: a local listener, and each connection the feed makes to it taking
//! the frames the feed sends and sending recorded ones back.

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{WebSocketStream, accept_async};

use super::WAIT;

/// One connection the feed made, as the venue's end of it.
pub(super) struct Server(pub(super) WebSocketStream<TcpStream>);

impl Server {
    /// The next text frame the feed sent.
    pub(super) async fn takes(&mut self) -> String {
        loop {
            match timeout(WAIT, self.0.next()).await.expect("a frame in time").expect("an open socket").unwrap() {
                Message::Text(text) => return text.to_string(),
                Message::Ping(_) | Message::Pong(_) => {}
                other => panic!("expected text, got {other:?}"),
            }
        }
    }

    pub(super) async fn sends(&mut self, text: &str) {
        self.0.send(Message::text(text)).await.unwrap();
    }
}

pub(super) async fn listen() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    (listener, url)
}

pub(super) async fn accept(listener: &TcpListener) -> Server {
    let (stream, _) = timeout(WAIT, listener.accept()).await.expect("the feed dials").unwrap();
    Server(accept_async(stream).await.unwrap())
}
