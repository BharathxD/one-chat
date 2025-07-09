use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{
    auth::{auth_middleware, AuthenticatedUser},
    db::DBManager,
    models::{Thread, Visibility, generate_id as generate_model_id},
};

// Request body for creating a new thread
#[derive(Deserialize)]
pub struct CreateThreadPayload {
    title: Option<String>,
    visibility: Option<Visibility>,
}

// Response for a single thread
#[derive(Serialize)]
pub struct ThreadResponse {
    id: String,
    user_id: String,
    title: String,
    visibility: Visibility,
    origin_thread_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<Thread> for ThreadResponse {
    fn from(thread: Thread) -> Self {
        ThreadResponse {
            id: thread.id.unwrap_or_default(),
            user_id: thread.user_id,
            title: thread.title,
            visibility: thread.visibility,
            origin_thread_id: thread.origin_thread_id,
            created_at: thread.created_at.to_rfc3339(),
            updated_at: thread.updated_at.to_rfc3339(),
        }
    }
}

// --- Message Structs ---
#[derive(Deserialize)]
pub struct CreateMessagePayload {
    role: crate::models::Role,
    content: Option<String>,
    parts: serde_json::Value,
    model: Option<String>,
    status: Option<crate::models::Status>,
    annotations: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct MessageResponse {
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

impl From<crate::models::Message> for MessageResponse {
    fn from(msg: crate::models::Message) -> Self {
        MessageResponse {
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

#[derive(Deserialize)]
struct ToggleVisibilityPayload {
    visibility: Visibility,
}

#[derive(Deserialize)]
struct BranchOutPayload {
    anchor_message_id: String,
    new_thread_id: Option<String>,
}

#[derive(Deserialize)]
struct GenerateTitlePayload {
    user_query: String,
}


// Router function to be called from main.rs
pub fn thread_router() -> Router {
    Router::new()
        .route("/", post(create_thread_handler))
        .route("/", get(get_user_threads_handler))
        .route("/:thread_id", get(get_thread_handler))
        .route("/:thread_id", delete(delete_thread_handler))
        .route("/:thread_id/visibility", put(toggle_thread_visibility_handler))
        .route("/:original_thread_id/branch", post(branch_out_handler))
        .route("/:thread_id/generate-title", post(generate_thread_title_handler))
        // Message routes nested under a thread
        .route("/:thread_id/messages", post(create_message_handler))
        .route("/:thread_id/messages", get(list_messages_handler))
        .route_layer(middleware::from_fn(auth_middleware))
}


async fn create_thread_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateThreadPayload>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} creating thread with title: {:?}", user.id, payload.title);
    match db.create_thread(&user.id, payload.title, payload.visibility).await {
        Ok(thread) => Ok((StatusCode::CREATED, Json(ThreadResponse::from(thread)))),
        Err(e) => {
            error!("Failed to create thread: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to create thread".to_string()))
        }
    }
}

async fn get_user_threads_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("Fetching threads for user {}", user.id);
    match db.find_threads_by_user_id(&user.id).await {
        Ok(threads) => {
            let thread_responses: Vec<ThreadResponse> =
                threads.into_iter().map(ThreadResponse::from).collect();
            Ok(Json(thread_responses))
        }
        Err(e) => {
            error!("Failed to fetch user threads: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch threads".to_string()))
        }
    }
}

async fn get_thread_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
    Path(thread_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("Fetching thread {} for user {}", thread_id, user.id);
    match db.find_thread_by_id(&thread_id).await {
        Ok(Some(thread)) => {
            if thread.user_id == user.id || thread.visibility == Visibility::Public {
                Ok(Json(ThreadResponse::from(thread)))
            } else {
                Err((StatusCode::FORBIDDEN, "You don't have permission to access this thread".to_string()))
            }
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, "Thread not found".to_string())),
        Err(e) => {
            error!("Failed to fetch thread {}: {}", thread_id, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch thread".to_string()))
        }
    }
}

async fn delete_thread_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
    Path(thread_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} attempting to delete thread {}", user.id, thread_id);
    match db.find_thread_by_id(&thread_id).await {
        Ok(Some(thread)) => {
            if thread.user_id != user.id {
                return Err((StatusCode::FORBIDDEN, "You don't have permission to delete this thread".to_string()));
            }
        }
        Ok(None) => return Err((StatusCode::NOT_FOUND, "Thread not found".to_string())),
        Err(e) => {
            error!("Error finding thread {} for deletion: {}", thread_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to process thread deletion".to_string()));
        }
    }

    match db.delete_thread(&thread_id).await {
        Ok(deleted_count) => {
            if deleted_count > 0 {
                Ok((StatusCode::NO_CONTENT, "".to_string()))
            } else {
                Err((StatusCode::NOT_FOUND, "Thread not found for deletion".to_string()))
            }
        }
        Err(e) => {
            error!("Failed to delete thread {}: {}", thread_id, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete thread".to_string()))
        }
    }
}

async fn toggle_thread_visibility_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
    Path(thread_id): Path<String>,
    Json(payload): Json<ToggleVisibilityPayload>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} attempting to toggle visibility of thread {} to {:?}", user.id, thread_id, payload.visibility);
    match db.find_thread_by_id(&thread_id).await {
        Ok(Some(thread)) => {
            if thread.user_id != user.id {
                return Err((StatusCode::FORBIDDEN, "You don't have permission to change this thread's visibility".to_string()));
            }
        }
        Ok(None) => return Err((StatusCode::NOT_FOUND, "Thread not found".to_string())),
        Err(e) => {
            error!("Error finding thread {}: {}", thread_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify thread".to_string()));
        }
    }

    match db.update_thread_visibility(&thread_id, payload.visibility).await {
        Ok(Some(updated_thread)) => Ok(Json(ThreadResponse::from(updated_thread))),
        Ok(None) => Err((StatusCode::NOT_FOUND, "Thread not found during update".to_string())),
        Err(e) => {
            error!("Failed to update thread visibility for {}: {}", thread_id, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to update thread visibility".to_string()))
        }
    }
}

async fn branch_out_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
    Path(original_thread_id): Path<String>,
    Json(payload): Json<BranchOutPayload>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} branching from thread {} at message {}, new thread ID suggested: {:?}", user.id, original_thread_id, payload.anchor_message_id, payload.new_thread_id);
    let new_thread_id = payload.new_thread_id.unwrap_or_else(generate_model_id);
    match db.branch_out_from_message(&user.id, &original_thread_id, &payload.anchor_message_id, &new_thread_id).await {
        Ok(new_thread) => Ok((StatusCode::CREATED, Json(ThreadResponse::from(new_thread)))),
        Err(e) => {
            error!("Failed to branch out from thread {}: {}. Anchor: {}, New ID: {}", original_thread_id, e, payload.anchor_message_id, new_thread_id);
            if e.to_string().contains("Original thread not found") || e.to_string().contains("Anchor message not found") {
                 Err((StatusCode::NOT_FOUND, e.to_string()))
            } else if e.to_string().contains("does not have permission") || e.to_string().contains("does not belong to the original thread") {
                 Err((StatusCode::FORBIDDEN, e.to_string()))
            } else {
                 Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to branch out thread".to_string()))
            }
        }
    }
}

async fn generate_thread_title_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
    Path(thread_id): Path<String>,
    Json(payload): Json<GenerateTitlePayload>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} requesting title generation for thread {} based on query: {:.50}...", user.id, thread_id, payload.user_query);

    match db.find_thread_by_id(&thread_id).await {
        Ok(Some(thread)) => {
            if thread.user_id != user.id {
                return Err((StatusCode::FORBIDDEN, "You don't have permission to modify this thread".to_string()));
            }
        }
        Ok(None) => return Err((StatusCode::NOT_FOUND, "Thread not found".to_string())),
        Err(e) => {
            error!("Error finding thread {}: {}", thread_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify thread".to_string()));
        }
    }

    let generated_title = match crate::ai_services::generate_title_for_prompt(&payload.user_query).await {
        Ok(title) => title,
        Err(e) => {
            error!("AI title generation failed for thread {}: {}", thread_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("AI title generation failed: {}", e)));
        }
    };

    match db.update_thread_title(&thread_id, &generated_title).await {
        Ok(Some(updated_thread)) => Ok((StatusCode::OK, Json(ThreadResponse::from(updated_thread)))),
        Ok(None) => Err((StatusCode::NOT_FOUND, "Thread not found during title update".to_string())),
        Err(e) => {
            error!("Failed to update thread title for {}: {}", thread_id, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to update thread title".to_string()))
        }
    }
}

// --- Message Handlers (within thread_routes.rs) ---

async fn create_message_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
    Path(thread_id): Path<String>,
    Json(payload): Json<CreateMessagePayload>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} creating message in thread {} with role: {:?}", user.id, thread_id, payload.role);
    match db.find_thread_by_id(&thread_id).await {
        Ok(Some(thread)) => {
            if thread.user_id != user.id && thread.visibility == Visibility::Private {
                return Err((StatusCode::FORBIDDEN, "You don't have permission to add messages to this thread".to_string()));
            }
        }
        Ok(None) => return Err((StatusCode::NOT_FOUND, "Thread not found".to_string())),
        Err(e) => {
            error!("Error finding thread {}: {}", thread_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify thread".to_string()));
        }
    }

    match db.create_message(&thread_id, payload.role, payload.content, payload.parts, payload.model, payload.status, payload.annotations).await {
        Ok(message) => Ok((StatusCode::CREATED, Json(MessageResponse::from(message)))),
        Err(e) => {
            error!("Failed to create message in thread {}: {}", thread_id, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to create message".to_string()))
        }
    }
}

async fn list_messages_handler(
    Extension(db): Extension<DBManager>,
    user: AuthenticatedUser,
    Path(thread_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} listing messages for thread {}", user.id, thread_id);
    match db.find_thread_by_id(&thread_id).await {
        Ok(Some(thread)) => {
            if thread.user_id != user.id && thread.visibility == Visibility::Private {
                return Err((StatusCode::FORBIDDEN, "You don't have permission to view messages in this thread".to_string()));
            }
        }
        Ok(None) => return Err((StatusCode::NOT_FOUND, "Thread not found".to_string())),
        Err(e) => {
            error!("Error finding thread {}: {}", thread_id, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify thread".to_string()));
        }
    }

    match db.find_messages_by_thread_id(&thread_id).await {
        Ok(messages) => {
            let message_responses: Vec<MessageResponse> = messages.into_iter().map(MessageResponse::from).collect();
            Ok(Json(message_responses))
        }
        Err(e) => {
            error!("Failed to fetch messages for thread {}: {}", thread_id, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch messages".to_string()))
        }
    }
}
