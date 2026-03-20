use serde_json::Value;
use std::net::SocketAddr;

use crate::error::{value_type_name, IssueCode, VldError};
use crate::schema::VldSchema;

#[derive(Clone)]
pub struct ZSocketAddr {
    min_port: Option<u16>,
    max_port: Option<u16>,
    custom_type_error: Option<String>,
}

impl ZSocketAddr {
    pub fn new() -> Self {
        Self {
            min_port: None,
            max_port: None,
            custom_type_error: None,
        }
    }

    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    pub fn min_port(mut self, p: u16) -> Self {
        self.min_port = Some(p);
        self
    }

    pub fn max_port(mut self, p: u16) -> Self {
        self.max_port = Some(p);
        self
    }

    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "string",
            "format": "socket-addr"
        })
    }
}

impl Default for ZSocketAddr {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZSocketAddr {
    type Output = SocketAddr;

    fn parse_value(&self, value: &Value) -> Result<Self::Output, VldError> {
        let s = value.as_str().ok_or_else(|| {
            let msg = self.custom_type_error.clone().unwrap_or_else(|| {
                format!(
                    "Expected socket address string, received {}",
                    value_type_name(value)
                )
            });
            VldError::single_with_value(
                IssueCode::InvalidType {
                    expected: "string (socket address)".to_string(),
                    received: value_type_name(value),
                },
                msg,
                value,
            )
        })?;
        let addr = s.parse::<SocketAddr>().map_err(|_| {
            VldError::single_with_value(
                IssueCode::Custom {
                    code: "invalid_socket_addr".to_string(),
                },
                "Invalid socket address (expected `host:port` or `[ipv6]:port`)",
                value,
            )
        })?;
        let port = addr.port();
        if let Some(min) = self.min_port {
            if port < min {
                return Err(VldError::single_with_value(
                    IssueCode::TooSmall {
                        minimum: min as f64,
                        inclusive: true,
                    },
                    format!("Port must be at least {}", min),
                    value,
                ));
            }
        }
        if let Some(max) = self.max_port {
            if port > max {
                return Err(VldError::single_with_value(
                    IssueCode::TooBig {
                        maximum: max as f64,
                        inclusive: true,
                    },
                    format!("Port must be at most {}", max),
                    value,
                ));
            }
        }
        Ok(addr)
    }
}
