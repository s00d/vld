//! # `vld_web::tungstenite` — validated WebSocket messages for `vld`
//!
//! Decode, encode and answer [`tungstenite::Message`]s whose payload is a
//! JSON message. Works with tungstenite 0.26 – 0.30 (the dependency is
//! `>=0.26, <0.31`, no default features), so it serves
//! [tokio-tungstenite](https://docs.rs/tokio-tungstenite) and
//! [async-tungstenite](https://docs.rs/async-tungstenite) too: both
//! re-export the same `Message` type (`tokio_tungstenite::tungstenite::Message`)
//! — use the tungstenite version your WebSocket crate depends on.
//!
//! | Function | Use case |
//! |---|---|
//! | [`decode`] | Validate an incoming text / binary frame (`None` for ping, pong, close and raw frames) |
//! | [`encode`] | Validate an outgoing value against its own schema and build a text frame |
//! | [`error_message`] | A text frame with the HTTP 422 body, to answer an invalid message |
//!
//! Malformed JSON is a single `parse_error` issue at path `""`, as for HTTP
//! JSON bodies; the error frame body is the
//! [422 body](crate::common#error-bodies) of the HTTP modules.
//!
//! # Example
//!
//! ```
//! use tungstenite::Message;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct Chat {
//!         pub room: String => vld::string().min(1),
//!         pub text: String => vld::string().min(1).max(500),
//!     }
//! }
//!
//! let incoming = Message::text(r#"{"room": "general", "text": "hi"}"#);
//! let chat: Chat = vld_web::tungstenite::decode(&incoming).unwrap().unwrap();
//! assert_eq!(chat.room, "general");
//!
//! let bad = Message::text(r#"{"room": "", "text": "hi"}"#);
//! let err = vld_web::tungstenite::decode::<Chat>(&bad).unwrap_err();
//! let reply = vld_web::tungstenite::error_message(&err); // send it back
//! assert!(reply.to_text().unwrap().contains(r#""path":".room""#));
//!
//! assert!(vld_web::tungstenite::decode::<Chat>(&Message::Ping(Default::default()))
//!     .unwrap()
//!     .is_none());
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/web/websocket>
use tungstenite::Message;
use vld::error::VldError;
use vld::schema::VldParse;

/// Validate the JSON payload of a text or binary frame.
///
/// `Ok(None)` for ping, pong, close and raw frames (let the WebSocket
/// library handle them); `Err` when the payload is not JSON or fails `T`'s
/// schema.
pub fn decode<T: VldParse>(message: &Message) -> Result<Option<T>, VldError> {
    match message {
        Message::Text(text) => crate::ws::decode(text.as_bytes()).map(Some),
        Message::Binary(data) => crate::ws::decode(data).map(Some),
        Message::Ping(_) | Message::Pong(_) | Message::Close(_) | Message::Frame(_) => Ok(None),
    }
}

/// Serialize `value` to a JSON text frame after checking the JSON against
/// `T`'s own schema (so a server never sends what its clients would reject).
pub fn encode<T: serde::Serialize + VldParse>(value: &T) -> Result<Message, VldError> {
    crate::ws::encode(value).map(Message::text)
}

/// A text frame with the HTTP 422 body:
/// `{"error": "Validation failed", "issues": [{"path", "message", "code"}]}`.
pub fn error_message(error: &VldError) -> Message {
    Message::text(crate::ws::error_text(error))
}

/// Prelude — import everything you need.
pub mod prelude {
    pub use super::{decode, encode, error_message};
    pub use vld::prelude::*;
}
