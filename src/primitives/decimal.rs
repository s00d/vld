use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;

use crate::error::{value_type_name, IssueCode, VldError};
use crate::schema::VldSchema;

#[derive(Clone)]
pub struct ZDecimal {
    min: Option<Decimal>,
    max: Option<Decimal>,
    positive: bool,
    negative: bool,
    non_negative: bool,
    non_positive: bool,
    custom_type_error: Option<String>,
}

impl ZDecimal {
    pub fn new() -> Self {
        Self {
            min: None,
            max: None,
            positive: false,
            negative: false,
            non_negative: false,
            non_positive: false,
            custom_type_error: None,
        }
    }

    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    pub fn min(mut self, value: impl AsRef<str>) -> Self {
        let parsed = Decimal::from_str(value.as_ref())
            .unwrap_or_else(|_| panic!("Invalid decimal literal: {}", value.as_ref()));
        self.min = Some(parsed);
        self
    }

    pub fn max(mut self, value: impl AsRef<str>) -> Self {
        let parsed = Decimal::from_str(value.as_ref())
            .unwrap_or_else(|_| panic!("Invalid decimal literal: {}", value.as_ref()));
        self.max = Some(parsed);
        self
    }

    pub fn positive(mut self) -> Self {
        self.positive = true;
        self
    }

    pub fn negative(mut self) -> Self {
        self.negative = true;
        self
    }

    pub fn non_negative(mut self) -> Self {
        self.non_negative = true;
        self
    }

    pub fn non_positive(mut self) -> Self {
        self.non_positive = true;
        self
    }

    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({
            "type": "string",
            "format": "decimal"
        });
        if let Some(min) = self.min {
            schema["x-minDecimal"] = serde_json::json!(min.to_string());
        }
        if let Some(max) = self.max {
            schema["x-maxDecimal"] = serde_json::json!(max.to_string());
        }
        schema
    }
}

impl Default for ZDecimal {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZDecimal {
    type Output = Decimal;

    fn parse_value(&self, value: &Value) -> Result<Decimal, VldError> {
        let parse_from_str = |s: &str| {
            Decimal::from_str(s).map_err(|_| {
                VldError::single_with_value(
                    IssueCode::Custom {
                        code: "invalid_decimal".to_string(),
                    },
                    format!("Invalid decimal value: {}", s),
                    value,
                )
            })
        };

        let n = match value {
            Value::String(s) => parse_from_str(s)?,
            Value::Number(n) => parse_from_str(&n.to_string())?,
            _ => {
                let msg = self.custom_type_error.clone().unwrap_or_else(|| {
                    format!(
                        "Expected decimal as string or number, received {}",
                        value_type_name(value)
                    )
                });
                return Err(VldError::single_with_value(
                    IssueCode::InvalidType {
                        expected: "decimal".to_string(),
                        received: value_type_name(value),
                    },
                    msg,
                    value,
                ));
            }
        };

        if let Some(min) = self.min {
            if n < min {
                return Err(VldError::single_with_value(
                    IssueCode::TooSmall {
                        minimum: 0.0,
                        inclusive: true,
                    },
                    format!("Decimal must be at least {}", min),
                    value,
                ));
            }
        }
        if let Some(max) = self.max {
            if n > max {
                return Err(VldError::single_with_value(
                    IssueCode::TooBig {
                        maximum: 0.0,
                        inclusive: true,
                    },
                    format!("Decimal must be at most {}", max),
                    value,
                ));
            }
        }
        if self.positive && n <= Decimal::ZERO {
            return Err(VldError::single_with_value(
                IssueCode::Custom {
                    code: "not_positive".to_string(),
                },
                "Decimal must be positive",
                value,
            ));
        }
        if self.negative && n >= Decimal::ZERO {
            return Err(VldError::single_with_value(
                IssueCode::Custom {
                    code: "not_negative".to_string(),
                },
                "Decimal must be negative",
                value,
            ));
        }
        if self.non_negative && n < Decimal::ZERO {
            return Err(VldError::single_with_value(
                IssueCode::Custom {
                    code: "not_non_negative".to_string(),
                },
                "Decimal must be non-negative",
                value,
            ));
        }
        if self.non_positive && n > Decimal::ZERO {
            return Err(VldError::single_with_value(
                IssueCode::Custom {
                    code: "not_non_positive".to_string(),
                },
                "Decimal must be non-positive",
                value,
            ));
        }
        Ok(n)
    }
}
