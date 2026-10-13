//! Bridge between the operation [`Context`] and the HTTP request.
//!
//! The context lives in the request extensions from the first layer that touches it. Each
//! middleware gets it with [`ContextRequestExt::context`] and inserts its own values; the handler
//! receives it as an Axum extractor and passes it down. Every handle shares the same contents, so
//! it does not matter which layer created it.

use std::convert::Infallible;

use axum::extract::{FromRequestParts, Request};
use axum::http::Extensions;
use axum::http::request::Parts;

use crate::domain::core::context::Context;

/// Access to the context of a request from a middleware.
pub trait ContextRequestExt {
    /// Context of this request, created empty if no earlier layer did.
    fn context(&mut self) -> Context;
}

impl ContextRequestExt for Extensions {
    fn context(&mut self) -> Context {
        if let Some(existing) = self.get::<Context>() {
            return existing.clone();
        }
        let fresh = Context::new();
        self.insert(fresh.clone());
        fresh
    }
}

impl ContextRequestExt for Request {
    fn context(&mut self) -> Context {
        self.extensions_mut().context()
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Context {
    /// Never rejects: a route without middlewares receives an empty context, and `require` reports
    /// which value was missing at the point of use.
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.extensions.context())
    }
}
