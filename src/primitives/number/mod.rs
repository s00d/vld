//! Number schemas: [`ZNumber`] (`f64`), [`ZInteger`] (`i8`..`u64`, with
//! [`ZInt`] = `ZInteger<i64>`) and [`ZF32`] share one check list.

use serde_json::Value;
use std::cmp::Ordering;

mod f32;
mod float;
mod int;

pub use f32::ZF32;
pub use float::ZNumber;
pub use int::{IntType, ZInt, ZInteger};

use crate::error::{IssueCode, SizeOrigin};

pub(super) const MAX_SAFE: i128 = 9_007_199_254_740_991;

/// A numeric value or bound: floats for [`ZNumber`], exact integers for
/// [`ZInteger`] (`i128` holds every `i64` and `u64`).
#[derive(Clone, Copy, Debug)]
pub(super) enum Num {
    F(f64),
    I(i128),
}

impl Num {
    pub(super) fn as_f64(self) -> f64 {
        match self {
            Num::F(f) => f,
            Num::I(i) => i as f64,
        }
    }

    /// `None` when either side is NaN — every bound check fails then.
    pub(super) fn cmp(self, other: Num) -> Option<Ordering> {
        match (self, other) {
            (Num::I(a), Num::I(b)) => Some(a.cmp(&b)),
            (a, b) => a.as_f64().partial_cmp(&b.as_f64()),
        }
    }

    pub(super) fn json(self) -> Value {
        match self {
            Num::F(f) => serde_json::json!(f),
            Num::I(i) => match (i64::try_from(i), u64::try_from(i)) {
                (Ok(v), _) => v.into(),
                (_, Ok(v)) => v.into(),
                _ => serde_json::json!(i as f64),
            },
        }
    }
}

impl std::fmt::Display for Num {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Num::F(v) => write!(f, "{}", v),
            Num::I(v) => write!(f, "{}", v),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Min(Num),
    Max(Num),
    Gt(Num),
    Lt(Num),
    Positive,
    Negative,
    NonNegative,
    NonPositive,
    Finite,
    MultipleOf(Num),
    Safe,
}

#[derive(Clone)]
pub(super) struct NumberCheck {
    pub(super) kind: Kind,
    /// Custom message; `None` renders the default for the issue code.
    pub(super) msg: Option<String>,
}

impl Kind {
    /// Stable key identifying the check category.
    pub(super) fn key(self) -> &'static str {
        match self {
            Kind::Min(_) | Kind::Gt(_) => "too_small",
            Kind::Max(_) | Kind::Lt(_) => "too_big",
            Kind::Positive => "not_positive",
            Kind::Negative => "not_negative",
            Kind::NonNegative => "not_non_negative",
            Kind::NonPositive => "not_non_positive",
            Kind::Finite => "not_finite",
            Kind::MultipleOf(_) => "not_multiple_of",
            Kind::Safe => "not_safe",
        }
    }

    /// Lower bound `(value, exclusive)` this check implies, if any.
    pub(super) fn lower(self) -> Option<(Num, bool)> {
        match self {
            Kind::Min(v) => Some((v, false)),
            Kind::Gt(v) => Some((v, true)),
            Kind::Positive => Some((Num::I(0), true)),
            Kind::NonNegative => Some((Num::I(0), false)),
            Kind::Safe => Some((Num::I(-MAX_SAFE), false)),
            _ => None,
        }
    }

    /// Upper bound `(value, exclusive)` this check implies, if any.
    pub(super) fn upper(self) -> Option<(Num, bool)> {
        match self {
            Kind::Max(v) => Some((v, false)),
            Kind::Lt(v) => Some((v, true)),
            Kind::Negative => Some((Num::I(0), true)),
            Kind::NonPositive => Some((Num::I(0), false)),
            Kind::Safe => Some((Num::I(MAX_SAFE), false)),
            _ => None,
        }
    }

    /// `None` when `n` passes, otherwise the issue code.
    pub(super) fn failure(self, n: Num) -> Option<IssueCode> {
        match self {
            Kind::Finite => match n {
                Num::F(f) if !f.is_finite() => Some(IssueCode::NotFinite),
                _ => None,
            },
            Kind::MultipleOf(v) => {
                if is_multiple_of(n, v) {
                    None
                } else {
                    Some(IssueCode::NotMultipleOf { divisor: v.json() })
                }
            }
            Kind::Safe => {
                let ok = match n {
                    Num::I(i) => (-MAX_SAFE..=MAX_SAFE).contains(&i),
                    Num::F(f) => (-(MAX_SAFE as f64)..=MAX_SAFE as f64).contains(&f),
                };
                if ok {
                    None
                } else {
                    Some(IssueCode::NotSafeInteger)
                }
            }
            _ => {
                if let Some((b, excl)) = self.lower() {
                    let ok = matches!(
                        (n.cmp(b), excl),
                        (Some(Ordering::Greater), _) | (Some(Ordering::Equal), false)
                    );
                    if !ok {
                        return Some(IssueCode::TooSmall {
                            origin: SizeOrigin::Number,
                            minimum: b.json(),
                            inclusive: !excl,
                        });
                    }
                }
                if let Some((b, excl)) = self.upper() {
                    let ok = matches!(
                        (n.cmp(b), excl),
                        (Some(Ordering::Less), _) | (Some(Ordering::Equal), false)
                    );
                    if !ok {
                        return Some(IssueCode::TooBig {
                            origin: SizeOrigin::Number,
                            maximum: b.json(),
                            inclusive: !excl,
                        });
                    }
                }
                None
            }
        }
    }
}

/// `n` is a multiple of `v`. A zero divisor never matches; floats use a
/// relative tolerance so that e.g. `0.3` is a multiple of `0.1`.
fn is_multiple_of(n: Num, v: Num) -> bool {
    match (n, v) {
        (_, Num::I(0)) => false,
        (Num::I(a), Num::I(b)) => a % b == 0,
        (n, v) => {
            let (n, v) = (n.as_f64(), v.as_f64());
            if v == 0.0 || !n.is_finite() || !v.is_finite() {
                return false;
            }
            let q = n / v;
            (q - q.round()).abs() <= 1e-9 * q.abs().max(1.0)
        }
    }
}
