use mongodb::{
    bson::{doc, Bson}, // Added Bson for potential future use with updates
    Client, Database, error::Result as MongoResult, Collection
};
use std::env;
use tracing::info;
use futures::stream::TryStreamExt; // For cursor.try_next()

use crate::models::{User, generate_id}; // Import User model and id generator

// A struct to hold the MongoDB client and database instances.
#[derive(Clone)]
pub struct DBManager {
    #[allow(dead_code)] // Client might be used for more advanced scenarios later
    client: Client,
    database: Database,
}

impl DBManager {
    pub async fn new() -> MongoResult<Self> {
        let db_uri = env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set in .env or environment");

        info!("Connecting to MongoDB at: {}", db_uri);
        let client = Client::with_uri_str(&db_uri).await?;

        let db_name = client.default_database().map(|db| db.name().to_string())
            .or_else(|| {
                mongodb::options::ClientOptions::parse(&db_uri).await
                    .ok()
                    .and_then(|opts| opts.default_database)
            })
            .unwrap_or_else(|| {
                info!("No database name found in DATABASE_URL, using default 'axum_chat_db'");
                "axum_chat_db".to_string()
            });

        info!("Using database: {}", db_name);
        let database = client.database(&db_name);

        client
            .database("admin")
            .run_command(mongodb::bson::doc! {"ping": 1}, None)
            .await?;
        info!("Successfully connected to MongoDB and pinged admin database.");

        Ok(DBManager { client, database })
    }

    // Generic method to get a handle to a collection
    fn get_collection<T>(&self, collection_name: &str) -> Collection<T> {
        self.database.collection::<T>(collection_name)
    }

    // Specific getters for collections
    pub fn users_collection(&self) -> Collection<User> {
        self.get_collection("users")
    }

    pub fn threads_collection(&self) -> Collection<crate::models::Thread> {
        self.get_collection("threads")
    }

    pub fn messages_collection(&self) -> Collection<crate::models::Message> {
        self.get_collection("messages")
    }

    // --- User Operations ---

    /// Creates a new user if one with the same external_id doesn't already exist.
    /// Returns the created or existing user.
    pub async fn create_user_if_not_exists(&self, external_id: &str) -> MongoResult<User> {
        let users_coll = self.users_collection();

        // Check if user already exists
        if let Some(existing_user) = self.find_user_by_external_id(external_id).await? {
            info!("User with external_id '{}' already exists.", external_id);
            return Ok(existing_user);
        }

        info!("Creating new user with external_id '{}'.", external_id);
        let new_user_id = generate_id(); // Generate our string ID
        let new_user = User {
            id: Some(new_user_id.clone()), // Store our generated ID in the _id field for MongoDB
            external_id: external_id.to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        users_coll.insert_one(&new_user, None).await?;
        // The insert_one operation doesn't return the document by default with our setup.
        // We return the `new_user` struct we constructed.
        // If MongoDB generated the _id, we might need to fetch it. But we set it.
        Ok(new_user)
    }

    pub async fn find_user_by_external_id(&self, external_id: &str) -> MongoResult<Option<User>> {
        let users_coll = self.users_collection();
        users_coll.find_one(doc! { "externalId": external_id }, None).await // Note: camelCase from Serde
    }

    pub async fn find_user_by_id(&self, user_id: &str) -> MongoResult<Option<User>> {
        let users_coll = self.users_collection();
        users_coll.find_one(doc! { "_id": user_id }, None).await
    }

    // --- Thread Operations ---

    pub async fn create_thread(&self, user_id: &str, title: Option<String>, visibility: Option<crate::models::Visibility>) -> MongoResult<crate::models::Thread> {
        let threads_coll = self.threads_collection();
        let new_thread_id = generate_id();
        let now = chrono::Utc::now();

        let thread = crate::models::Thread {
            id: Some(new_thread_id.clone()),
            user_id: user_id.to_string(),
            title: title.unwrap_or_else(|| "New Thread".to_string()),
            visibility: visibility.unwrap_or(crate::models::Visibility::Private),
            origin_thread_id: None,
            created_at: now,
            updated_at: now,
        };

        threads_coll.insert_one(&thread, None).await?;
        Ok(thread)
    }

    pub async fn find_thread_by_id(&self, thread_id: &str) -> MongoResult<Option<crate::models::Thread>> {
        let threads_coll = self.threads_collection();
        threads_coll.find_one(doc! { "_id": thread_id }, None).await
    }

    pub async fn find_threads_by_user_id(&self, user_id: &str) -> MongoResult<Vec<crate::models::Thread>> {
        let threads_coll = self.threads_collection();
        let mut cursor = threads_coll.find(doc! { "userId": user_id }, None).await?; // camelCase from Serde
        let mut threads = Vec::new();
        while let Some(result) = cursor.try_next().await? { // use futures::stream::TryStreamExt;
            threads.push(result);
        }
        Ok(threads)
    }

    pub async fn update_thread_title(&self, thread_id: &str, new_title: &str) -> MongoResult<Option<crate::models::Thread>> {
        let threads_coll = self.threads_collection();
        let now = chrono::Utc::now();
        let update_doc = doc! {
            "$set": {
                "title": new_title,
                "updatedAt": mongodb::bson::DateTime::from_chrono(now) // Ensure BSON DateTime
            }
        };

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(mongodb::options::ReturnDocument::After)
            .build();

        threads_coll.find_one_and_update(doc! { "_id": thread_id }, update_doc, options).await
    }

    pub async fn update_thread_visibility(&self, thread_id: &str, visibility: crate::models::Visibility) -> MongoResult<Option<crate::models::Thread>> {
        let threads_coll = self.threads_collection();
        let now = chrono::Utc::now();
        let update_doc = doc! {
            "$set": {
                "visibility": serde_json::to_value(&visibility).unwrap_or(mongodb::bson::Bson::Null), // Ensure enum is serialized correctly for BSON
                "updatedAt": mongodb::bson::DateTime::from_chrono(now)
            }
        };
        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(mongodb::options::ReturnDocument::After)
            .build();
        threads_coll.find_one_and_update(doc!{ "_id": thread_id }, update_doc, options).await
    }


    pub async fn delete_thread(&self, thread_id: &str) -> MongoResult<u64> {
        let threads_coll = self.threads_collection();
        // Also consider deleting associated messages (cascade delete logic)
        // For now, just deleting the thread.
        let result = threads_coll.delete_one(doc! { "_id": thread_id }, None).await?;
        if result.deleted_count > 0 {
            // If thread is deleted, also delete its messages
            self.delete_messages_by_thread_id(thread_id).await?;
        }
        Ok(result.deleted_count)
    }

    // --- Message Operations ---

    pub async fn create_message(
        &self,
        thread_id: &str,
        role: crate::models::Role,
        content: Option<String>,
        parts: serde_json::Value, // Assuming parts is flexible JSON
        model: Option<String>,
        status: Option<crate::models::Status>,
        annotations: Option<serde_json::Value>,
    ) -> MongoResult<crate::models::Message> {
        let messages_coll = self.messages_collection();
        let new_message_id = generate_id();
        let now = chrono::Utc::now();

        let message = crate::models::Message {
            id: Some(new_message_id.clone()),
            thread_id: thread_id.to_string(),
            parts,
            content,
            role,
            annotations,
            model,
            status: status.unwrap_or(crate::models::Status::Done),
            is_errored: false,
            is_stopped: false,
            error_message: None,
            created_at: now,
            updated_at: now,
        };

        messages_coll.insert_one(&message, None).await?;
        Ok(message)
    }

    pub async fn find_messages_by_thread_id(
        &self,
        thread_id: &str,
        // Add options for pagination, sorting (e.g., by created_at)
        // limit: Option<i64>,
        // skip: Option<u64>,
        // sort_by_creation: Option<bool>, // true for asc, false for desc
    ) -> MongoResult<Vec<crate::models::Message>> {
        let messages_coll = self.messages_collection();

        // Example: Sort by createdAt ascending by default
        let find_options = mongodb::options::FindOptions::builder()
            .sort(doc! { "createdAt": 1 }) // 1 for ascending, -1 for descending
            // .limit(limit)
            // .skip(skip)
            .build();

        let mut cursor = messages_coll.find(doc! { "threadId": thread_id }, find_options).await?;
        let mut messages = Vec::new();
        while let Some(result) = cursor.try_next().await? {
            messages.push(result);
        }
        Ok(messages)
    }

    pub async fn find_message_by_id(&self, message_id: &str) -> MongoResult<Option<crate::models::Message>> {
        let messages_coll = self.messages_collection();
        messages_coll.find_one(doc! { "_id": message_id }, None).await
    }


    pub async fn update_message_content(&self, message_id: &str, new_content: &str, new_parts: Option<serde_json::Value>) -> MongoResult<Option<crate::models::Message>> {
        let messages_coll = self.messages_collection();
        let now = chrono::Utc::now();

        let mut set_doc = doc! {
            "content": new_content,
            "updatedAt": mongodb::bson::DateTime::from_chrono(now)
        };
        if let Some(parts) = new_parts {
            match mongodb::bson::to_bson(&parts) {
                Ok(bson_parts) => { set_doc.insert("parts", bson_parts); },
                Err(e) => {
                    tracing::error!("Failed to serialize parts to BSON: {}", e);
                    // Decide how to handle: error out, or proceed without updating parts
                }
            }
        }

        let update_doc = doc! { "$set": set_doc };

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(mongodb::options::ReturnDocument::After)
            .build();

        messages_coll.find_one_and_update(doc! { "_id": message_id }, update_doc, options).await
    }

    /// Updates the status of a message.
    pub async fn update_message_status(&self, message_id: &str, status: crate::models::Status, error_message: Option<String>) -> MongoResult<Option<crate::models::Message>> {
        let messages_coll = self.messages_collection();
        let now = chrono::Utc::now();
        let mut set_doc = doc! {
            "status": serde_json::to_value(&status).unwrap_or(mongodb::bson::Bson::Null),
            "updatedAt": mongodb::bson::DateTime::from_chrono(now)
        };
        if status == crate::models::Status::Error {
            set_doc.insert("isErrored", true);
            if let Some(err_msg) = error_message {
                set_doc.insert("errorMessage", err_msg);
            }
        } else {
            set_doc.insert("isErrored", false);
            set_doc.insert("errorMessage", mongodb::bson::Bson::Null); // Clear error message if not in error state
        }
         if status == crate::models::Status::Stopped {
            set_doc.insert("isStopped", true);
        }


        let update_doc = doc! { "$set": set_doc };
        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(mongodb::options::ReturnDocument::After)
            .build();
        messages_coll.find_one_and_update(doc!{ "_id": message_id }, update_doc, options).await
    }


    pub async fn delete_message(&self, message_id: &str) -> MongoResult<u64> {
        let messages_coll = self.messages_collection();
        let result = messages_coll.delete_one(doc! { "_id": message_id }, None).await?;
        Ok(result.deleted_count)
    }

    /// Deletes all messages associated with a given thread_id.
    pub async fn delete_messages_by_thread_id(&self, thread_id: &str) -> MongoResult<u64> {
        let messages_coll = self.messages_collection();
        let result = messages_coll.delete_many(doc! { "threadId": thread_id }, None).await?;
        info!("Deleted {} messages for thread_id '{}'", result.deleted_count, thread_id);
        Ok(result.deleted_count)
    }

    // --- Complex Message Deletion Operations ---

    /// Deletes all messages in the same thread that were created *after* the message with `message_id_anchor`.
    pub async fn delete_trailing_messages(&self, message_id_anchor: &str) -> MongoResult<u64> {
        let messages_coll = self.messages_collection();

        // 1. Find the anchor message to get its thread_id and created_at
        let anchor_message = match self.find_message_by_id(message_id_anchor).await? {
            Some(msg) => msg,
            None => {
                info!("Anchor message {} not found for deleting trailing messages.", message_id_anchor);
                return Ok(0); // No message, so nothing to delete
            }
        };

        // 2. Delete messages in the same thread created after the anchor message
        let filter = doc! {
            "threadId": anchor_message.thread_id,
            "createdAt": { "$gt": mongodb::bson::DateTime::from_chrono(anchor_message.created_at) },
            "_id": { "$ne": anchor_message.id.unwrap_or_default() } // Ensure anchor itself isn't included if somehow timestamp is exact
        };
        let result = messages_coll.delete_many(filter, None).await?;
        info!("Deleted {} trailing messages after message_id '{}'", result.deleted_count, message_id_anchor);
        Ok(result.deleted_count)
    }

    /// Deletes the message with `message_id_anchor` AND all messages in the same thread created after it.
    pub async fn delete_message_and_trailing(&self, message_id_anchor: &str) -> MongoResult<u64> {
        let messages_coll = self.messages_collection();

        // 1. Find the anchor message
        let anchor_message = match self.find_message_by_id(message_id_anchor).await? {
            Some(msg) => msg,
            None => {
                info!("Anchor message {} not found for deleting message and trailing.", message_id_anchor);
                return Ok(0);
            }
        };

        // 2. Delete messages in the same thread created at or after the anchor message's timestamp
        // This includes the anchor message itself.
        let filter = doc! {
            "threadId": anchor_message.thread_id,
            "createdAt": { "$gte": mongodb::bson::DateTime::from_chrono(anchor_message.created_at) }
        };
        // A more precise way if IDs are ordered or if timestamps can be identical for multiple messages:
        // First delete trailing, then delete anchor. Or, if IDs are sortable and sequential by time,
        // one could use a filter that combines createdAt and ID.
        // The current filter `$gte createdAt` should cover the anchor message if its timestamp is unique at that moment or part of the batch.
        // Let's refine to ensure the anchor is definitely included, even if other messages share the exact timestamp.
        // This can be complex if multiple messages have the exact same `createdAt`.
        // A simpler approach for "message and trailing" is to delete all messages with `createdAt >= anchor.createdAt`
        // OR whose ID is the anchor_message_id.

        // Alternative: Delete trailing first, then the anchor message. This is cleaner.
        let trailing_deleted_count = self.delete_trailing_messages(message_id_anchor).await?;

        let anchor_delete_result = messages_coll.delete_one(doc!{ "_id": message_id_anchor }, None).await?;

        let total_deleted = trailing_deleted_count + anchor_delete_result.deleted_count;
        info!("Deleted message_id '{}' and {} trailing messages. Total: {}", message_id_anchor, trailing_deleted_count, total_deleted);
        Ok(total_deleted)
    }

    // --- Complex Thread Operations ---
    pub async fn branch_out_from_message(
        &self,
        user_id: &str,
        original_thread_id: &str,
        anchor_message_id: &str,
        new_thread_id_val: &str, // Use a different name to avoid conflict with model field if any
    ) -> MongoResult<crate::models::Thread> {
        let threads_coll = self.threads_collection();
        let messages_coll = self.messages_collection();

        // 1. Find the original thread and the anchor message
        let original_thread = match self.find_thread_by_id(original_thread_id).await? {
            Some(t) => t,
            None => return Err(mongodb::error::Error::custom(anyhow::anyhow!("Original thread not found"))),
        };
        if original_thread.user_id != user_id && original_thread.visibility == crate::models::Visibility::Private {
             return Err(mongodb::error::Error::custom(anyhow::anyhow!("User does not have permission to branch from this thread")));
        }


        let anchor_message = match self.find_message_by_id(anchor_message_id).await? {
            Some(m) => {
                if m.thread_id != original_thread_id {
                    return Err(mongodb::error::Error::custom(anyhow::anyhow!("Anchor message does not belong to the original thread")));
                }
                m
            }
            None => return Err(mongodb::error::Error::custom(anyhow::anyhow!("Anchor message not found"))),
        };

        // 2. Get all messages from the original thread up to and including the anchor message, sorted by creation time
        let filter = doc! {
            "threadId": original_thread_id,
            "createdAt": { "$lte": mongodb::bson::DateTime::from_chrono(anchor_message.created_at) }
        };
        let sort_options = mongodb::options::FindOptions::builder().sort(doc! { "createdAt": 1 }).build();
        let mut cursor = messages_coll.find(filter, sort_options).await?;

        let mut messages_to_copy = Vec::new();
        let mut found_anchor_in_cursor = false;
        while let Some(msg_result) = cursor.try_next().await? {
            messages_to_copy.push(msg_result.clone());
            if msg_result.id.as_deref() == Some(anchor_message_id) {
                found_anchor_in_cursor = true;
                // break; // Stop if we want messages strictly up to and including the anchor.
                        // If multiple messages can have the same timestamp, $lte might grab more than desired if not careful.
                        // However, since we are iterating and collecting, this ensures we get all relevant ones up to the anchor's timestamp.
                        // And we explicitly check if the anchor_message_id itself was found.
            }
        }
         if !found_anchor_in_cursor && !messages_to_copy.iter().any(|m: &crate::models::Message| m.id.as_deref() == Some(anchor_message_id)) {
             // This might happen if anchor_message has a later timestamp than what $lte picked up,
             // or if it wasn't part of the sorted list up to its own timestamp (highly unlikely with correct sorting).
             // For safety, if it wasn't in the list, add it.
             let still_not_found = messages_to_copy.iter().all(|m: &crate::models::Message| m.id.as_deref() != Some(anchor_message_id));
             if still_not_found {
                 messages_to_copy.push(anchor_message.clone()); // Ensure anchor is included
                 // Re-sort if necessary, though if anchor was the last, it's fine.
                 messages_to_copy.sort_by_key(|m| m.created_at);
             }
        }


        if messages_to_copy.is_empty() {
            return Err(mongodb::error::Error::custom(anyhow::anyhow!("No messages found to branch from, including the anchor message")));
        }


        // 3. Create the new thread
        let now = chrono::Utc::now();
        let new_thread_title = format!("Branch of {}", original_thread.title); // Or some other default

        let new_branched_thread = crate::models::Thread {
            id: Some(new_thread_id_val.to_string()),
            user_id: user_id.to_string(),
            title: new_thread_title,
            visibility: original_thread.visibility, // Or default to private
            origin_thread_id: Some(original_thread_id.to_string()),
            created_at: now,
            updated_at: now,
        };
        threads_coll.insert_one(&new_branched_thread, None).await?;

        // 4. Copy messages to the new thread
        let mut new_messages_for_branch = Vec::new();
        for old_msg in messages_to_copy {
            let new_msg_id = generate_id(); // Generate new ID for each copied message
            let copied_msg = crate::models::Message {
                id: Some(new_msg_id),
                thread_id: new_thread_id_val.to_string(), // Link to the new thread
                parts: old_msg.parts.clone(),
                content: old_msg.content.clone(),
                role: old_msg.role.clone(),
                annotations: old_msg.annotations.clone(),
                model: old_msg.model.clone(),
                status: old_msg.status.clone(), // Or reset status, e.g., to Done
                is_errored: old_msg.is_errored,
                is_stopped: old_msg.is_stopped,
                error_message: old_msg.error_message.clone(),
                created_at: old_msg.created_at, // Preserve original creation time for sorting
                updated_at: now, // Set new updated_at time
            };
            new_messages_for_branch.push(copied_msg);
        }

        if !new_messages_for_branch.is_empty() {
            messages_coll.insert_many(new_messages_for_branch, None).await?;
        }

        Ok(new_branched_thread)
    }

    // --- PartialShare Operations ---

    pub fn partial_shares_collection(&self) -> Collection<crate::models::PartialShare> {
        self.get_collection("partial_shares")
    }

    pub async fn create_partial_share(
        &self,
        token: String, // Allow client to suggest a token or generate if needed
        thread_id: &str,
        user_id: &str,
        shared_up_to_message_id: &str,
    ) -> MongoResult<crate::models::PartialShare> {
        let shares_coll = self.partial_shares_collection();
        let now = chrono::Utc::now();

        // Check if thread and message exist, and user owns thread (optional, but good practice)
        let thread = self.find_thread_by_id(thread_id).await?.ok_or_else(|| mongodb::error::Error::custom(anyhow::anyhow!("Thread not found")))?;
        if thread.user_id != user_id {
            return Err(mongodb::error::Error::custom(anyhow::anyhow!("User does not own the thread")));
        }
        self.find_message_by_id(shared_up_to_message_id).await?.ok_or_else(|| mongodb::error::Error::custom(anyhow::anyhow!("Anchor message for share not found")))?;


        let partial_share = crate::models::PartialShare {
            token: token.clone(),
            thread_id: thread_id.to_string(),
            user_id: user_id.to_string(),
            shared_up_to_message_id: shared_up_to_message_id.to_string(),
            created_at: now,
            updated_at: now,
        };
        // Use update_one with upsert to handle cases where a token might be re-used/updated,
        // or insert_one if tokens must be unique on creation.
        // For simplicity, assuming token is unique and new here. If client can suggest, check existence first.
        // If token is generated server-side, insert_one is fine.
        // Let's assume the token is provided and should be unique.
        match shares_coll.find_one(doc! {"_id": &token}, None).await? {
            Some(_) => return Err(mongodb::error::Error::custom(anyhow::anyhow!("Share token already exists"))),
            None => {}
        }

        shares_coll.insert_one(&partial_share, None).await?;
        Ok(partial_share)
    }

    pub async fn find_partial_share_by_token(&self, token: &str) -> MongoResult<Option<crate::models::PartialShare>> {
        let shares_coll = self.partial_shares_collection();
        shares_coll.find_one(doc! { "_id": token }, None).await
    }

    pub async fn find_partial_shares_by_user_id(&self, user_id: &str) -> MongoResult<Vec<crate::models::PartialShare>> {
        let shares_coll = self.partial_shares_collection();
        let mut cursor = shares_coll.find(doc! { "userId": user_id }, None).await?;
        let mut shares = Vec::new();
        while let Some(share) = cursor.try_next().await? {
            shares.push(share);
        }
        Ok(shares)
    }

    pub async fn delete_partial_share_by_token(&self, token: &str, user_id: &str) -> MongoResult<u64> {
        let shares_coll = self.partial_shares_collection();
        // Ensure user owns the share link before deleting
        let result = shares_coll.delete_one(doc! { "_id": token, "userId": user_id }, None).await?;
        Ok(result.deleted_count)
    }
}
