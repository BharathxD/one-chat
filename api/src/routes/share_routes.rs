use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{delete, get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{
    auth::{auth_middleware, AuthenticatedUser},
    db::DBManager,
    models::{self as db_models, PartialShare, generate_id as generate_model_id}, // generate_id for tokens
};

// Payloads
#[derive(Deserialize)]
struct CreateSharePayload {
    thread_id: String,
    shared_up_to_message_id: String,
    token: Option<String>, // Client can suggest a token
}

// Responses
#[derive(Serialize)]
struct PartialShareResponse {
    token: String,
    thread_id: String,
    user_id: String,
    shared_up_to_message_id: String,
    created_at: String,
}

impl From<PartialShare> for PartialShareResponse {
    fn from(ps: PartialShare) -> Self {
        PartialShareResponse {
            token: ps.token,
            thread_id: ps.thread_id,
            user_id: ps.user_id,
            shared_up_to_message_id: ps.shared_up_to_message_id,
            created_at: ps.created_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize)]
struct SharedThreadDataResponse {
    thread: super::thread_routes::ThreadResponse, // Reuse ThreadResponse
    messages: Vec<super::thread_routes::MessageResponse>, // Reuse MessageResponse
}

pub fn share_router() -> Router {
    // Authenticated routes for managing shares
    let protected_share_routes = Router::new()
        .route("/", post(create_partial_share_handler))
        .route("/", get(get_user_partial_shares_handler))
        .route("/:token", delete(delete_partial_share_handler))
        .route_layer(middleware::from_fn(auth_middleware));

    // Public route to get shared data
    let public_share_route = Router::new()
        .route("/:token/data", get(get_shared_thread_data_handler));

    Router::new()
        .merge(protected_share_routes)
        .merge(public_share_route)
}

// --- Protected Handlers ---

async fn create_partial_share_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateSharePayload>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let token = payload.token.unwrap_or_else(generate_model_id);
    info!(
        "User {} creating partial share for thread {} up to message {}. Token: {}",
        user.id, payload.thread_id, payload.shared_up_to_message_id, token
    );

    match db.create_partial_share(
        token,
        &payload.thread_id,
        &user.id,
        &payload.shared_up_to_message_id,
    ).await {
        Ok(ps) => Ok((StatusCode::CREATED, Json(PartialShareResponse::from(ps)))),
        Err(e) => {
            error!("Failed to create partial share: {}", e);
            if e.to_string().contains("already exists") {
                Err((StatusCode::CONFLICT, e.to_string()))
            } else if e.to_string().contains("not found") || e.to_string().contains("does not own") {
                Err((StatusCode::BAD_REQUEST, e.to_string()))
            }
            else {
                Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to create partial share".to_string()))
            }
        }
    }
}

async fn get_user_partial_shares_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} fetching their partial shares", user.id);
    match db.find_partial_shares_by_user_id(&user.id).await {
        Ok(shares) => {
            let responses: Vec<PartialShareResponse> = shares.into_iter().map(PartialShareResponse::from).collect();
            Ok(Json(responses))
        }
        Err(e) => {
            error!("Failed to fetch user partial shares: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch partial shares".to_string()))
        }
    }
}

async fn delete_partial_share_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
    Path(token): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} deleting partial share with token {}", user.id, token);
    match db.delete_partial_share_by_token(&token, &user.id).await {
        Ok(deleted_count) => {
            if deleted_count > 0 {
                Ok(StatusCode::NO_CONTENT)
            } else {
                Err((StatusCode::NOT_FOUND, "Share token not found or user does not own it".to_string()))
            }
        }
        Err(e) => {
            error!("Failed to delete partial share {}: {}", token, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete partial share".to_string()))
        }
    }
}

// --- Public Handler ---

async fn get_shared_thread_data_handler(
    Extension(db): Extension<DBManager>,
    Path(token): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("Fetching shared data for token {}", token);

    // 1. Find the partial share
    let share_info = match db.find_partial_share_by_token(&token).await {
        Ok(Some(info)) => info,
        Ok(None) => return Err((StatusCode::NOT_FOUND, "Share token not found".to_string())),
        Err(e) => {
            error!("Error finding share token {}: {}", token, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Error retrieving share information".to_string()));
        }
    };

    // 2. Fetch the thread (must be public or owned by share creator, but public access is implied by share link)
    let thread_model = match db.find_thread_by_id(&share_info.thread_id).await {
        Ok(Some(t)) => {
            // Optional: Add a check here if shares should only work for public threads or if creator matters.
            // For now, if a share link exists, we assume it's valid to share.
            t
        }
        Ok(None) => return Err((StatusCode::NOT_FOUND, "Shared thread not found".to_string())),
        Err(e) => {
            error!("Error fetching shared thread {}: {}", share_info.thread_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Error retrieving shared thread".to_string()));
        }
    };

    // 3. Fetch messages up to the shared_up_to_message_id
    // We need a DB function for this: find_messages_up_to(thread_id, message_id_limit)
    let anchor_message = match db.find_message_by_id(&share_info.shared_up_to_message_id).await? {
        Some(m) => m,
        None => return Err((StatusCode::NOT_FOUND, "Anchor message for share not found".to_string())),
    };

    let messages_filter = mongodb::bson::doc! {
        "threadId": &share_info.thread_id,
        "createdAt": { "$lte": mongodb::bson::DateTime::from_chrono(anchor_message.created_at) }
    };
    let sort_options = mongodb::options::FindOptions::builder().sort(mongodb::bson::doc! { "createdAt": 1 }).build();

    let mut cursor = db.messages_collection().find(messages_filter, sort_options).await
        .map_err(|e| {
            error!("Error fetching messages for shared thread {}: {}", share_info.thread_id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Error retrieving shared messages".to_string())
        })?;

    let mut messages_models = Vec::new();
    let mut found_anchor_in_shared_messages = false;
    while let Some(msg_result) = cursor.try_next().await.map_err(|e| {
        error!("Error iterating messages for shared thread {}: {}", share_info.thread_id, e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Error processing shared messages".to_string())
    })? {
        messages_models.push(msg_result.clone());
         if msg_result.id.as_deref() == Some(&share_info.shared_up_to_message_id) {
            found_anchor_in_shared_messages = true;
        }
    }
    // Ensure the anchor message is included if $lte didn't catch it due to timestamp granularity
    if !found_anchor_in_shared_messages && !messages_models.iter().any(|m: &db_models::Message| m.id.as_deref() == Some(&share_info.shared_up_to_message_id)) {
        if anchor_message.thread_id == share_info.thread_id { // Basic check
             messages_models.push(anchor_message);
             messages_models.sort_by_key(|m| m.created_at);
        }
    }


    let response = SharedThreadDataResponse {
        thread: super::thread_routes::ThreadResponse::from(thread_model),
        messages: messages_models.into_iter().map(super::thread_routes::MessageResponse::from).collect(),
    };

    Ok((StatusCode::OK, Json(response)))
}
