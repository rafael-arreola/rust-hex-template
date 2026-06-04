use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

// ---------------------------------------------------------------------------
// DomainIdValue — trait for types usable as the inner value of a DomainId
// ---------------------------------------------------------------------------

/// Types that can be stored inside a [`DomainId`].
///
/// Implemented out-of-the-box for `String`, `i64`, `u64`, `i32`, and `u32`.
/// Implement this trait for your own type if you need a custom ID value
/// (e.g. a UUID newtype).
pub trait DomainIdValue:
    Clone + Debug + Display + PartialEq + Eq + Hash + Send + Sync + 'static
{
    /// Attempt to parse from a string representation.
    ///
    /// Used by [`DomainId::parse`] to dynamically construct a typed ID from
    /// path parameters, query strings, or any other string input.
    fn parse_from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized;
}

// ---------------------------------------------------------------------------
// Built-in implementations
// ---------------------------------------------------------------------------

impl DomainIdValue for String {
    fn parse_from_str(s: &str) -> Result<Self, String> {
        Ok(s.to_owned())
    }
}

macro_rules! impl_domain_id_value_for_int {
    ($t:ty) => {
        impl DomainIdValue for $t {
            fn parse_from_str(s: &str) -> Result<Self, String> {
                s.parse::<$t>()
                    .map_err(|e| format!("invalid {} value '{}': {}", stringify!($t), s, e))
            }
        }
    };
}

impl_domain_id_value_for_int!(i64);
impl_domain_id_value_for_int!(u64);
impl_domain_id_value_for_int!(i32);
impl_domain_id_value_for_int!(u32);

// ---------------------------------------------------------------------------
// DomainId
// ---------------------------------------------------------------------------

/// Type-safe domain identifier parameterized by a marker type `T` and an
/// optional inner value type `V` (defaults to [`String`]).
///
/// ```ignore
/// #[derive(Debug, Clone)]
/// pub struct UserMarker;
/// pub type UserId = DomainId<UserMarker>;             // String-backed
/// pub type NumericUserId = DomainId<UserMarker, i64>; // i64-backed
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DomainId<T, V = String>
where
    V: DomainIdValue,
{
    id: V,
    _marker: PhantomData<T>,
}

impl<T, V: DomainIdValue> DomainId<T, V> {
    /// Construct from the inner value directly.
    ///
    /// ```ignore
    /// let id = UserId::new("usr_abc123");
    /// let num_id = NumericUserId::new(42_i64);
    /// ```
    pub fn new(id: impl Into<V>) -> Self {
        Self { id: id.into(), _marker: PhantomData }
    }

    /// Attempt to parse from a string representation (dynamic parsing).
    ///
    /// Useful for converting HTTP path/query parameters to typed IDs.
    ///
    /// ```ignore
    /// let id: UserId = UserId::parse("usr_abc123").unwrap();
    /// let num_id: NumericUserId = NumericUserId::parse("42").unwrap();
    /// ```
    pub fn parse(s: &str) -> Result<Self, String> {
        Ok(Self { id: V::parse_from_str(s)?, _marker: PhantomData })
    }

    /// Consume and return the inner value.
    pub fn into_inner(self) -> V {
        self.id
    }

    /// Borrow the inner value.
    pub fn inner(&self) -> &V {
        &self.id
    }
}

// ---------------------------------------------------------------------------
// Serialization — delegates to the inner value
// ---------------------------------------------------------------------------

impl<T, V: DomainIdValue + Serialize> Serialize for DomainId<T, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.id.serialize(serializer)
    }
}

impl<'de, T, V: DomainIdValue + Deserialize<'de>> Deserialize<'de> for DomainId<T, V> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let id = V::deserialize(deserializer)?;
        Ok(Self::new(id))
    }
}

// ---------------------------------------------------------------------------
// Display — delegates to the inner value
// ---------------------------------------------------------------------------

impl<T, V: DomainIdValue> Display for DomainId<T, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.id, f)
    }
}

// ---------------------------------------------------------------------------
// String-specific traits — only when V = String (backward compatible)
// ---------------------------------------------------------------------------

impl<T> Deref for DomainId<T, String> {
    type Target = str;

    fn deref(&self) -> &str {
        &self.id
    }
}

impl<T> AsRef<str> for DomainId<T, String> {
    fn as_ref(&self) -> &str {
        &self.id
    }
}

impl<T> From<String> for DomainId<T, String> {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl<T> From<&str> for DomainId<T, String> {
    fn from(s: &str) -> Self {
        Self::new(s.to_owned())
    }
}

// ---------------------------------------------------------------------------
// Default — available for any V that implements Default
// ---------------------------------------------------------------------------

impl<T, V: DomainIdValue + Default> Default for DomainId<T, V> {
    fn default() -> Self {
        Self::new(V::default())
    }
}
