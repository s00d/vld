use serde_json::Value;

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

/// Error for a union where every variant failed: one `invalid_union` issue
/// (with the received value) followed by each variant's own issues, their
/// messages prefixed with `[variant N]` (paths are kept).
///
/// A variant that is itself a union (`union!` nests them) is flattened: its
/// `invalid_union` issue is dropped and its variants are renumbered, so `N`
/// counts the leaf schemas and prefixes never stack.
pub(crate) fn union_error(value: &Value, variants: Vec<VldError>) -> VldError {
    let mut err = VldError::from_code_with_value(IssueCode::InvalidUnion, value);
    let mut n = 0;
    for e in variants {
        let nested = e
            .issues
            .first()
            .is_some_and(|i| i.code == IssueCode::InvalidUnion && i.path.is_empty());
        if !nested {
            n += 1;
            for mut issue in e.issues {
                issue.message = format!("[variant {}] {}", n, issue.message);
                err.issues.push(issue);
            }
            continue;
        }
        let mut inner_count = 0;
        for mut issue in e.issues.into_iter().skip(1) {
            if let Some((k, rest)) = split_variant_prefix(&issue.message) {
                inner_count = inner_count.max(k);
                issue.message = format!("[variant {}] {}", n + k, rest);
            }
            err.issues.push(issue);
        }
        n += inner_count.max(1);
    }
    err
}

/// `"[variant 3] msg"` → `(3, "msg")`.
pub(crate) fn split_variant_prefix(message: &str) -> Option<(usize, &str)> {
    let rest = message.strip_prefix("[variant ")?;
    let (k, rest) = rest.split_once("] ")?;
    Some((k.parse().ok()?, rest))
}

/// A value that can be one of two types.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Either<A, B> {
    /// Value from the first schema.
    Left(A),
    /// Value from the second schema.
    Right(B),
}

impl<A, B> Either<A, B> {
    /// `true` if the first schema matched.
    pub fn is_left(&self) -> bool {
        matches!(self, Either::Left(_))
    }
    /// `true` if the second schema matched.
    pub fn is_right(&self) -> bool {
        matches!(self, Either::Right(_))
    }
    /// The first schema's value, if it matched.
    pub fn left(self) -> Option<A> {
        match self {
            Either::Left(a) => Some(a),
            _ => None,
        }
    }
    /// The second schema's value, if it matched.
    pub fn right(self) -> Option<B> {
        match self {
            Either::Right(b) => Some(b),
            _ => None,
        }
    }
}

/// Union of two schemas. Tries the first, then the second.
///
/// When both fail, the error starts with an `invalid_union` issue carrying the
/// received value, followed by every variant's issues (`[variant N] ...`).
///
/// Created via [`vld::union()`](crate::union()).
pub struct ZUnion2<A: VldSchema, B: VldSchema> {
    first: A,
    second: B,
}

impl<A: VldSchema, B: VldSchema> ZUnion2<A, B> {
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new(first: A, second: B) -> Self {
        Self { first, second }
    }

    /// Access the first schema.
    pub fn schema_a(&self) -> &A {
        &self.first
    }
    /// Access the second schema.
    pub fn schema_b(&self) -> &B {
        &self.second
    }
}

impl<A: VldSchema, B: VldSchema> VldSchema for ZUnion2<A, B> {
    type Output = Either<A::Output, B::Output>;

    fn parse_value(&self, value: &Value) -> Result<Self::Output, VldError> {
        let e1 = match self.first.parse_value(value) {
            Ok(v) => return Ok(Either::Left(v)),
            Err(e) => e,
        };
        let e2 = match self.second.parse_value(value) {
            Ok(v) => return Ok(Either::Right(v)),
            Err(e) => e,
        };
        Err(union_error(value, vec![e1, e2]))
    }
}

/// A value that can be one of three types.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Either3<A, B, C> {
    /// Value from the first schema.
    First(A),
    /// Value from the second schema.
    Second(B),
    /// Value from the third schema.
    Third(C),
}

/// Union of three schemas.
///
/// Created via [`vld::union3()`](crate::union3).
pub struct ZUnion3<A: VldSchema, B: VldSchema, C: VldSchema> {
    first: A,
    second: B,
    third: C,
}

impl<A: VldSchema, B: VldSchema, C: VldSchema> ZUnion3<A, B, C> {
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new(first: A, second: B, third: C) -> Self {
        Self {
            first,
            second,
            third,
        }
    }

    /// Access the first schema.
    pub fn schema_a(&self) -> &A {
        &self.first
    }
    /// Access the second schema.
    pub fn schema_b(&self) -> &B {
        &self.second
    }
    /// Access the third schema.
    pub fn schema_c(&self) -> &C {
        &self.third
    }
}

impl<A: VldSchema, B: VldSchema, C: VldSchema> VldSchema for ZUnion3<A, B, C> {
    type Output = Either3<A::Output, B::Output, C::Output>;

    fn parse_value(&self, value: &Value) -> Result<Self::Output, VldError> {
        let e1 = match self.first.parse_value(value) {
            Ok(v) => return Ok(Either3::First(v)),
            Err(e) => e,
        };
        let e2 = match self.second.parse_value(value) {
            Ok(v) => return Ok(Either3::Second(v)),
            Err(e) => e,
        };
        let e3 = match self.third.parse_value(value) {
            Ok(v) => return Ok(Either3::Third(v)),
            Err(e) => e,
        };
        Err(union_error(value, vec![e1, e2, e3]))
    }
}
