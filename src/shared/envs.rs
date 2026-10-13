use std::env;
use std::fmt;
use std::sync::OnceLock;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Required environment variable {name} is missing or empty")]
    Missing { name: &'static str },
    #[error("Environment variable {name} must contain valid Unicode")]
    NotUnicode { name: &'static str },
    #[error("Invalid environment variable {name}")]
    Invalid { name: &'static str },
    #[error("Failed to load environment configuration file")]
    EnvironmentFile,
}

/// Process environment, loaded and validated once by `init()` during bootstrap.
///
/// Read it with `get()` only from bootstrap and composition (`main.rs`, `app.rs`, `shared`).
/// Adapters and services receive the values they need through their constructors.
pub struct Env {
    pub port: u16,
    pub service_env: String,
    pub service_name: String,
    pub project_id: String,
    pub mongo_url: String,
    pub mongo_db: String,
    pub debug_level: String,
    pub drain_timeout_secs: u64,
    pub request_timeout_secs: u64,
    pub body_limit_bytes: usize,
    pub msgpack_enabled: bool,
}

impl fmt::Debug for Env {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Env")
            .field("port", &self.port)
            .field("drain_timeout_secs", &self.drain_timeout_secs)
            .field("request_timeout_secs", &self.request_timeout_secs)
            .field("body_limit_bytes", &self.body_limit_bytes)
            .field("msgpack_enabled", &self.msgpack_enabled)
            .finish_non_exhaustive()
    }
}

static ENV: OnceLock<Env> = OnceLock::new();

/// Loads and validates the environment once; call it first in bootstrap. Repeated calls return
/// the already loaded value.
pub fn init() -> Result<&'static Env, ConfigError> {
    if let Some(env) = ENV.get() {
        return Ok(env);
    }
    let env = Env::load()?;
    Ok(ENV.get_or_init(|| env))
}

/// The environment loaded by `init()`. It waits for `init()`, which runs before anything else in
/// `main`, so in practice it never blocks.
pub fn get() -> &'static Env {
    ENV.wait()
}

impl Env {
    /// Loads `.env` when present. Absent or empty variables take their default; a present but
    /// invalid value fails instead of falling back.
    fn load() -> Result<Self, ConfigError> {
        match dotenvy::dotenv() {
            Ok(_) => {}
            Err(error) if error.not_found() => {}
            Err(_) => return Err(ConfigError::EnvironmentFile),
        }

        Ok(Self {
            port: or("PORT", 8080)?,
            service_env: or("SERVICE_ENV", "DEV".to_string())?,
            service_name: required("SERVICE_NAME")?,
            project_id: or("PROJECT_ID", String::new())?,
            mongo_url: required("MONGO_URL")?,
            mongo_db: required("MONGO_DB")?,
            debug_level: or("DEBUG_LEVEL", "info".to_string())?,
            drain_timeout_secs: or("DRAIN_TIMEOUT_SECS", 10)?,
            request_timeout_secs: or("REQUEST_TIMEOUT_SECS", 300)?,
            body_limit_bytes: or("BODY_LIMIT_BYTES", 32 * 1024 * 1024)?,
            msgpack_enabled: or("ENABLE_MSGPACK", true)?,
        })
    }
}

/// Typed value read from one trimmed, non-empty variable; `None` means the text is invalid.
trait EnvValue: Sized {
    fn parse(raw: &str) -> Option<Self>;
}

impl EnvValue for String {
    fn parse(raw: &str) -> Option<Self> {
        Some(raw.to_string())
    }
}

/// Numeric settings here are ports, timeouts and sizes: zero is never a valid value.
macro_rules! positive_number {
    ($($number:ty),*) => {$(
        impl EnvValue for $number {
            fn parse(raw: &str) -> Option<Self> {
                raw.parse().ok().filter(|value| *value > 0)
            }
        }
    )*};
}
positive_number!(u16, u64, usize);

impl EnvValue for bool {
    fn parse(raw: &str) -> Option<Self> {
        match raw.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" => Some(true),
            "0" | "false" | "no" => Some(false),
            _ => None,
        }
    }
}

/// Comma-separated list without empty entries; duplicates are dropped, order is kept.
impl EnvValue for Vec<String> {
    fn parse(raw: &str) -> Option<Self> {
        let mut entries: Vec<String> = Vec::new();
        for entry in raw.split(',').map(str::trim) {
            if entry.is_empty() {
                return None;
            }
            if !entries.iter().any(|existing| existing == entry) {
                entries.push(entry.to_string());
            }
        }
        Some(entries)
    }
}

fn optional<T: EnvValue>(name: &'static str) -> Result<Option<T>, ConfigError> {
    let raw = match env::var(name) {
        Ok(raw) => raw,
        Err(env::VarError::NotPresent) => return Ok(None),
        Err(env::VarError::NotUnicode(_)) => return Err(ConfigError::NotUnicode { name }),
    };
    match raw.trim() {
        "" => Ok(None),
        value => T::parse(value).map(Some).ok_or(ConfigError::Invalid { name }),
    }
}

fn required<T: EnvValue>(name: &'static str) -> Result<T, ConfigError> {
    optional(name)?.ok_or(ConfigError::Missing { name })
}

fn or<T: EnvValue>(name: &'static str, default: T) -> Result<T, ConfigError> {
    Ok(optional(name)?.unwrap_or(default))
}
