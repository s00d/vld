use serde_json::Value;

use crate::error::{FormatKind, IssueCode, VldError};
use crate::schema::VldSchema;

/// Schema for an IP network in CIDR notation (`10.0.0.0/8`). Built with `vld::ip_network()`.
#[derive(Clone)]
pub struct ZIpNetwork {
    must_be_ipv4: bool,
    must_be_ipv6: bool,
    custom_type_error: Option<String>,
}

impl ZIpNetwork {
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new() -> Self {
        Self {
            must_be_ipv4: false,
            must_be_ipv6: false,
            custom_type_error: None,
        }
    }

    /// Message used when the input is not a string.
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    /// Accept IPv4 networks only.
    pub fn ipv4_only(mut self) -> Self {
        self.must_be_ipv4 = true;
        self.must_be_ipv6 = false;
        self
    }

    /// Accept IPv6 networks only.
    pub fn ipv6_only(mut self) -> Self {
        self.must_be_ipv6 = true;
        self.must_be_ipv4 = false;
        self
    }
}

impl Default for ZIpNetwork {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZIpNetwork {
    type Output = ipnet::IpNet;

    fn parse_value(&self, value: &Value) -> Result<Self::Output, VldError> {
        let s = value.as_str().ok_or_else(|| {
            crate::primitives::type_error(&self.custom_type_error, "ip_network", value)
        })?;
        let net = s.parse::<ipnet::IpNet>().map_err(|_| {
            VldError::from_code_with_value(
                IssueCode::InvalidFormat {
                    format: FormatKind::IpNetwork,
                },
                value,
            )
        })?;
        if self.must_be_ipv4 && !matches!(net, ipnet::IpNet::V4(_)) {
            return Err(VldError::from_code_with_value(
                IssueCode::InvalidFormat {
                    format: FormatKind::Ipv4Network,
                },
                value,
            ));
        }
        if self.must_be_ipv6 && !matches!(net, ipnet::IpNet::V6(_)) {
            return Err(VldError::from_code_with_value(
                IssueCode::InvalidFormat {
                    format: FormatKind::Ipv6Network,
                },
                value,
            ));
        }
        Ok(net)
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZIpNetwork {
    fn json_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "string",
            "format": "cidr"
        })
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZIpNetwork {}
