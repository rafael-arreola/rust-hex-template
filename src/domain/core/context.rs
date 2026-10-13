//! Operation context: a typed bag that carries common values of one operation (actor, country,
//! tenant...) from whoever learns them down to every layer that needs them.
//!
//! It plays the role of Go's `context.Context` for **carrying values**, not for cancellation: a
//! Rust future is cancelled by dropping it, and the request budget is enforced by the HTTP timeout.
//!
//! It holds **one value per type**, and the type is the key: `ctx.insert(country)` and later
//! `ctx.require::<Country>()?`. A primitive that needs its own name is modelled as a newtype
//! (`struct TenantId(String)`) and never enters the bag bare.
//!
//! - `insert`, `get` and `require` take `&self`: any layer can add or read values without `&mut`,
//!   and several futures may share the same context at once.
//! - `Clone` returns a handle to the **same** contents, like passing `ctx` by value in Go. Detached
//!   work builds its own bag from the payload that woke it (`Context::new().with(actor)`).
//! - The domain does not know business types: the context stores what upper layers decide and each
//!   reader takes only what it declares it needs.

use std::any::{Any, TypeId, type_name};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::error::{DomainError, DomainResult};

/// Requirements of every value carried by the context. Values are handed out by copy from behind
/// a lock, so a large or costly value is stored wrapped in `Arc`.
pub trait ContextValue: Any + Clone + Send + Sync + 'static {}

impl<T> ContextValue for T where T: Any + Clone + Send + Sync + 'static {}

/// Erased values keyed by their [`TypeId`]. The inner `Arc` lets the whole map be shared when the
/// `Context` is cloned without knowing the concrete types it holds.
type Values = HashMap<TypeId, Arc<dyn Any + Send + Sync>>;

/// Typed bag of values of the current operation. See the module documentation.
#[derive(Clone, Default)]
pub struct Context {
    values: Arc<RwLock<Values>>,
}

impl Context {
    /// Empty context, the equivalent of `context.Background()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Chainable constructor for code that builds a context outside HTTP, such as a worker that
    /// rebuilds it from its task payload: `Context::new().with(actor)`.
    #[must_use]
    pub fn with<T: ContextValue>(self, value: T) -> Self {
        self.insert(value);
        self
    }

    /// Stores `value` under its type and returns the previous value of that type, if any.
    pub fn insert<T: ContextValue>(&self, value: T) -> Option<T> {
        self.write()
            .insert(TypeId::of::<T>(), Arc::new(value))
            .and_then(|previous| previous.downcast_ref::<T>().cloned())
    }

    /// Copy of the value of type `T`, if some layer inserted it.
    pub fn get<T: ContextValue>(&self) -> Option<T> {
        self.read().get(&TypeId::of::<T>()).and_then(|value| value.downcast_ref::<T>()).cloned()
    }

    /// Like [`get`](Self::get), but absence is a wiring defect: the layer that should insert `T`
    /// did not run for this operation. It is reported as an internal error, never as a 4xx.
    pub fn require<T: ContextValue>(&self) -> DomainResult<T> {
        self.get::<T>().ok_or_else(|| {
            DomainError::internal(format!(
                "Operation context has no value of type `{}`",
                type_name::<T>()
            ))
        })
    }

    /// A poisoned lock only means another thread panicked mid-write; the bag stays readable and
    /// there is no invariant to protect.
    fn read(&self) -> RwLockReadGuard<'_, Values> {
        self.values.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, Values> {
        self.values.write().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Values may be personal data (an `Actor`), so only the entry count is printed.
impl fmt::Debug for Context {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Context").field("entries", &self.read().len()).finish()
    }
}
