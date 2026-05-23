use dotenvy::dotenv;
use std::env;
use std::sync::OnceLock;

#[derive(Debug)]
pub struct Env {
    pub port: u16,
    pub app_env: String,
    pub service_name: String,
    pub project_id: String,
    pub mongo_url: String,
    pub mongo_db: String,
    pub debug_level: String,
    pub cors_origins: String,
}

static CONFIG: OnceLock<Env> = OnceLock::new();

pub fn get() -> &'static Env {
    CONFIG.get_or_init(Env::load)
}

impl Env {
    fn load() -> Self {
        dotenv().ok();

        Self {
            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .unwrap_or(3000),
            service_name: env::var("SERVICE_NAME").unwrap_or_else(|_| "service".to_string()),
            app_env: env::var("APP_ENV").unwrap_or_else(|_| "DEV".to_string()),
            project_id: env::var("PROJECT_ID").unwrap_or_default(),
            mongo_url: env::var("MONGO_URL")
                .unwrap_or_else(|_| "mongodb://localhost:27017".to_string()),
            mongo_db: env::var("MONGO_DB").unwrap_or_else(|_| "service_db".to_string()),
            debug_level: env::var("DEBUG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            cors_origins: env::var("CORS_ORIGINS").unwrap_or_else(|_| "*".to_string()),
        }
    }
}
