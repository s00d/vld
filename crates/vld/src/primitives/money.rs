use rust_decimal::Decimal;
use serde_json::Value;
use std::sync::Arc;

use crate::error::{IssueCode, PathSegment, StringValidation, VldError, absent_field_error};
use crate::primitives::ZDecimal;
use crate::primitives::decimal::scale_too_big;
use crate::primitives::minor_units;
use crate::schema::VldSchema;

/// An amount of money: the output of [`vld::money()`](crate::money).
///
/// Serializes as `{"amount": "12.50", "currency": "USD"}` (`rust_decimal`
/// writes the amount as a string, keeping its scale).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Money {
    /// The amount, at most the currency's minor units after the point.
    pub amount: Decimal,
    /// ISO 4217 code (`"USD"`).
    pub currency: String,
}

/// Schema for `{"amount": <decimal>, "currency": "<ISO 4217 code>"}` into
/// [`Money`]. Created via [`vld::money()`](crate::money).
///
/// - `currency`: an active ISO 4217 code with minor units (uppercase:
///   `"USD"`, `"JPY"`, `"BHD"`), else `invalid_string` (`currency`) at
///   `.currency`; with [`currencies`](Self::currencies) also one of the
///   allowed codes (`invalid_enum_value`).
/// - `amount`: a decimal string or number checked by the amount schema
///   ([`amount`](Self::amount), default `vld::decimal()`), then its scale **as
///   written** against the currency's minor units: USD 2, JPY 0, BHD 3, so
///   `"10.5"` and `"10.50"` are valid USD but `"10.500"` is `too_big`
///   (origin `scale`, maximum 2) at `.amount`.
///
/// Missing fields are `missing_field`; issues of both fields are reported.
///
/// ```
/// use vld::prelude::*;
///
/// let m = vld::money().parse(r#"{"amount": "12.50", "currency": "USD"}"#).unwrap();
/// assert_eq!(m.amount.to_string(), "12.50");
/// let err = vld::money().parse(r#"{"amount": "1.5", "currency": "JPY"}"#).unwrap_err();
/// assert_eq!(err.issues[0].path_string(), ".amount");
/// assert_eq!(err.issues[0].code.key(), "too_big");
/// ```
#[derive(Clone)]
pub struct ZMoney {
    amount: ZDecimal,
    currencies: Option<Vec<String>>,
    messages: crate::primitives::Messages,
}

impl ZMoney {
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new() -> Self {
        Self {
            amount: ZDecimal::new(),
            currencies: None,
            messages: None,
        }
    }

    /// Check the amount with `schema` (e.g. `vld::decimal().positive()`)
    /// before the currency's scale rule. Replaces messages set earlier with
    /// [`with_messages`](Self::with_messages) for the amount.
    pub fn amount(mut self, schema: ZDecimal) -> Self {
        self.amount = schema;
        self
    }

    /// Allow only these currency codes (each must be an ISO 4217 code).
    ///
    /// # Panics
    /// On a code missing from the ISO 4217 table.
    pub fn currencies(mut self, codes: &[&str]) -> Self {
        for c in codes {
            assert!(minor_units(c).is_some(), "Unknown ISO 4217 code: {}", c);
        }
        self.currencies = Some(codes.iter().map(|c| c.to_string()).collect());
        self
    }

    /// Override messages by check key: `"currency"` (unknown or not allowed
    /// code) plus the amount's [`ZDecimal::with_messages`] keys (the
    /// currency scale rule uses `"max_scale"`).
    pub fn with_messages<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        let f = Arc::new(f);
        let g = f.clone();
        self.amount = self.amount.with_messages(move |k| g(k));
        self.messages = Some(f);
        self
    }

    /// The currency code and its minor units.
    fn currency(&self, value: &Value) -> Result<(String, u32), VldError> {
        let code = value
            .as_str()
            .ok_or_else(|| crate::primitives::type_error(&None, "string", value))?;
        let msg = self.messages.as_ref().and_then(|f| f("currency"));
        let units = minor_units(code).ok_or_else(|| {
            let code = IssueCode::InvalidString {
                validation: StringValidation::Currency,
            };
            crate::error::issue_error(code, msg.as_ref(), value)
        })?;
        if let Some(allowed) = self
            .currencies
            .as_ref()
            .filter(|a| !a.iter().any(|c| c == code))
        {
            let code = IssueCode::InvalidEnumValue {
                options: allowed.clone(),
            };
            return Err(crate::error::issue_error(code, msg.as_ref(), value));
        }
        Ok((code.to_string(), units))
    }
}

impl Default for ZMoney {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse field `key` of `obj` with `parse`; an absent key is `missing_field`.
fn field<T>(
    obj: &serde_json::Map<String, Value>,
    key: &str,
    parse: impl Fn(&Value) -> Result<T, VldError>,
) -> Result<T, VldError> {
    match obj.get(key) {
        Some(v) => parse(v),
        None => Err(absent_field_error(
            parse(&Value::Null).err().unwrap_or_default(),
        )),
    }
    .map_err(|e| e.with_prefix(PathSegment::Field(key.to_string())))
}

impl VldSchema for ZMoney {
    type Output = Money;

    fn parse_value(&self, value: &Value) -> Result<Money, VldError> {
        let obj = crate::error::expect_object(value)?;
        let currency = field(obj, "currency", |v| self.currency(v));
        let amount = field(obj, "amount", |v| {
            let n = self.amount.parse_value(v)?;
            match &currency {
                Ok((_, units)) if n.scale() > *units => {
                    Err(self.amount.fail("max_scale", scale_too_big(*units), v))
                }
                _ => Ok(n),
            }
        });
        match (amount, currency) {
            (Ok(amount), Ok((currency, _))) => Ok(Money { amount, currency }),
            (a, c) => Err(a
                .err()
                .unwrap_or_default()
                .merge(c.err().unwrap_or_default())),
        }
    }
}

/// `{"type": "object", "properties": {"amount": <decimal>, "currency":
/// {"type": "string", "format": "currency-code"}},
/// "required": ["amount", "currency"], "x-money": true}`; with
/// `currencies(...)` the currency is an `enum`. The scale rule per currency
/// is not expressible in JSON Schema (`x-money` marks it; `vld::fake`
/// honours it).
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZMoney {
    fn json_schema(&self) -> Value {
        let currency = match &self.currencies {
            Some(c) => serde_json::json!({"type": "string", "enum": c}),
            None => serde_json::json!({"type": "string", "format": "currency-code"}),
        };
        serde_json::json!({
            "type": "object",
            "properties": {"amount": self.amount.json_schema(), "currency": currency},
            "required": ["amount", "currency"],
            "x-money": true,
        })
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZMoney {}
