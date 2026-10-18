//! A mainnet [`Feed`] through a local socket that keeps each text frame the venue sends, so a test
//! can hold what the venue said against its recording: the feed itself hands back typed frames.

use std::time::Duration;

use df_hyperliquid_core::Network;
use df_hyperliquid_core::client::native::Feed;
use df_hyperliquid_core::live::{Event, Frame, Method, Session, Subscription, Timing};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio::time::{Instant, timeout, timeout_at};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, connect_async};

/// How long a channel has to be acknowledged and send its first frame.
pub const CHANNEL: Duration = Duration::from_secs(20);

/// The feed, and the text of each frame it read, in order: one for each [`Event::Frame`] or
/// [`Event::Unreadable`] it hands back.
pub struct Tapped {
    feed: Feed,
    kept: UnboundedReceiver<String>,
}

impl Tapped {
    /// Each time the feed dials, the tap dials the venue. The venue closes a socket after a minute
    /// in which it sent nothing but acknowledgements, so the feed pings after ten quiet seconds,
    /// not thirty, to stay open across the channels that send only on change.
    pub async fn open() -> Tapped {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let (keep, kept) = unbounded_channel::<String>();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let mut feed = accept_async(stream).await.unwrap();
                let (mut venue, _) = connect_async(Network::Mainnet.ws_url()).await.unwrap();
                loop {
                    tokio::select! {
                        sent = feed.next() => match sent {
                            Some(Ok(Message::Text(text))) => {
                                if venue.send(Message::Text(text)).await.is_err() {
                                    break;
                                }
                            }
                            Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                            Some(Ok(_)) => {}
                        },
                        heard = venue.next() => match heard {
                            Some(Ok(Message::Text(text))) => {
                                let _ = keep.send(text.to_string());
                                if feed.send(Message::Text(text)).await.is_err() {
                                    break;
                                }
                            }
                            Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                            Some(Ok(_)) => {}
                        },
                    }
                }
            }
        });
        let timing = Timing { ping_after_ms: 10_000, ..Timing::DEFAULT };
        Tapped { feed: Feed::start_at(url, Session::new(Network::Mainnet).with_timing(timing)), kept }
    }

    /// Subscribes, waits for the venue's acknowledgement and the channel's first frame, and
    /// unsubscribes whatever came. Gives the frame and the text it was read from, or nothing when
    /// none came within [`CHANNEL`].
    pub async fn first(&mut self, subscription: &Subscription) -> Result<Option<(Frame, String)>, String> {
        self.feed.subscribe(subscription.clone()).await.map_err(|error| error.to_string())?;
        let found = self.heard(subscription).await;
        self.feed.unsubscribe(subscription.clone()).map_err(|error| error.to_string())?;
        found
    }

    /// The acknowledgement and the first frame on the channel, in whichever order they come.
    async fn heard(&mut self, subscription: &Subscription) -> Result<Option<(Frame, String)>, String> {
        let deadline = Instant::now() + CHANNEL;
        let (mut acked, mut found) = (false, None);
        while !(acked && found.is_some()) {
            let Ok(event) = timeout_at(deadline, self.feed.next()).await else { break };
            let frame = match event.ok_or("the feed stopped")? {
                Event::Frame(frame) => *frame,
                Event::Unreadable(bad) => return Err(bad.to_string()),
                Event::Closed { reason, .. } => return Err(format!("the socket closed: {reason}")),
                Event::Opened => continue,
            };
            let text =
                timeout(CHANNEL, self.kept.recv()).await.ok().flatten().ok_or("the tap kept no text for a frame")?;
            match frame {
                Frame::Ack(ack) if ack.method == Method::Subscribe => match ack.subscription() {
                    Ok(echoed) if echoed == *subscription => acked = true,
                    _ => return Err(format!("acknowledged as {}", ack.echo.compact())),
                },
                frame if found.is_none() && channel(&text) == subscription.channel() => found = Some((frame, text)),
                _ => {}
            }
        }
        match acked {
            true => Ok(found),
            false => Err("never acknowledged".to_owned()),
        }
    }
}

/// The `channel` a frame's text names.
fn channel(text: &str) -> String {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|frame| frame["channel"].as_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}
