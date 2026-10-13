use std::time::Duration;

use async_trait::async_trait;

use crate::domain::core::error::DomainResult;

/// Binary objects addressed by a relative path (`a/b/file.pdf`) inside one configured store.
///
/// Adapters reject empty, oversized or traversal (`.`/`..`) paths and never expose object names
/// or provider messages in errors.
#[async_trait]
pub trait ObjectStorage: Send + Sync {
    /// Stores `content` at `path`, replacing any existing object. An empty `content_type` lets
    /// the store infer or default it.
    async fn upload_file(
        &self,
        path: &str,
        content: Vec<u8>,
        content_type: &str,
    ) -> DomainResult<()>;

    /// Reads the whole object into memory; intended for small files, not streaming.
    /// A missing object is `NotFound`, distinct from provider failures.
    async fn download_file(&self, path: &str) -> DomainResult<Vec<u8>>;

    /// Time-limited GET URL usable by anyone holding it until it expires; keep `expires_in`
    /// short. `disposition` sets the response `Content-Disposition`
    /// (e.g. `attachment; filename="a.pdf"`).
    async fn get_public_url(
        &self,
        path: &str,
        expires_in: Duration,
        disposition: Option<&str>,
    ) -> DomainResult<String>;
}
