//! # `vld_data::socketio` — Socket.IO client (socketio-next) for `vld`
//!
//! Validate Socket.IO event arguments and acks as JSON:
//!
//! - [`emit`] — validate a value and emit it as the event's one argument;
//! - [`emit_with_ack`] — validate the request, emit it, then validate the
//!   server's ack (its first argument);
//! - [`validated`] / [`validated_or`] — wrap a `ClientBuilder::on` handler so
//!   it runs only with a valid first argument;
//! - [`answer`] / [`answer_or`] — the same for events the server emits with a
//!   callback: the handler's return value is validated and sent as the ack;
//! - [`ack`] — answer a server callback with a validated value by hand;
//! - [`decode`] / [`decode_arg`] / [`decode_args`] — validate a received
//!   [`Event`]'s first argument, argument `index`, or all arguments as one
//!   array (a tuple or `Vec` schema); [`decode_payload_arg`] /
//!   [`decode_payload_args`] — the same on a [`Payload`] (an ack reply).
//!
//! Sent values are `Serialize + VldParse` (a `vld::schema!` struct with
//! `#[derive(serde::Serialize)]`). **Binary data** is not JSON: a binary
//! argument is missing from the JSON arguments ([`VldSocketIoError::NoArgument`])
//! and a buffer inside an argument is the socket.io placeholder object
//! `{"_placeholder":true,"num":n}`; read buffers with `Payload::binary`.
//!
//! **Invalid incoming data.** [`validated`] and [`answer`] never call the
//! handler with invalid arguments; they hand the event and the error to
//! [`log_invalid`] (a `tracing` warning; a server callback stays
//! unanswered). Pass your own callback to [`validated_or`] / [`answer_or`] to
//! emit an error event or ack an error instead.
//!
//! Feature `socketio` (socketio-next 0.2 with its default features off: no
//! TLS backend, so `http://` / `ws://` only — enable `socketio-next`'s
//! `rustls` or `native-tls` feature in your own `Cargo.toml` for `https`).
//! **socketio-next is 0.2: its API may change in a minor release.**
//!
//! ```no_run
//! use std::time::Duration;
//! use socketio_next::{Client, ClientBuilder};
//! use vld_data::socketio::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug, serde::Serialize)]
//!     pub struct ChatMessage {
//!         pub text: String => vld::string().min(1).max(500),
//!     }
//! }
//! vld::schema! {
//!     #[derive(Debug, serde::Serialize)]
//!     pub struct Saved {
//!         pub id: i64 => vld::number().int().min(1),
//!     }
//! }
//!
//! # async fn run() -> Result<(), VldSocketIoError> {
//! let client = ClientBuilder::new("http://localhost:3000")
//!     .on("chat", validated(|msg: ChatMessage, _client: Client| async move {
//!         println!("{}", msg.text);
//!     }))
//!     .connect()
//!     .await?;
//!
//! emit(&client, "chat", &ChatMessage { text: "hi".into() })?;
//! let saved: Saved = emit_with_ack(
//!     &client, "save", &ChatMessage { text: "keep".into() }, Duration::from_secs(5),
//! ).await?;
//! # let _ = saved; Ok(()) }
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/messaging/socketio>

use socketio_next::{Client, Event, Payload};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

pub use vld;

/// The future a wrapped handler returns.
pub type HandlerFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Serialize `value` to JSON and validate it against its own schema.
pub fn encode<V>(value: &V) -> Result<serde_json::Value, VldSocketIoError>
where
    V: serde::Serialize + vld::schema::VldParse,
{
    let json =
        serde_json::to_value(value).map_err(|e| VldSocketIoError::Serialization(e.to_string()))?;
    V::vld_parse_value(&json)?;
    Ok(json)
}

/// Validate argument `index` of a payload against `T`.
///
/// [`VldSocketIoError::NoArgument`] when there is no such JSON argument.
pub fn decode_payload_arg<T>(payload: &Payload, index: usize) -> Result<T, VldSocketIoError>
where
    T: vld::schema::VldParse,
{
    let arg = payload
        .args()
        .get(index)
        .ok_or(VldSocketIoError::NoArgument(index))?;
    Ok(T::vld_parse_value(arg)?)
}

/// Validate all arguments of a payload, as one JSON array, against `T`.
pub fn decode_payload_args<T>(payload: &Payload) -> Result<T, VldSocketIoError>
where
    T: vld::schema::VldParse,
{
    Ok(T::vld_parse_value(&serde_json::Value::Array(
        payload.args().to_vec(),
    ))?)
}

/// Validate an event's first argument against `T`.
pub fn decode<T>(event: &Event) -> Result<T, VldSocketIoError>
where
    T: vld::schema::VldParse,
{
    decode_payload_arg(event.payload(), 0)
}

/// Validate an event's argument `index` against `T`.
pub fn decode_arg<T>(event: &Event, index: usize) -> Result<T, VldSocketIoError>
where
    T: vld::schema::VldParse,
{
    decode_payload_arg(event.payload(), index)
}

/// Validate all of an event's arguments, as one JSON array, against `T`.
pub fn decode_args<T>(event: &Event) -> Result<T, VldSocketIoError>
where
    T: vld::schema::VldParse,
{
    decode_payload_args(event.payload())
}

/// Validate `value` and emit it as `event`'s one argument.
pub fn emit<V>(client: &Client, event: &str, value: &V) -> Result<(), VldSocketIoError>
where
    V: serde::Serialize + vld::schema::VldParse,
{
    let json = encode(value)?;
    Ok(client.emit(event, json)?)
}

/// Validate `req`, emit it as `event`'s one argument, wait up to `timeout`
/// for the server's ack and validate the ack's first argument as `Resp`.
pub async fn emit_with_ack<Req, Resp>(
    client: &Client,
    event: &str,
    req: &Req,
    timeout: Duration,
) -> Result<Resp, VldSocketIoError>
where
    Req: serde::Serialize + vld::schema::VldParse,
    Resp: vld::schema::VldParse,
{
    let json = encode(req)?;
    let reply = client.emit_with_ack(event, json, timeout).await?;
    decode_payload_arg(&reply, 0)
}

/// Validate `value` and send it as the ack of a server callback.
///
/// [`VldSocketIoError::NoAck`] when the server emitted `event` without a
/// callback. Only the first ack of an event is sent (socket.io-client rule).
pub fn ack<V>(event: &Event, value: &V) -> Result<(), VldSocketIoError>
where
    V: serde::Serialize + vld::schema::VldParse,
{
    if !event.wants_ack() {
        return Err(VldSocketIoError::NoAck);
    }
    let json = encode(value)?;
    Ok(event.ack(json)?)
}

/// The default reaction to invalid data in [`validated`] / [`answer`]: a
/// `tracing` warning with the event name and the error.
pub async fn log_invalid(event: Event, _client: Client, error: VldSocketIoError) {
    tracing::warn!(event = event.name(), %error, "socket.io event rejected by vld");
}

/// Wrap a handler for `ClientBuilder::on` / `Client::on`: it runs only when
/// the event's first argument is a valid `T`; otherwise [`log_invalid`].
pub fn validated<T, F, Fut>(handler: F) -> impl Fn(Event, Client) -> HandlerFuture + Send + Sync
where
    T: vld::schema::VldParse + Send + 'static,
    F: Fn(T, Client) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    validated_or(handler, log_invalid)
}

/// [`validated`] with your own reaction to invalid data: `on_invalid`
/// receives the event (to ack or inspect it), the client (to emit an error
/// event) and the error.
pub fn validated_or<T, F, Fut, E, EFut>(
    handler: F,
    on_invalid: E,
) -> impl Fn(Event, Client) -> HandlerFuture + Send + Sync
where
    T: vld::schema::VldParse + Send + 'static,
    F: Fn(T, Client) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
    E: Fn(Event, Client, VldSocketIoError) -> EFut + Send + Sync + 'static,
    EFut: Future<Output = ()> + Send + 'static,
{
    move |event: Event, client: Client| -> HandlerFuture {
        match decode::<T>(&event) {
            Ok(value) => Box::pin(handler(value, client)),
            Err(error) => Box::pin(on_invalid(event, client, error)),
        }
    }
}

/// Wrap a handler for an event the server emits with a callback: it runs
/// only when the first argument is a valid `T`, and its return value is
/// validated and sent as the ack. Invalid input, an invalid return value or
/// a missing callback ([`VldSocketIoError::NoAck`]) go to [`log_invalid`].
pub fn answer<T, R, F, Fut>(handler: F) -> impl Fn(Event, Client) -> HandlerFuture + Send + Sync
where
    T: vld::schema::VldParse + Send + 'static,
    R: serde::Serialize + vld::schema::VldParse + Send + 'static,
    F: Fn(T, Client) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
{
    answer_or(handler, log_invalid)
}

/// [`answer`] with your own reaction to invalid data (see [`validated_or`]);
/// the event still holds the server's callback, so `on_invalid` can ack an
/// error value.
pub fn answer_or<T, R, F, Fut, E, EFut>(
    handler: F,
    on_invalid: E,
) -> impl Fn(Event, Client) -> HandlerFuture + Send + Sync
where
    T: vld::schema::VldParse + Send + 'static,
    R: serde::Serialize + vld::schema::VldParse + Send + 'static,
    F: Fn(T, Client) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    E: Fn(Event, Client, VldSocketIoError) -> EFut + Clone + Send + Sync + 'static,
    EFut: Future<Output = ()> + Send + 'static,
{
    move |event: Event, client: Client| -> HandlerFuture {
        let value = match decode::<T>(&event) {
            Ok(value) => value,
            Err(error) => return Box::pin(on_invalid(event, client, error)),
        };
        let reply = handler(value, client.clone());
        let on_invalid = on_invalid.clone();
        Box::pin(async move {
            let reply = reply.await;
            if let Err(error) = ack(&event, &reply) {
                on_invalid(event, client, error).await;
            }
        })
    }
}

/// Error type of the socketio module.
#[derive(Debug)]
#[non_exhaustive]
pub enum VldSocketIoError {
    /// The value or argument failed schema validation.
    Validation(vld::error::VldError),
    /// The value could not be serialized to JSON.
    Serialization(String),
    /// The event or ack has no JSON argument at this index.
    NoArgument(usize),
    /// [`ack`] was given an event the server emitted without a callback.
    NoAck,
    /// The client failed (not connected, closed, ack timeout, …).
    SocketIo(socketio_next::Error),
}

impl fmt::Display for VldSocketIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldSocketIoError::Validation(e) => write!(f, "Validation error: {e}"),
            VldSocketIoError::Serialization(e) => write!(f, "Serialization error: {e}"),
            VldSocketIoError::NoArgument(i) => write!(f, "No JSON argument at index {i}"),
            VldSocketIoError::NoAck => write!(f, "The server did not ask for an ack"),
            VldSocketIoError::SocketIo(e) => write!(f, "Socket.IO error: {e}"),
        }
    }
}

impl std::error::Error for VldSocketIoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldSocketIoError::Validation(e) => Some(e),
            VldSocketIoError::SocketIo(e) => Some(e),
            VldSocketIoError::Serialization(_)
            | VldSocketIoError::NoArgument(_)
            | VldSocketIoError::NoAck => None,
        }
    }
}

impl From<vld::error::VldError> for VldSocketIoError {
    fn from(e: vld::error::VldError) -> Self {
        Self::Validation(e)
    }
}

impl From<socketio_next::Error> for VldSocketIoError {
    fn from(e: socketio_next::Error) -> Self {
        Self::SocketIo(e)
    }
}

/// Common imports: the functions, `VldSocketIoError` and `vld::prelude`.
pub mod prelude {
    pub use super::{
        VldSocketIoError, ack, answer, answer_or, decode, decode_arg, decode_args,
        decode_payload_arg, decode_payload_args, emit, emit_with_ack, encode, log_invalid,
        validated, validated_or,
    };
    pub use vld::prelude::*;
}
