/// Serializes a struct safely for structured tracing logs.
///
/// If serialization fails, returns a JSON object with the error detail instead of panicking.
#[macro_export]
macro_rules! as_json {
    ($val:expr) => {
        &serde_json::to_string($val)
            .unwrap_or_else(|e| format!("{{\"error\":\"Serialization failed: {}\"}}", e))
    };
}
