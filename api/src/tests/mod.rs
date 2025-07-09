// This file makes modules within src/tests/ accessible.
// For example, if you have src/tests/health_check_tests.rs:

pub mod health_check_tests;
// pub mod thread_tests; // etc.

// Common test utilities can also go here or in a separate test_utils.rs
use axum::{Extension, Router};
use std::net::{SocketAddr, TcpListener};
use tokio::net::TcpListener as TokioTcpListener; // Use Tokio's TcpListener for async server

use crate::db::DBManager;
use crate::redis_utils::{RateLimiter, RedisManager};
use crate::routes::attachment_routes::attachment_router;
use crate::routes::health_routes::health_router;
use crate::routes::message_routes::message_router;
use crate::routes::openai_compatible_routes::openai_compatible_router;
use crate::routes::share_routes::share_router;
use crate::routes::thread_routes::thread_router;
use crate::routes::voice_routes::voice_router;
use crate::auth; // For JWT creation in tests

// Helper to spawn the app in the background for testing.
// Returns the server's local address.
pub async fn spawn_app() -> String {
    // Use a random available port
    let listener = TokioTcpListener::bind("127.0.0.1:0").await.expect("Failed to bind random port");
    let addr = listener.local_addr().unwrap();
    let server_url = format!("http://{}", addr);

    // Setup minimal environment for tests if not already set globally
    // IMPORTANT: For tests, ensure env vars like JWT_SECRET, DATABASE_URL (for test DB), REDIS_URL are set.
    // It's better to configure these via a .env.test file loaded by dotenvy or specific test setup.
    // For now, we assume they might be set or use test defaults within the app logic if possible.
    // A robust test setup would use a test-specific .env or config.
    dotenvy::dotenv().ok(); // Load .env if available, might override with test specifics later

    // Test-specific JWT config (can override env for consistency in tests)
    std::env::set_var("JWT_SECRET", "test_jwt_secret_for_integration_tests");
    std::env::set_var("JWT_EXPIRATION_HOURS", "1");
    // Mock DATABASE_URL and REDIS_URL if they point to real dev instances and you have test instances
    // e.g., std::env::set_var("DATABASE_URL", "mongodb://localhost:27017/test_app_db");
    // std::env::set_var("REDIS_URL", "redis://localhost:6379/1"); // Use a different Redis DB for tests

    let db_manager = DBManager::new().await.expect("Failed to init test DBManager");
    let redis_manager = RedisManager::new(&std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string())).await.expect("Failed to init test RedisManager");
    let voice_rate_limiter = RateLimiter::new(redis_manager.clone(), "test_rl_voice", 20, 3600);
    let http_client = reqwest::Client::new();

    let app = Router::new()
        .route("/", axum::routing::get(|| async {"Test Root OK"})) // Keep root for basic check
        .nest("/api/threads", thread_router())
        .nest("/api/messages", message_router())
        .nest("/api/shares", share_router())
        .nest("/api/health", health_router())
        .nest("/api/attachments", attachment_router())
        .nest("/api/voice", voice_router())
        .nest("/v1", openai_compatible_router())
        .layer(Extension(db_manager.clone()))
        .layer(Extension(voice_rate_limiter.clone()))
        .layer(Extension(http_client.clone()));

    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.unwrap();
    });

    server_url
}

// Helper to create a valid JWT for testing protected routes
pub fn generate_test_jwt(user_id: &str) -> String {
    let config = auth::TokenConfig {
        secret: "test_jwt_secret_for_integration_tests".to_string(), // Must match what spawn_app sets/expects
        expiration_hours: 1,
    };
    auth::create_jwt(user_id, &config).unwrap()
}
