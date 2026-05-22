/// Pagination parameters for repository queries.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Pagination {
    /// Page number (1-indexed)
    pub page: u32,
    /// Items per page
    pub limit: u32,
}

impl Pagination {
    pub fn get_skip(&self) -> u64 {
        ((self.page.saturating_sub(1)) * self.limit) as u64
    }

    pub fn get_limit(&self) -> i64 {
        self.limit as i64
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Self { page: 1, limit: 20 }
    }
}
