use crate::domain::core::error::{DomainError, DomainResult, StatusCode};

/// Validated pagination parameters for repository queries.
#[derive(Debug, Clone, Copy)]
pub struct Pagination {
    page: u32,
    limit: u32,
}

impl Pagination {
    pub fn new(page: u32, limit: u32) -> DomainResult<Self> {
        if page == 0 {
            return Err(DomainError::new(
                StatusCode::BAD_REQUEST,
                "Invalid page: must be at least 1",
            )
            .with_public("Invalid page: must be at least 1")
            .with_trace_code("INVALID_PAGINATION_PAGE"));
        }
        if !(1..=100).contains(&limit) {
            return Err(DomainError::new(
                StatusCode::BAD_REQUEST,
                "Invalid limit: must be between 1 and 100",
            )
            .with_public("Invalid limit: must be between 1 and 100")
            .with_trace_code("INVALID_PAGINATION_LIMIT"));
        }
        Ok(Self { page, limit })
    }

    pub fn page(&self) -> u32 {
        self.page
    }

    pub fn limit(&self) -> u32 {
        self.limit
    }

    pub fn get_skip(&self) -> u64 {
        u64::from(self.page.saturating_sub(1)) * u64::from(self.limit)
    }

    pub fn get_limit(&self) -> i64 {
        i64::from(self.limit)
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Self { page: 1, limit: 20 }
    }
}

/// Items and total match the same filter, but need not share a database snapshot.
#[derive(Debug, Clone)]
pub struct PageResult<T> {
    pub items: Vec<T>,
    pub total: u64,
}
