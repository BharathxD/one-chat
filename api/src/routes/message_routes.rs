use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{delete, post, put}, // Added post
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize}; // Added Serialize
use tracing::{error, info};

use crate::{
    auth::{auth_middleware, AuthenticatedUser},
    db::DBManager,
    models::Message, // For checking ownership
};

// Re-use MessageResponse from thread_routes for consistency
// This requires thread_routes to be a sibling module or making MessageResponse public in a shared models/responses module
// For now, let's assume we might duplicate it or move it to a shared location later.
// To avoid circular dependencies or complex module paths now, I'll define a local one.
#[derive(Serialize)]
pub struct LocalMessageResponse {
    id: String,
    thread_id: String,
    parts: serde_json::Value,
    content: Option<String>,
    role: crate::models::Role,
    annotations: Option<serde_json::Value>,
    model: Option<String>,
    status: crate::models::Status,
    is_errored: bool,
    is_stopped: bool,
    error_message: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<crate::models::Message> for LocalMessageResponse {
    fn from(msg: crate::models::Message) -> Self {
        LocalMessageResponse {
            id: msg.id.unwrap_or_default(),
            thread_id: msg.thread_id,
            parts: msg.parts,
            content: msg.content,
            role: msg.role,
            annotations: msg.annotations,
            model: msg.model,
            status: msg.status,
            is_errored: msg.is_errored,
            is_stopped: msg.is_stopped,
            error_message: msg.error_message,
            created_at: msg.created_at.to_rfc3339(),
            updated_at: msg.updated_at.to_rfc3339(),
        }
    }
}


// Payload for updating a message
#[derive(Deserialize)]
pub struct UpdateMessagePayload {
    content: Option<String>,
    parts: Option<serde_json::Value>,
    status: Option<crate::models::Status>,
    error_message: Option<String>,
}

#[derive(Serialize)]
struct DeletionResponse {
    deleted_count: u64,
    message: String,
}

pub fn message_router() -> Router {
    Router::new()
        .route("/:message_id", put(update_message_handler))
        .route("/:message_id", delete(delete_message_handler))
        .route("/:message_id/delete-trailing", post(delete_trailing_messages_handler))
        .route("/:message_id/delete-inclusive-trailing", post(delete_message_and_trailing_handler))
        .route_layer(middleware::from_fn(auth_middleware))
}

async fn update_message_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
    Path(message_id): Path<String>,
    Json(payload): Json<UpdateMessagePayload>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} updating message {}", user.id, message_id);

    let message = match db.find_message_by_id(&message_id).await {
        Ok(Some(msg)) => msg,
        Ok(None) => return Err((StatusCode::NOT_FOUND, "Message not found".to_string())),
        Err(e) => {
            error!("Error finding message {}: {}", message_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify message".to_string()));
        }
    };

    match db.find_thread_by_id(&message.thread_id).await {
        Ok(Some(thread)) => {
            if thread.user_id != user.id {
                return Err((StatusCode::FORBIDDEN, "You don't have permission to update this message".to_string()));
            }
        }
        Ok(None) => {
            error!("Data inconsistency: Message {} exists but its thread {} not found.", message_id, message.thread_id);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify message ownership due to data inconsistency".to_string()));
        }
        Err(e) => {
            error!("Error finding thread {} for message {}: {}", message.thread_id, message_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify message ownership".to_string()));
        }
    }

    let mut updated_message_model: Option<Message> = None;

    if payload.content.is_some() || payload.parts.is_some() {
         match db.update_message_content(&message_id, payload.content.as_deref().unwrap_or(""), payload.parts.clone()).await {
            Ok(Some(msg)) => updated_message_model = Some(msg),
            Ok(None) => return Err((StatusCode::NOT_FOUND, "Message not found during content update".to_string())),
            Err(e) => {
                error!("Failed to update message content for {}: {}", message_id, e);
                return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to update message content".to_string()));
            }
        }
    }

    if let Some(status) = payload.status {
        let target_message_id_for_status_update = updated_message_model.as_ref().map_or(&message_id, |m| m.id.as_ref().unwrap_or(&message_id));
        match db.update_message_status(target_message_id_for_status_update, status, payload.error_message).await {
            Ok(Some(msg)) => updated_message_model = Some(msg),
            Ok(None) => return Err((StatusCode::NOT_FOUND, "Message not found during status update".to_string())),
            Err(e) => {
                error!("Failed to update message status for {}: {}", message_id, e);
                return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to update message status".to_string()));
            }
        }
    }

    if let Some(final_message_model) = updated_message_model {
        Ok((StatusCode::OK, Json(LocalMessageResponse::from(final_message_model))))
    } else if payload.content.is_none() && payload.status.is_none() && payload.parts.is_none() {
        Ok((StatusCode::OK, Json(LocalMessageResponse::from(message))))
    } else {
        error!("Message update attempted for {} but no resulting message obtained.", message_id);
        Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to apply message updates".to_string()))
    }
}

async fn delete_message_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
    Path(message_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} deleting message {}", user.id, message_id);

    let message = match db.find_message_by_id(&message_id).await {
        Ok(Some(msg)) => msg,
        Ok(None) => return Err((StatusCode::NOT_FOUND, "Message not found".to_string())),
        Err(e) => {
            error!("Error finding message {}: {}", message_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify message".to_string()));
        }
    };

    match db.find_thread_by_id(&message.thread_id).await {
        Ok(Some(thread)) => {
            if thread.user_id != user.id {
                return Err((StatusCode::FORBIDDEN, "You don't have permission to delete this message".to_string()));
            }
        }
        Ok(None) => {
            error!("Data inconsistency: Message {} exists but its thread {} not found.", message_id, message.thread_id);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify message ownership due to data inconsistency".to_string()));
        }
        Err(e) => {
            error!("Error finding thread {} for message {}: {}", message.thread_id, message_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify message ownership".to_string()));
        }
    }

    match db.delete_message(&message_id).await {
        Ok(deleted_count) => {
            if deleted_count > 0 {
                Ok(StatusCode::NO_CONTENT)
            } else {
                Err((StatusCode::NOT_FOUND, "Message not found for deletion".to_string()))
            }
        }
        Err(e) => {
            error!("Failed to delete message {}: {}", message_id, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete message".to_string()))
        }
    }
}

async fn delete_trailing_messages_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
    Path(message_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} deleting trailing messages after message {}", user.id, message_id);

    let anchor_message = match db.find_message_by_id(&message_id).await {
        Ok(Some(msg)) => msg,
        Ok(None) => return Err((StatusCode::NOT_FOUND, "Anchor message not found".to_string())),
        Err(e) => {
            error!("Error finding anchor message {}: {}", message_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify anchor message".to_string()));
        }
    };

    match db.find_thread_by_id(&anchor_message.thread_id).await {
        Ok(Some(thread)) => {
            if thread.user_id != user.id {
                return Err((StatusCode::FORBIDDEN, "You don't have permission to modify messages in this thread".to_string()));
            }
        }
        Ok(None) => {
            error!("Data inconsistency: Anchor message {} exists but its thread {} not found.", message_id, anchor_message.thread_id);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Data inconsistency".to_string()));
        }
        Err(e) => {
            error!("Error finding thread {} for anchor message {}: {}", anchor_message.thread_id, message_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify ownership".to_string()));
        }
    }

    match db.delete_trailing_messages(&message_id).await {
        Ok(deleted_count) => Ok((
            StatusCode::OK,
            Json(DeletionResponse {
                deleted_count,
                message: format!("Successfully deleted {} trailing messages.", deleted_count),
            }),
        )),
        Err(e) => {
            error!("Failed to delete trailing messages for anchor {}: {}", message_id, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete trailing messages".to_string()))
        }
    }
}

async fn delete_message_and_trailing_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
    Path(message_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} deleting message {} and its trailing messages", user.id, message_id);

    let anchor_message = match db.find_message_by_id(&message_id).await {
        Ok(Some(msg)) => msg,
        Ok(None) => return Err((StatusCode::NOT_FOUND, "Anchor message not found".to_string())),
        Err(e) => {
            error!("Error finding anchor message {}: {}", message_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify anchor message".to_string()));
        }
    };

    match db.find_thread_by_id(&anchor_message.thread_id).await {
        Ok(Some(thread)) => {
            if thread.user_id != user.id {
                return Err((StatusCode::FORBIDDEN, "You don't have permission to modify messages in this thread".to_string()));
            }
        }
        Ok(None) => {
            error!("Data inconsistency: Anchor message {} exists but its thread {} not found.", message_id, anchor_message.thread_id);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Data inconsistency".to_string()));
        }
        Err(e) => {
            error!("Error finding thread {} for anchor message {}: {}", anchor_message.thread_id, message_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify ownership".to_string()));
        }
    }

    match db.delete_message_and_trailing(&message_id).await {
        Ok(deleted_count) => Ok((
            StatusCode::OK,
            Json(DeletionResponse {
                deleted_count,
                message: format!("Successfully deleted message and {} trailing messages. Total: {}", deleted_count.saturating_sub(1), deleted_count),
            }),
        )),
        Err(e) => {
            error!("Failed to delete message and trailing for anchor {}: {}", message_id, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete message and trailing messages".to_string()))
        }
    }
}
