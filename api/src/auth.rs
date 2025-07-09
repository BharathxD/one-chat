use axum::{
    async_trait,
    extract::{FromRequestParts, Request, TypedHeader},
    headers::{authorization::Bearer, Authorization},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::env;
use tracing::warn;

// The claims that will be encoded into the JWT and extracted.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // Subject (user ID)
    pub exp: usize,  // Expiration time (timestamp)
    // Add any other claims you need, e.g., roles, permissions
}

// Struct to represent the authenticated user, to be added as a request extension.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub id: String,
}

// Configuration for JWT generation and validation
pub struct TokenConfig {
    secret: String,
    expiration_hours: i64,
}

impl TokenConfig {
    pub fn from_env() -> Result<Self, String> {
        let secret = env::var("JWT_SECRET").map_err(|_| "JWT_SECRET not set".to_string())?;
        let expiration_hours_str = env::var("JWT_EXPIRATION_HOURS").unwrap_or_else(|_| "24".to_string());
        let expiration_hours = expiration_hours_str
            .parse::<i64>()
            .map_err(|_| "Invalid JWT_EXPIRATION_HOURS".to_string())?;
        Ok(Self { secret, expiration_hours })
    }
}

/// Generates a JWT for a given user ID.
pub fn create_jwt(user_id: &str, config: &TokenConfig) -> Result<String, jsonwebtoken::errors::Error> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(config.expiration_hours))
        .expect("Failed to calculate expiration")
        .timestamp();

    let claims = Claims {
        sub: user_id.to_owned(),
        exp: expiration as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.secret.as_ref()),
    )
}

/// Validates a JWT and returns the claims if valid.
fn validate_jwt(token: &str, config: &TokenConfig) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.secret.as_ref()),
        &Validation::default(), // Default validation checks 'exp' and signature
    )
}

// Axum middleware for JWT authentication
pub async fn auth_middleware(
    TypedHeader(Authorization(bearer)): TypedHeader<Authorization<Bearer>>, // Extracts the Bearer token
    mut request: Request,
    next: Next,
) -> Response {
    let token_config = match TokenConfig::from_env() {
        Ok(config) => config,
        Err(e) => {
            warn!("JWT TokenConfig error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Token configuration error").into_response();
        }
    };

    match validate_jwt(bearer.token(), &token_config) {
        Ok(claims) => {
            let user = AuthenticatedUser { id: claims.sub };
            request.extensions_mut().insert(user); // Add AuthenticatedUser to request extensions
            next.run(request).await
        }
        Err(err) => {
            warn!("JWT validation error: {}", err);
            (StatusCode::UNAUTHORIZED, "Invalid or expired token").into_response()
        }
    }
}


// Extractor for AuthenticatedUser. This allows handlers to easily get the user.
// Example: async fn protected_route(user: AuthenticatedUser) -> impl IntoResponse { ... }
#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut axum::http::request::Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or_else(|| {
                warn!("AuthenticatedUser not found in request extensions. Is auth_middleware missing for this route?");
                AuthError::MissingCredentials
            })
    }
}

#[derive(Debug)]
pub enum AuthError {
    MissingCredentials,
    // InvalidToken, // This is handled by the middleware returning StatusCode::UNAUTHORIZED directly
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AuthError::MissingCredentials => (StatusCode::UNAUTHORIZED, "Missing credentials"),
            // AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token"),
        };
        (status, error_message).into_response()
    }
}

/*
Usage in main.rs for a protected route:

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer; // If you need CORS

// ... other imports ...
use crate::auth::{auth_middleware, AuthenticatedUser}; // Import middleware and extractor

async fn protected_handler(user: AuthenticatedUser) -> impl IntoResponse {
    format!("Hello, authenticated user {}!", user.id)
}

async fn public_handler() -> impl IntoResponse {
    "This is a public route."
}

// In main():
// let protected_routes = Router::new()
//     .route("/protected", get(protected_handler))
//     .route_layer(axum::middleware::from_fn(auth_middleware));
//
// let app = Router::new()
//     .route("/public", get(public_handler))
//     .merge(protected_routes)
//     // ... other layers like DBManager, CORS ...
//     .layer(Extension(db_manager))
//     .layer(CorsLayer::permissive()); // Example
*/

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration as StdDuration; // Renamed to avoid conflict with chrono::Duration

    fn test_config() -> TokenConfig {
        TokenConfig {
            secret: "test_secret_key_very_secure".to_string(),
            expiration_hours: 1,
        }
    }

    #[test]
    fn test_create_and_validate_jwt_ok() {
        let config = test_config();
        let user_id = "user123";

        let token = create_jwt(user_id, &config).expect("Failed to create JWT");

        let claims = validate_jwt(&token, &config).expect("Failed to validate JWT");

        assert_eq!(claims.sub, user_id);
        // Check expiration is roughly correct (within a few seconds of 1 hour from now)
        let now_ts = Utc::now().timestamp();
        let expected_exp_ts = Utc::now().checked_add_signed(Duration::hours(1)).unwrap().timestamp();
        assert!((claims.exp as i64 - expected_exp_ts).abs() < 5, "Expiration time mismatch");
        assert!(claims.exp as i64 > now_ts, "Token should not be expired yet");
    }

    #[test]
    fn test_validate_jwt_expired() {
        let config = TokenConfig {
            secret: "test_secret_key_very_secure".to_string(),
            expiration_hours: -1, // Token expired an hour ago
        };
        let user_id = "user123";

        let token = create_jwt(user_id, &config).expect("Failed to create expired JWT");

        // Need to wait for a moment for time to pass if exp is set to exactly now or very close past
        // However, expiration_hours: -1 should make it clearly in the past.
        // If testing with 0 expiration or very small positive, sleep might be needed.
        // sleep(StdDuration::from_secs(1)); // Not strictly needed for -1 hour exp

        let result = validate_jwt(&token, &config);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.kind(), &jsonwebtoken::errors::ErrorKind::ExpiredSignature);
        } else {
            panic!("Token should have been invalid (expired)");
        }
    }

    #[test]
    fn test_validate_jwt_wrong_secret() {
        let config1 = test_config();
        let user_id = "user123";
        let token = create_jwt(user_id, &config1).expect("Failed to create JWT");

        let config2 = TokenConfig {
            secret: "wrong_secret_key".to_string(),
            expiration_hours: 1,
        };

        let result = validate_jwt(&token, &config2);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.kind(), &jsonwebtoken::errors::ErrorKind::InvalidSignature);
        } else {
            panic!("Token should have been invalid (wrong secret)");
        }
    }

    #[test]
    fn test_validate_jwt_tampered() {
        let config = test_config();
        let user_id = "user123";
        let token = create_jwt(user_id, &config).expect("Failed to create JWT");

        // Tamper with the token (e.g., append garbage)
        let tampered_token = format!("{}.garbage", token);

        let result = validate_jwt(&tampered_token, &config);
        assert!(result.is_err());
        // This might result in InvalidToken, InvalidSignature, or InvalidAlgorithmName
        // depending on how it's malformed. Often InvalidToken or InvalidSignature.
        // For simple appending, it usually breaks the signature part.
        match result.unwrap_err().kind() {
            jsonwebtoken::errors::ErrorKind::InvalidToken |
            jsonwebtoken::errors::ErrorKind::InvalidSignature |
            jsonwebtoken::errors::ErrorKind::Malformed => (), // Expected errors
            other_error => panic!("Unexpected error kind for tampered token: {:?}", other_error),
        }
    }
}
