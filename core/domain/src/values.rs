use std::marker::PhantomData;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Type-safe domain identifier parameterized by a marker type.
///
/// Serializes/deserializes as a plain string (not an object).
///
/// ```ignore
/// #[derive(Debug, Clone)]
/// pub struct UserMarker;
/// pub type UserId = DomainId<UserMarker>;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DomainId<T> {
    id: String,
    _marker: PhantomData<T>,
}

// ===== Construction =====

impl<T> DomainId<T> {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            _marker: PhantomData,
        }
    }

    pub fn into_inner(self) -> String {
        self.id
    }
}

// ===== Serde (plain string, not object) =====

impl<T> Serialize for DomainId<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.id.serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for DomainId<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let id = String::deserialize(deserializer)?;
        Ok(Self::new(id))
    }
}

// ===== Conversions & Access =====

impl<T> Deref for DomainId<T> {
    type Target = str;

    fn deref(&self) -> &str {
        &self.id
    }
}

impl<T> AsRef<str> for DomainId<T> {
    fn as_ref(&self) -> &str {
        &self.id
    }
}

impl<T> std::fmt::Display for DomainId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id.fmt(f)
    }
}

impl<T> From<String> for DomainId<T> {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl<T> From<&str> for DomainId<T> {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl<T> Default for DomainId<T> {
    fn default() -> Self {
        Self::new(String::new())
    }
}
