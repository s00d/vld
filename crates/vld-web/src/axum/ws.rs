//! # `vld_web::axum::ws` — validated axum WebSocket messages
//!
//! Decode, encode and answer [`axum::extract::ws::Message`]s whose payload
//! is a JSON message (feature `axum-ws` = `axum` + axum's `ws`). The same
//! rules as [`vld_web::tungstenite`](https://docs.rs/vld-web/latest/vld_web/tungstenite/):
//! malformed JSON is a single `parse_error` issue at path `""`, and the
//! error frame body is the [422 body](crate::common#error-bodies) of the
//! HTTP extractors.
//!
//! | Function | Use case |
//! |---|---|
//! | [`decode`] | Validate an incoming text / binary frame (`None` for ping, pong and close) |
//! | [`encode`] | Validate an outgoing value against its own schema and build a text frame |
//! | [`error_message`] | A text frame with the HTTP 422 body, to answer an invalid message |
//!
//! # Example
//!
//! ```
//! use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
//! use axum::response::Response;
//! use axum::{routing::any, Router};
//! use vld_web::axum::ws::{decode, error_message};
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct Chat {
//!         pub room: String => vld::string().min(1),
//!         pub text: String => vld::string().min(1).max(500),
//!     }
//! }
//!
//! async fn chat(ws: WebSocketUpgrade) -> Response {
//!     ws.on_upgrade(handle)
//! }
//!
//! async fn handle(mut socket: WebSocket) {
//!     while let Some(Ok(msg)) = socket.recv().await {
//!         let reply = match decode::<Chat>(&msg) {
//!             Ok(Some(chat)) => Message::text(format!("{}: {}", chat.room, chat.text)),
//!             Ok(None) => continue, // ping / pong / close
//!             Err(err) => error_message(&err),
//!         };
//!         if socket.send(reply).await.is_err() {
//!             break;
//!         }
//!     }
//! }
//!
//! let app: Router = Router::new().route("/ws", any(chat));
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/web/websocket>
use axum::extract::ws::Message;
use vld::error::VldError;
use vld::schema::VldParse;

/// Validate the JSON payload of a text or binary frame.
///
/// `Ok(None)` for ping, pong and close (axum answers pings itself); `Err`
/// when the payload is not JSON or fails `T`'s schema.
pub fn decode<T: VldParse>(message: &Message) -> Result<Option<T>, VldError> {
    match message {
        Message::Text(text) => crate::ws::decode(text.as_bytes()).map(Some),
        Message::Binary(data) => crate::ws::decode(data).map(Some),
        Message::Ping(_) | Message::Pong(_) | Message::Close(_) => Ok(None),
    }
}

/// Serialize `value` to a JSON text frame after checking the JSON against
/// `T`'s own schema.
pub fn encode<T: serde::Serialize + VldParse>(value: &T) -> Result<Message, VldError> {
    crate::ws::encode(value).map(Message::text)
}

/// A text frame with the HTTP 422 body:
/// `{"error": "Validation failed", "issues": [{"path", "message", "code"}]}`.
pub fn error_message(error: &VldError) -> Message {
    Message::text(crate::ws::error_text(error))
}
