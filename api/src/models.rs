use chrono::{DateTime, Utc};
use mongodb::bson::{doc, oid::ObjectId}; // oid::ObjectId might not be used if we stick to string IDs
use serde::{Deserialize, Serialize};
use serde_json::Value; // For fields that were JSONB

// Enums matching the Drizzle schema

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    User,
    Assistant,
    System,
    Data,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Pending,
    Streaming,
    Done,
    Error,
    Stopped,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Visibility {
    Private,
    Public,
}

// User model (basic version for now)
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>, // Or Option<ObjectId> if using MongoDB ObjectIds
    // Add other user fields as necessary, e.g., email, name
    // For now, matching the reference in Thread: userId: varchar("user_id")
    pub external_id: String, // This would correspond to the ID from the auth provider
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>, // Corresponds to nanoid generated ID
    pub user_id: String,    // Foreign key to User's external_id or internal _id
    #[serde(default = "default_thread_title")]
    pub title: String,
    #[serde(default = "default_visibility")]
    pub visibility: Visibility,
    pub origin_thread_id: Option<String>,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

fn default_thread_title() -> String {
    "New Thread".to_string()
}

fn default_visibility() -> Visibility {
    Visibility::Private
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>, // Corresponds to nanoid generated ID
    pub thread_id: String,
    pub parts: Value, // JSONB in Postgres, maps to BSON document or array
    pub content: Option<String>,
    pub role: Role,
    #[serde(default)]
    pub annotations: Option<Value>, // JSONB array in Postgres
    pub model: Option<String>,
    #[serde(default = "default_status")]
    pub status: Status,
    #[serde(default)]
    pub is_errored: bool,
    #[serde(default)]
    pub is_stopped: bool,
    pub error_message: Option<String>,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

fn default_status() -> Status {
    Status::Done
}

// Helper for generating string IDs if not using MongoDB's ObjectId
// You might use the `nanoid` crate or similar if you want to replicate that.
// For now, this is just a conceptual placeholder.
pub fn generate_id() -> String {
    // In a real app, use a proper ID generation library like nanoid or uuid
    // For simplicity, using a basic ObjectId string representation here,
    // but actual nanoid would be different.
    ObjectId::new().to_hex()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PartialShare {
    #[serde(rename = "_id")] // Use token as the _id for MongoDB
    pub token: String, // User-generated or server-generated unique token
    pub thread_id: String,
    pub user_id: String, // The user who created this share link
    pub shared_up_to_message_id: String,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}


// Example of how you might add these to your main.rs or lib.rs
// pub mod models;
// use models::{User, Thread, Message, Role, Status, Visibility};
