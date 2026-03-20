use serde_json::Value;

use crate::error::{value_type_name, IssueCode, VldError};
use crate::schema::VldSchema;

#[derive(Clone)]
pub struct ZIpNetwork {
    must_be_ipv4: bool,
    must_be_ipv6: bool,
    custom_type_error: Option<String>,
}

impl ZIpNetwork {
    pub fn new() -> Self {
        Self {
            must_be_ipv4: false,
            must_be_ipv6: false,
            custom_type_error: None,
        }
    }

    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    pub fn ipv4_only(mut self) -> Self {
        self.must_be_ipv4 = true;
        self.must_be_ipv6 = false;
        self
    }

    pub fn ipv6_only(mut self) -> Self {
        self.must_be_ipv6 = true;
        self.must_be_ipv4 = false;
        self
    }

    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "string",
            "format": "cidr"
        })
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
            let msg = self.custom_type_error.clone().unwrap_or_else(|| {
                format!("Expected CIDR string, received {}", value_type_name(value))
            });
            VldError::single_with_value(
                IssueCode::InvalidType {
                    expected: "string (cidr)".to_string(),
                    received: value_type_name(value),
                },
                msg,
                value,
            )
        })?;
        let net = s.parse::<ipnet::IpNet>().map_err(|_| {
            VldError::single_with_value(
                IssueCode::Custom {
                    code: "invalid_ip_network".to_string(),
                },
                "Invalid IP network/CIDR",
                value,
            )
        })?;
        if self.must_be_ipv4 && !matches!(net, ipnet::IpNet::V4(_)) {
            return Err(VldError::single_with_value(
                IssueCode::Custom {
                    code: "not_ipv4_network".to_string(),
                },
                "Expected IPv4 network",
                value,
            ));
        }
        if self.must_be_ipv6 && !matches!(net, ipnet::IpNet::V6(_)) {
            return Err(VldError::single_with_value(
                IssueCode::Custom {
                    code: "not_ipv6_network".to_string(),
                },
                "Expected IPv6 network",
                value,
            ));
        }
        Ok(net)
    }
}
