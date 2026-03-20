//! # vld-lapin
//!
//! Lapin (RabbitMQ) integration for `vld`.
//!
//! ## Overview
//!
//! `vld-lapin` keeps one entrypoint macro:
//!
//! - `impl_to_lapin!(channel)`
//!
//! After rebinding, `channel` becomes a validating wrapper with:
//!
//! - publish helper: `publish`
//! - get/decode helpers: `basic_get`, `decode_bytes`, `decode_delivery`, `decode_get`
//! - ack helpers: `ack_decode`, `nack_decode`, `reject_decode`
//! - get+ack helpers: `ack_decode_get`, `nack_decode_get`, `reject_decode_get`
//!
//! All other native `lapin::Channel` methods are still available through deref.

use std::fmt;
use std::ops::{Deref, DerefMut};

pub use vld;

/// Lapin channel wrapper with validate+JSON behavior.
pub struct LapinChannel {
    inner: lapin::Channel,
}

impl LapinChannel {
    pub fn new(inner: lapin::Channel) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> lapin::Channel {
        self.inner
    }

    fn encode_payload<V>(value: &V) -> Result<Vec<u8>, VldLapinError>
    where
        V: serde::Serialize + vld::schema::VldParse + ?Sized,
    {
        let json = serde_json::to_value(value)
            .map_err(|e| VldLapinError::Serialization(e.to_string()))?;
        <V as vld::schema::VldParse>::vld_parse_value(&json).map_err(VldLapinError::Validation)?;
        serde_json::to_vec(&json).map_err(|e| VldLapinError::Serialization(e.to_string()))
    }

    pub fn decode_bytes<T>(&self, payload: &[u8]) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        let value: serde_json::Value = serde_json::from_slice(payload)
            .map_err(|e| VldLapinError::Deserialization(e.to_string()))?;
        <T as vld::schema::VldParse>::vld_parse_value(&value).map_err(VldLapinError::Validation)
    }

    pub async fn queue_declare(
        &self,
        queue: &str,
        options: lapin::options::QueueDeclareOptions,
        arguments: lapin::types::FieldTable,
    ) -> Result<lapin::Queue, VldLapinError> {
        self.inner
            .queue_declare(queue, options, arguments)
            .await
            .map_err(VldLapinError::Lapin)
    }

    pub async fn basic_get(
        &self,
        queue: &str,
        options: lapin::options::BasicGetOptions,
    ) -> Result<Option<lapin::message::BasicGetMessage>, VldLapinError> {
        self.inner
            .basic_get(queue, options)
            .await
            .map_err(VldLapinError::Lapin)
    }

    pub async fn publish<V>(
        &self,
        exchange: &str,
        routing_key: &str,
        options: lapin::options::BasicPublishOptions,
        properties: lapin::BasicProperties,
        value: &V,
    ) -> Result<lapin::publisher_confirm::Confirmation, VldLapinError>
    where
        V: serde::Serialize + vld::schema::VldParse + ?Sized,
    {
        let payload = Self::encode_payload(value)?;

        let confirm = self
            .inner
            .basic_publish(exchange, routing_key, options, &payload, properties)
            .await
            .map_err(VldLapinError::Lapin)?;
        confirm.await.map_err(VldLapinError::Lapin)
    }

    pub fn decode_delivery<T>(&self, delivery: &lapin::message::Delivery) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        self.decode_bytes(&delivery.data)
    }

    pub fn decode_get<T>(&self, message: &lapin::message::BasicGetMessage) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        self.decode_delivery(message)
    }

    pub async fn ack_decode<T>(
        &self,
        delivery: &lapin::message::Delivery,
        options: lapin::options::BasicAckOptions,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        let parsed = self.decode_delivery(delivery)?;
        delivery.ack(options).await.map_err(VldLapinError::Lapin)?;
        Ok(parsed)
    }

    pub async fn nack_decode<T>(
        &self,
        delivery: &lapin::message::Delivery,
        options: lapin::options::BasicNackOptions,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        let parsed = self.decode_delivery(delivery)?;
        delivery.nack(options).await.map_err(VldLapinError::Lapin)?;
        Ok(parsed)
    }

    pub async fn reject_decode<T>(
        &self,
        delivery: &lapin::message::Delivery,
        options: lapin::options::BasicRejectOptions,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        let parsed = self.decode_delivery(delivery)?;
        delivery.reject(options).await.map_err(VldLapinError::Lapin)?;
        Ok(parsed)
    }

    pub async fn ack_decode_get<T>(
        &self,
        message: &lapin::message::BasicGetMessage,
        options: lapin::options::BasicAckOptions,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        self.ack_decode(message, options).await
    }

    pub async fn nack_decode_get<T>(
        &self,
        message: &lapin::message::BasicGetMessage,
        options: lapin::options::BasicNackOptions,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        self.nack_decode(message, options).await
    }

    pub async fn reject_decode_get<T>(
        &self,
        message: &lapin::message::BasicGetMessage,
        options: lapin::options::BasicRejectOptions,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        self.reject_decode(message, options).await
    }
}

impl Deref for LapinChannel {
    type Target = lapin::Channel;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for LapinChannel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Error type for `vld-lapin`.
#[derive(Debug)]
pub enum VldLapinError {
    Validation(vld::error::VldError),
    Serialization(String),
    Deserialization(String),
    Lapin(lapin::Error),
}

impl fmt::Display for VldLapinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldLapinError::Validation(e) => write!(f, "Validation error: {e}"),
            VldLapinError::Serialization(e) => write!(f, "Serialization error: {e}"),
            VldLapinError::Deserialization(e) => write!(f, "Deserialization error: {e}"),
            VldLapinError::Lapin(e) => write!(f, "Lapin error: {e}"),
        }
    }
}

impl std::error::Error for VldLapinError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldLapinError::Validation(e) => Some(e),
            VldLapinError::Lapin(e) => Some(e),
            VldLapinError::Serialization(_) | VldLapinError::Deserialization(_) => None,
        }
    }
}

impl From<vld::error::VldError> for VldLapinError {
    fn from(value: vld::error::VldError) -> Self {
        Self::Validation(value)
    }
}

impl From<lapin::Error> for VldLapinError {
    fn from(value: lapin::Error) -> Self {
        Self::Lapin(value)
    }
}

#[macro_export]
macro_rules! impl_to_lapin {
    ($channel:ident) => {
        let $channel = $crate::LapinChannel::new($channel);
    };
}

pub mod prelude {
    pub use crate::{impl_to_lapin, LapinChannel, VldLapinError};
    pub use vld::prelude::*;
}
