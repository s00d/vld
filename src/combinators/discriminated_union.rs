use serde_json::Value;

use crate::error::{value_type_name, IssueCode, VldError};
use crate::object::DynSchema;
use crate::schema::VldSchema;

/// Entry in a discriminated union: a discriminator value and its associated schema.
struct Variant {
    discriminator_value: Value,
    schema: Box<dyn DynSchema>,
}

/// Discriminated union: chooses a schema based on a discriminator field value.
///
/// More efficient than a regular union because it looks up the correct variant
/// by the discriminator field value instead of trying each schema in order.
///
/// Created via [`vld::discriminated_union()`](crate::discriminated_union).
///
/// # Example
/// ```ignore
/// let schema = vld::discriminated_union("type")
///     .variant("dog", vld::object().field("type", vld::literal("dog")).field("bark", vld::boolean()))
///     .variant("cat", vld::object().field("type", vld::literal("cat")).field("lives", vld::number().int()));
/// ```
pub struct ZDiscriminatedUnion {
    discriminator: String,
    variants: Vec<Variant>,
}

impl ZDiscriminatedUnion {
    pub fn new(discriminator: impl Into<String>) -> Self {
        Self {
            discriminator: discriminator.into(),
            variants: vec![],
        }
    }

    /// Add a variant: when the discriminator field equals `value`, use `schema`.
    pub fn variant<S: DynSchema + 'static>(mut self, value: impl Into<Value>, schema: S) -> Self {
        self.variants.push(Variant {
            discriminator_value: value.into(),
            schema: Box::new(schema),
        });
        self
    }

    /// Add a string variant (convenience).
    pub fn variant_str<S: DynSchema + 'static>(self, value: &str, schema: S) -> Self {
        self.variant(Value::String(value.to_string()), schema)
    }
}

impl VldSchema for ZDiscriminatedUnion {
    type Output = Value;

    fn parse_value(&self, value: &Value) -> Result<Value, VldError> {
        let obj = value.as_object().ok_or_else(|| {
            VldError::single(
                IssueCode::InvalidType {
                    expected: "object".to_string(),
                    received: value_type_name(value),
                },
                format!("Expected object, received {}", value_type_name(value)),
            )
        })?;

        let disc_value = obj.get(&self.discriminator).ok_or_else(|| {
            VldError::single(
                IssueCode::MissingField,
                format!("Missing discriminator field \"{}\"", self.discriminator),
            )
        })?;

        for variant in &self.variants {
            if *disc_value == variant.discriminator_value {
                return variant.schema.dyn_parse(value);
            }
        }

        let known: Vec<String> = self
            .variants
            .iter()
            .map(|v| format!("{}", v.discriminator_value))
            .collect();

        Err(VldError::single(
            IssueCode::Custom {
                code: "invalid_discriminator".to_string(),
            },
            format!(
                "Invalid discriminator value {}. Expected one of: {}",
                disc_value,
                known.join(", ")
            ),
        ))
    }
}
