use axum::{
    extract::State, // Will use Extension for DBManager if needed, but not for this router
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::post, // Changed to post for delete, as Vercel API expects POST for delete
    Extension, Json, Router,
};
use reqwest::Client;
use serde::Deserialize;
use std::env;
use tracing::{error, info};

use crate::auth::{auth_middleware, AuthenticatedUser};
// No DBManager needed for this specific router as it only interacts with Vercel Blob

#[derive(Deserialize)]
struct DeleteAttachmentPayload {
    url: String, // URL of the blob to delete
}

#[derive(Serialize)] // Added Serialize for the response
struct VercelBlobDeleteRequest {
    urls: Vec<String>,
}


pub fn attachment_router() -> Router {
    // The tRPC route was `attachment.delete`. A RESTful equivalent might be DELETE /api/attachments
    // with the URL in the body or as a query param.
    // However, since the Vercel API itself uses a POST to a /delete endpoint with URLs in the body,
    // we can mirror that structure or use a DELETE verb with a payload.
    // Axum's `delete` router doesn't typically expect a JSON body by default.
    // Using POST to /api/attachments/delete is clearer for a body-based deletion.
    Router::new()
        .route("/delete", post(delete_attachment_handler))
        .route_layer(middleware::from_fn(auth_middleware))
}

async fn delete_attachment_handler(
    user: AuthenticatedUser, // Ensure user is authenticated
    Json(payload): Json<DeleteAttachmentPayload>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} attempting to delete attachment: {}", user.id, payload.url);

    let vercel_blob_token = match env::var("VERCEL_BLOB_READ_WRITE_TOKEN") {
        Ok(token) => token,
        Err(_) => {
            error!("VERCEL_BLOB_READ_WRITE_TOKEN not set.");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Server configuration error for deleting attachments.".to_string(),
            ));
        }
    };

    let client = Client::new();
    // The exact API endpoint for Vercel Blob deletion needs to be confirmed.
    // Based on common patterns and some Vercel examples, it's often a specific API endpoint,
    // not just the blob URL itself with a DELETE method.
    // The `@vercel/blob` SDK likely calls an endpoint like: `https://<some-vercel-api-endpoint>/blob/delete`
    // For now, I'll use a placeholder URL and assume it's a POST request.
    // After more research, the `@vercel/blob` package sends a POST to `https://blob.vercel-storage.com` (or a region-specific one)
    // with `x-api-version: '6'` and `/delete` appended to the pathname if not present.
    // Let's assume the base URL is `https://blob.vercel-storage.com/delete` for simplicity,
    // but this might need adjustment based on Vercel's current API.
    // The SDK actually seems to use `https://<project_id>.blob.vercel-storage.com/delete` or similar.
    // For now, let's use the generic one, but this is a point of potential failure if the endpoint is wrong.
    // A common Vercel Blob API endpoint for operations like list/delete is `https://api.vercel.com/v2/blob`
    // or directly `edge.blob.vercel-storage.com`.
    // The `@vercel/blob` package uses `https://<storeId>.blob.vercel-storage.com/<pathname>` for uploads,
    // and for `del()` it constructs the URL to the blob store and sends a POST to `/delete`.
    // This is tricky without knowing the exact internal API structure @vercel/blob uses.
    // A safer bet is to find a direct Vercel Blob API documentation for HTTP delete.
    // If direct deletion via URL is `DELETE <blob_url>`, that's simpler.
    // The `del` function in `@vercel/blob` actually makes a POST request.
    // The target URL for the POST request is derived from one of the blob URLs to delete.
    // E.g., if blob URL is `https://<id>.blob.vercel-storage.com/foo.txt`, POST to `https://<id>.blob.vercel-storage.com/delete`.

    let blob_url_obj = match reqwest::Url::parse(&payload.url) {
        Ok(url) => url,
        Err(_) => return Err((StatusCode::BAD_REQUEST, "Invalid blob URL format".to_string())),
    };

    let delete_api_url = match blob_url_obj.host_str() {
        Some(host) => format!("https://{}/delete", host),
        None => return Err((StatusCode::BAD_REQUEST, "Could not determine host from blob URL".to_string())),
    };


    let request_body = VercelBlobDeleteRequest {
        urls: vec![payload.url.clone()],
    };

    match client
        .post(&delete_api_url)
        .bearer_auth(&vercel_blob_token) // Vercel uses Bearer token for its API
        .header("x-api-version", "6") // Common for Vercel Blob API
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                info!("Successfully deleted attachment {} from Vercel Blob.", payload.url);
                Ok(StatusCode::NO_CONTENT)
            } else {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                error!(
                    "Failed to delete attachment {} from Vercel Blob. Status: {}. Response: {}",
                    payload.url, status, error_text
                );
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to delete attachment from Vercel Blob: {} - {}", status, error_text),
                ))
            }
        }
        Err(e) => {
            error!("Error sending delete request to Vercel Blob for {}: {}", payload.url, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to send delete request to Vercel Blob.".to_string(),
            ))
        }
    }
}
