use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Types that can be stored inside a [`DomainId`].
pub trait DomainIdValue:
    Clone + Debug + Display + PartialEq + Eq + Hash + Send + Sync + 'static
{
}

impl<V> DomainIdValue for V where
    V: Clone + Debug + Display + PartialEq + Eq + Hash + Send + Sync + 'static
{
}

/// Type-safe domain identifier parameterized by a zero-sized marker `T` and an inner value `V`
/// (defaults to [`String`]).
///
/// Traits are implemented manually so the marker `T` never needs to derive `Clone`, `Debug`,
/// `PartialEq`, `Eq` or `Hash`.
pub struct DomainId<T, V = String>
where
    V: DomainIdValue,
{
    id: V,
    _marker: PhantomData<T>,
}

impl<T, V: DomainIdValue> Clone for DomainId<T, V> {
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), _marker: PhantomData }
    }
}

impl<T, V: DomainIdValue> Debug for DomainId<T, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DomainId").field(&self.id).finish()
    }
}

impl<T, V: DomainIdValue> PartialEq for DomainId<T, V> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T, V: DomainIdValue> Eq for DomainId<T, V> {}

impl<T, V: DomainIdValue> Hash for DomainId<T, V> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T, V: DomainIdValue> DomainId<T, V> {
    pub fn new(id: impl Into<V>) -> Self {
        Self { id: id.into(), _marker: PhantomData }
    }

    pub fn into_inner(self) -> V {
        self.id
    }

    pub fn inner(&self) -> &V {
        &self.id
    }
}

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
        V::deserialize(deserializer).map(Self::new)
    }
}

impl<T, V: DomainIdValue> Display for DomainId<T, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.id, f)
    }
}

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

impl<T, V: DomainIdValue + Default> Default for DomainId<T, V> {
    fn default() -> Self {
        Self::new(V::default())
    }
}
