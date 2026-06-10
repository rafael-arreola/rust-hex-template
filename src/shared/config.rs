use dotenvy::dotenv;
use std::env;
use std::process;
use std::sync::OnceLock;

#[derive(Debug)]
pub struct Env {
    pub port: u16,
    pub app_env: String,
    pub service_name: String,
    pub project_id: String,
    pub mongo_url: String,
    pub mongo_db: String,
    // pub redis_url: String,
    // pub redis_prefix: String,
    pub debug_level: String,
    pub cors_origins: String,
    pub drain_timeout_secs: u64,
}

static CONFIG: OnceLock<Env> = OnceLock::new();

pub fn get() -> &'static Env {
    CONFIG.get_or_init(Env::load)
}

impl Env {
    fn load() -> Self {
        dotenv().ok();

        Self {
            port: parse_port(),
            service_name: require_env("SERVICE_NAME"),
            app_env: env::var("APP_ENV")
                .or_else(|_| env::var("ENV"))
                .unwrap_or_else(|_| "DEV".to_string()),
            project_id: env::var("PROJECT_ID").unwrap_or_default(),
            mongo_url: require_env("MONGO_URL"),
            mongo_db: require_env("MONGO_DB"),
            // redis_url: require_env("REDIS_URL"),
            // redis_prefix: env::var("REDIS_PREFIX").unwrap_or_else(|_| "service".to_string()),
            debug_level: env::var("DEBUG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            cors_origins: env::var("CORS_ORIGINS").unwrap_or_else(|_| "*".to_string()),
            drain_timeout_secs: parse_timeout("DRAIN_TIMEOUT_SECS", 10),
        }
    }
}

fn require_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| {
        eprintln!("CRITICAL ERROR: Missing required environment variable '{}'", name);
        process::exit(1);
    })
}

/// Validates that the environment variable is present and is a clean DNS/hostname (no 'http://' or slashes).
#[allow(dead_code)]
fn require_dns(name: &str) -> String {
    let val = require_env(name);
    let clean = val.trim();
    if clean.contains("://") || clean.contains('/') {
        eprintln!(
            "CRITICAL ERROR: Variable '{}' must be a clean DNS/hostname (e.g. 'example.com'), got '{}'",
            name, val
        );
        process::exit(1);
    }
    clean.to_string()
}

/// Validates that the environment variable is present and is a valid HTTP/HTTPS URL.
#[allow(dead_code)]
fn require_url(name: &str) -> String {
    let val = require_env(name);
    let clean = val.trim();
    if !clean.starts_with("http://") && !clean.starts_with("https://") {
        eprintln!(
            "CRITICAL ERROR: Variable '{}' must be a valid HTTP/HTTPS URL starting with http:// or https://, got '{}'",
            name, val
        );
        process::exit(1);
    }
    clean.to_string()
}

/// Tries to load any of the specified environment variable names, failing if none of them are present.
#[allow(dead_code)]
fn require_env_any(names: &[&str]) -> String {
    for name in names {
        if let Ok(val) = env::var(name) {
            return val.trim().to_string();
        }
    }
    eprintln!(
        "CRITICAL ERROR: Missing required environment variable. Must provide one of: {:?}",
        names
    );
    process::exit(1);
}

fn parse_port() -> u16 {
    let port_str = env::var("PORT").unwrap_or_else(|_| "3000".into());
    port_str.parse().unwrap_or_else(|_| {
        eprintln!("CRITICAL ERROR: PORT must be a valid number, got '{}'", port_str);
        process::exit(1);
    })
}

fn parse_timeout(name: &str, default: u64) -> u64 {
    env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
