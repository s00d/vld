//! [`FakeSocketIo`]: a minimal Socket.IO v4 server on a localhost websocket
//! for one client. It sends the Engine.IO open packet, answers every
//! Socket.IO CONNECT (any namespace) with a sid, and otherwise relays:
//! [`FakeSocketIo::send`] writes a Socket.IO packet (`2["ev",…]`, `3<id>[…]`)
//! to the client, [`FakeSocketIo::recv`] returns the next packet the client
//! sent, without the Engine.IO `4` prefix. Engine.IO pings are not sent (the
//! intervals outlast any test).

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

const OPEN: &str = r#"0{"sid":"e1","upgrades":[],"pingInterval":300000,"pingTimeout":300000,"maxPayload":1000000}"#;

pub struct FakeSocketIo {
    /// Connect with `TransportType::Websocket`.
    pub url: String,
    to_client: mpsc::UnboundedSender<Message>,
    from_client: mpsc::UnboundedReceiver<String>,
}

impl FakeSocketIo {
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (to_client, mut outgoing) = mpsc::unbounded_channel::<Message>();
        let (incoming, from_client) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            ws.send(Message::text(OPEN)).await.unwrap();
            loop {
                tokio::select! {
                    msg = ws.next() => {
                        let Some(Ok(Message::Text(text))) = msg else { break };
                        let text = text.as_str();
                        if let Some(rest) = text.strip_prefix("40") {
                            // CONNECT `40[/nsp,][{auth}]` -> `40[/nsp,]{"sid":…}`
                            let nsp = match rest.find(',') {
                                Some(i) if rest.starts_with('/') => &rest[..=i],
                                _ => "",
                            };
                            let reply = format!(r#"40{nsp}{{"sid":"s1"}}"#);
                            ws.send(Message::text(reply)).await.unwrap();
                        } else if let Some(packet) = text.strip_prefix('4') {
                            let _ = incoming.send(packet.to_owned());
                        }
                    }
                    out = outgoing.recv() => {
                        let Some(out) = out else { break };
                        ws.send(out).await.unwrap();
                    }
                }
            }
        });
        Self {
            url,
            to_client,
            from_client,
        }
    }

    /// Send a Socket.IO packet (`2["ev",…]`) to the client.
    pub fn send(&self, packet: &str) {
        self.to_client
            .send(Message::text(format!("4{packet}")))
            .unwrap();
    }

    /// Send a binary attachment frame (after a `5<n>-…` packet).
    pub fn send_binary(&self, bytes: &[u8]) {
        self.to_client
            .send(Message::binary(bytes.to_vec()))
            .unwrap();
    }

    /// The next Socket.IO packet from the client.
    pub async fn recv(&mut self) -> String {
        self.from_client.recv().await.unwrap()
    }
}
