/// Serializa de forma segura una estructura para logs estructurados en tracing.
///
/// Si la serialización falla, retorna un objeto JSON con el detalle del error en lugar de provocar un pánico.
#[macro_export]
macro_rules! as_json {
    ($val:expr) => {
        &serde_json::to_string($val)
            .unwrap_or_else(|e| format!("{{\"error\":\"Serialization failed: {}\"}}", e))
    };
}
