use serde_json::Value;

use crate::error::{value_type_name, IssueCode, PathSegment, VldError};
use crate::schema::DynSchema;
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

    /// Name of the discriminator field.
    pub fn discriminator(&self) -> &str {
        &self.discriminator
    }

    /// Variants as `(discriminator value, schema)` pairs, in insertion order.
    pub fn variants(&self) -> impl Iterator<Item = (&Value, &dyn DynSchema)> {
        self.variants
            .iter()
            .map(|v| (&v.discriminator_value, v.schema.as_ref()))
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
            VldError::from_code_with_value(
                IssueCode::InvalidType {
                    expected: "object".to_string(),
                    received: value_type_name(value),
                },
                value,
            )
        })?;
        let at_disc = |e: VldError| e.with_prefix(PathSegment::Field(self.discriminator.clone()));

        let disc_value = obj
            .get(&self.discriminator)
            .ok_or_else(|| at_disc(VldError::from_code(IssueCode::MissingField)))?;

        for variant in &self.variants {
            if *disc_value == variant.discriminator_value {
                return variant.schema.dyn_parse(value);
            }
        }

        let options = self
            .variants
            .iter()
            .map(|v| v.discriminator_value.clone())
            .collect();
        Err(at_disc(VldError::from_code_with_value(
            IssueCode::InvalidDiscriminator { options },
            disc_value,
        )))
    }
}
