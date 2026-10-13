//! Validating request extractors; rejections stay inside the HTTP envelope.

mod body;
mod path;
mod query;

use serde::Deserialize;
use validator::Validate;

use crate::domain::core::error::{DomainError, DomainResult};
use crate::domain::core::pagination::Pagination;

pub use body::ValidatedBody;
pub use path::PathParam;
pub use query::ValidatedQuery;

/// Shared transport query for list endpoints; `Pagination::new` remains the authoritative check.
#[derive(Deserialize, Validate)]
pub struct PaginationQuery {
    #[validate(range(min = 1))]
    pub page: Option<u32>,
    #[validate(range(min = 1, max = 100))]
    pub limit: Option<u32>,
}

impl TryFrom<PaginationQuery> for Pagination {
    type Error = DomainError;

    fn try_from(query: PaginationQuery) -> DomainResult<Self> {
        Self::new(query.page.unwrap_or(1), query.limit.unwrap_or(20))
    }
}
