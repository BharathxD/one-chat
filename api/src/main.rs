use axum::{routing::get, Extension, Router};
use std::env;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Define module placeholders - we'll fill these in later
pub mod auth;
pub mod db;
pub mod ai_services;
pub mod redis_utils;
// pub mod handlers; // Still a placeholder
pub mod models;
pub mod routes;

use crate::db::DBManager;
use crate::redis_utils::{RateLimiter, RedisManager};
use crate::routes::attachment_routes::attachment_router;
use crate::routes::health_routes::health_router;
use crate::routes::message_routes::message_router;
use crate::routes::share_routes::share_router;
use crate::routes::thread_routes::thread_router;
use crate::routes::voice_routes::voice_router;
use crate::routes::openai_compatible_routes::openai_compatible_router; // Import new router

#[tokio::main]
async fn main() {
    // Initialize tracing (logging)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::new(
                std::env::var("RUST_LOG").unwrap_or_else(|_| "api=debug,tower_http=debug".into()),
            )
            .and_then(tracing_subscriber::fmt::layer()), // Corrected way to combine EnvFilter and fmt::layer
        )
        .init();


    // Load environment variables from .env file
    if dotenvy::dotenv().is_err() {
        info!(".env file not found, using environment variables directly if set");
    }

    // Initialize database connection
    let db_manager = DBManager::new()
        .await
        .expect("Failed to initialize DBManager");
    info!("DBManager initialized successfully.");

    // Initialize RedisManager and RateLimiter for Voice
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let redis_manager = RedisManager::new(&redis_url)
        .await
        .expect("Failed to initialize RedisManager");
    info!("RedisManager initialized successfully.");

    let voice_rate_limiter = RateLimiter::new(redis_manager.clone(), "rl_voice", 20, 3600);
    info!("VoiceRateLimiter initialized.");

    // Initialize shared reqwest client
    let http_client = reqwest::Client::new();
    info!("Shared HTTP Client initialized.");

    // Build application routes
    let app = Router::new()
        .route("/", get(root_handler))
        .nest("/api/threads", thread_router())
        .nest("/api/messages", message_router())
        .nest("/api/shares", share_router())
        .nest("/api/health", health_router())
        .nest("/api/attachments", attachment_router())
        .nest("/api/voice", voice_router())
        .nest("/v1", openai_compatible_router()) // Mount OpenAI compatible routes under /v1
        .layer(Extension(db_manager.clone()))
        .layer(Extension(voice_rate_limiter))
        .layer(Extension(http_client.clone())); // Add shared reqwest client

    // Determine port from environment variable or default
    let port_str = std::env::var("SERVER_PORT").unwrap_or_else(|_| "3001".to_string());
    let port = port_str.parse::<u16>().unwrap_or(3001);
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));

    info!("🚀 Server listening on {}", addr);

    // Run the server
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root_handler() -> &'static str {
    info!("Request received for / (root)");
    "Hello, World from Axum API!"
}

#[cfg(test)]
mod tests; // Declare the tests module
