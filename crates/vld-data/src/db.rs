//! Shared building blocks for the database modules (`sqlx08` / `sqlx09`,
//! `diesel`, `sea1` / `sea2`, `surrealdb`).
//!
//! The types themselves are generated *inside* each module by the macros
//! below, so every module has its own error type and column wrappers and can
//! implement its database traits (`sqlx::Type`, `diesel::ToSql`, …) for them.
//! Crate-internal: the macros are not exported.

/// Validate `{"value": v}` against schema `S` (used by the column wrappers).
///
/// The wrapped scalars never serialize to `null` except a non-finite float
/// (`NaN` / `±inf`), which is rejected with `not_finite` instead of being
/// checked as a missing value.
pub(crate) fn check_value<S: vld::schema::VldParse>(
    v: serde_json::Value,
) -> Result<(), vld::error::VldError> {
    if v.is_null() {
        return Err(vld::error::VldError::from_code(
            vld::error::IssueCode::NotFinite,
        ));
    }
    S::vld_parse_value(&serde_json::json!({ "value": v })).map(|_| ())
}

/// Generate the crate's error enum (`Validation` / `Serialization`) plus a
/// crate-private `__vld_validate::<S, T>(&T)` helper.
macro_rules! __vld_db_error {
    ($(#[$m:meta])* $Err:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone)]
        pub enum $Err {
            /// Schema validation failed.
            Validation(::vld::error::VldError),
            /// Failed to serialize the value to JSON for validation.
            Serialization(String),
        }

        impl ::std::fmt::Display for $Err {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                match self {
                    $Err::Validation(e) => write!(f, "Validation error: {}", e),
                    $Err::Serialization(e) => write!(f, "Serialization error: {}", e),
                }
            }
        }

        impl ::std::error::Error for $Err {
            fn source(&self) -> Option<&(dyn ::std::error::Error + 'static)> {
                match self {
                    $Err::Validation(e) => Some(e),
                    $Err::Serialization(_) => None,
                }
            }
        }

        impl From<::vld::error::VldError> for $Err {
            fn from(e: ::vld::error::VldError) -> Self {
                $Err::Validation(e)
            }
        }

        /// Serialize `value` to JSON and validate it against `S`.
        #[allow(dead_code)]
        fn __vld_validate<S, T>(value: &T) -> Result<S, $Err>
        where
            S: ::vld::schema::VldParse,
            T: ::serde::Serialize + ?Sized,
        {
            let json = ::serde_json::to_value(value)
                .map_err(|e| $Err::Serialization(e.to_string()))?;
            S::vld_parse_value(&json).map_err($Err::Validation)
        }
    };
}
pub(crate) use __vld_db_error;

/// Generate `Validated<S, T>` in the calling crate. Requires `__vld_db_error!`.
macro_rules! __vld_db_validated {
    ($Err:ident) => {
        /// A wrapper that proves its inner value has been validated against schema `S`.
        ///
        /// `T` must be `serde::Serialize` so it can be converted to JSON for
        /// validation. Serializes transparently as the inner value.
        ///
        /// **The original `T` is stored, not the schema's parsed output**:
        /// defaults, transforms and coercions of `S` (`with_default`,
        /// `trim()`, `coerce()`, …) are checked against but never applied to
        /// the stored value. Normalize `T` yourself (or parse with `S` and
        /// build `T` from its output) when the stored value must be the
        /// normalized one.
        pub struct Validated<S, T> {
            inner: T,
            _schema: ::std::marker::PhantomData<S>,
        }

        impl<S, T> Validated<S, T>
        where
            S: ::vld::schema::VldParse,
            T: ::serde::Serialize,
        {
            /// Validate `value` against schema `S` and wrap it on success.
            pub fn new(value: T) -> Result<Self, $Err> {
                __vld_validate::<S, T>(&value)?;
                Ok(Self {
                    inner: value,
                    _schema: ::std::marker::PhantomData,
                })
            }
        }

        impl<S, T> Validated<S, T> {
            /// Get a reference to the validated inner value.
            pub fn inner(&self) -> &T {
                &self.inner
            }

            /// Consume and return the inner value.
            pub fn into_inner(self) -> T {
                self.inner
            }
        }

        impl<S, T> ::std::ops::Deref for Validated<S, T> {
            type Target = T;
            fn deref(&self) -> &T {
                &self.inner
            }
        }

        impl<S, T: ::std::fmt::Debug> ::std::fmt::Debug for Validated<S, T> {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.debug_struct("Validated")
                    .field("inner", &self.inner)
                    .finish()
            }
        }

        impl<S, T: Clone> Clone for Validated<S, T> {
            fn clone(&self) -> Self {
                Self {
                    inner: self.inner.clone(),
                    _schema: ::std::marker::PhantomData,
                }
            }
        }

        impl<S, T: ::serde::Serialize> ::serde::Serialize for Validated<S, T> {
            fn serialize<Ser: ::serde::Serializer>(
                &self,
                serializer: Ser,
            ) -> Result<Ser::Ok, Ser::Error> {
                self.inner.serialize(serializer)
            }
        }
    };
}
pub(crate) use __vld_db_validated;

/// Generate one validated scalar column wrapper (internal).
macro_rules! __vld_db_scalar {
    ($Err:ident, $(#[$m:meta])* $Name:ident, $inner:ty, copy) => {
        $crate::db::__vld_db_scalar!(@base $Err, $(#[$m])* $Name, $inner);
        impl<S> Clone for $Name<S> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<S> Copy for $Name<S> {}
        impl<S> $Name<S> {
            /// Get the inner value.
            pub fn get(&self) -> $inner {
                self.value
            }
        }
        impl<S> ::std::fmt::Debug for $Name<S> {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}({:?})", stringify!($Name), self.value)
            }
        }
    };
    (@base $Err:ident, $(#[$m:meta])* $Name:ident, $inner:ty) => {
        $(#[$m])*
        pub struct $Name<S> {
            value: $inner,
            _schema: ::std::marker::PhantomData<S>,
        }

        impl<S: ::vld::schema::VldParse> $Name<S> {
            /// Create a validated value; `input` is checked as `{"value": input}` against `S`.
            pub fn new(input: impl Into<$inner>) -> Result<Self, $Err> {
                let value: $inner = input.into();
                $crate::db::check_value::<S>(::serde_json::json!(value))
                    .map_err($Err::Validation)?;
                Ok(Self::new_unchecked(value))
            }
        }

        impl<S> $Name<S> {
            /// Create without validation (e.g. for data loaded from a trusted DB).
            pub fn new_unchecked(input: impl Into<$inner>) -> Self {
                Self {
                    value: input.into(),
                    _schema: ::std::marker::PhantomData,
                }
            }
        }

        impl<S> PartialEq for $Name<S> {
            fn eq(&self, other: &Self) -> bool {
                self.value == other.value
            }
        }

        impl<S> ::std::ops::Deref for $Name<S> {
            type Target = $inner;
            fn deref(&self) -> &$inner {
                &self.value
            }
        }

        impl<S> ::std::fmt::Display for $Name<S> {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::fmt::Display::fmt(&self.value, f)
            }
        }

        impl<S> ::serde::Serialize for $Name<S> {
            fn serialize<Ser: ::serde::Serializer>(
                &self,
                serializer: Ser,
            ) -> Result<Ser::Ok, Ser::Error> {
                ::serde::Serialize::serialize(&self.value, serializer)
            }
        }

        impl<'de, S: ::vld::schema::VldParse>
            ::serde::Deserialize<'de> for $Name<S>
        {
            fn deserialize<D: ::serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                let v = <$inner as ::serde::Deserialize>::deserialize(
                    deserializer,
                )?;
                Self::new(v).map_err(<D::Error as ::serde::de::Error>::custom)
            }
        }
    };
}
pub(crate) use __vld_db_scalar;

/// Generate `VldText`, `VldInt`, `VldFloat`, `VldBool` column wrappers in the
/// calling crate. Each takes optional extra attributes (e.g. diesel derives).
/// Requires `__vld_db_error!`.
macro_rules! __vld_db_wrappers {
    (
        $Err:ident;
        text: [$(#[$tm:meta])*]
        int: [$(#[$im:meta])*]
        float: [$(#[$fm:meta])*]
        bool: [$(#[$bm:meta])*]
    ) => {
        $crate::db::__vld_db_scalar!(@base $Err,
            /// A validated text column: a `String` that passed schema `S`
            /// (checked as `{"value": "..."}`; the schema needs a `value` field).
            $(#[$tm])*
            VldText, String);

        impl<S> Clone for VldText<S> {
            fn clone(&self) -> Self {
                Self::new_unchecked(self.value.clone())
            }
        }
        impl<S> Eq for VldText<S> {}
        impl<S> PartialOrd for VldText<S> {
            fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl<S> Ord for VldText<S> {
            fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
                self.value.cmp(&other.value)
            }
        }
        impl<S> ::std::hash::Hash for VldText<S> {
            fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                self.value.hash(state);
            }
        }
        impl<S> VldText<S> {
            /// Get the inner string.
            pub fn as_str(&self) -> &str {
                &self.value
            }
            /// Consume and return the inner `String`.
            pub fn into_inner(self) -> String {
                self.value
            }
        }
        impl<S> AsRef<str> for VldText<S> {
            fn as_ref(&self) -> &str {
                &self.value
            }
        }
        impl<S> ::std::fmt::Debug for VldText<S> {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "VldText({:?})", self.value)
            }
        }

        $crate::db::__vld_db_scalar!($Err,
            /// A validated integer column: an `i64` that passed schema `S`
            /// (checked as `{"value": n}`).
            $(#[$im])*
            VldInt, i64, copy);
        impl<S> Eq for VldInt<S> {}
        impl<S> PartialOrd for VldInt<S> {
            fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl<S> Ord for VldInt<S> {
            fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
                self.value.cmp(&other.value)
            }
        }
        impl<S> ::std::hash::Hash for VldInt<S> {
            fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                self.value.hash(state);
            }
        }

        $crate::db::__vld_db_scalar!($Err,
            /// A validated float column: an `f64` that passed schema `S`
            /// (checked as `{"value": x}`).
            $(#[$fm])*
            VldFloat, f64, copy);
        impl<S> PartialOrd for VldFloat<S> {
            fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
                self.value.partial_cmp(&other.value)
            }
        }

        $crate::db::__vld_db_scalar!($Err,
            /// A validated boolean column: a `bool` that passed schema `S`
            /// (checked as `{"value": b}`).
            $(#[$bm])*
            VldBool, bool, copy);
        impl<S> Eq for VldBool<S> {}
        impl<S> PartialOrd for VldBool<S> {
            fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl<S> Ord for VldBool<S> {
            fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
                self.value.cmp(&other.value)
            }
        }
        impl<S> ::std::hash::Hash for VldBool<S> {
            fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                self.value.hash(state);
            }
        }
    };
}
pub(crate) use __vld_db_wrappers;
