# Axum API Endpoint Documentation

This document outlines the available API endpoints for the chat application's Axum backend.

**Base URL for API:** `/api` (e.g., `http://localhost:3001/api` or deployed equivalent)
**OpenAI Compatible Base URL:** `/v1` (e.g., `http://localhost:3001/v1`)

**Authentication:** Most private endpoints require a JWT Bearer token in the `Authorization` header:
`Authorization: Bearer <your_jwt_token>`

---

## Health Check

### `GET /api/health`
- **Description:** Checks the health of the API and its database connection.
- **Auth:** None.
- **Responses:**
  - `200 OK`: Healthy. Body: `{ "status": "ok", "database": "connected" }`
  - `503 Service Unavailable`: Database connection issue. Body: `{ "status": "error", "database": "disconnected" }`

---

## Threads (`/api/threads`)

### `POST /api/threads`
- **Description:** Creates a new thread.
- **Auth:** Required.
- **Request Body:**
  ```json
  {
    "title": "Optional thread title", // Optional
    "visibility": "private" // Optional, "private" or "public", defaults to "private"
  }
  ```
- **Response:** `201 CREATED` with the new thread object.
  ```json
  {
    "id": "thread_id_string",
    "userId": "user_id_string",
    "title": "Thread Title",
    "visibility": "private",
    "originThreadId": null,
    "createdAt": "iso_timestamp",
    "updatedAt": "iso_timestamp"
  }
  ```

### `GET /api/threads`
- **Description:** Gets all threads for the authenticated user.
- **Auth:** Required.
- **Response:** `200 OK` with an array of thread objects.

### `GET /api/threads/:thread_id`
- **Description:** Gets a specific thread by its ID.
- **Auth:** Required (user must own the thread or it must be public).
- **Response:** `200 OK` with the thread object. `403 Forbidden` or `404 Not Found`.

### `DELETE /api/threads/:thread_id`
- **Description:** Deletes a specific thread and its associated messages.
- **Auth:** Required (user must own the thread).
- **Response:** `204 No Content`. `403 Forbidden` or `404 Not Found`.

### `PUT /api/threads/:thread_id/visibility`
- **Description:** Toggles the visibility of a thread.
- **Auth:** Required (user must own the thread).
- **Request Body:**
  ```json
  {
    "visibility": "public" // or "private"
  }
  ```
- **Response:** `200 OK` with the updated thread object.

### `POST /api/threads/:original_thread_id/branch`
- **Description:** Creates a new thread by branching from an existing message in another thread.
- **Auth:** Required.
- **Request Body:**
  ```json
  {
    "anchor_message_id": "message_id_to_branch_from",
    "new_thread_id": "optional_suggested_id_for_new_thread" // Server generates if not provided
  }
  ```
- **Response:** `201 CREATED` with the new branched thread object.

### `POST /api/threads/:thread_id/generate-title`
- **Description:** Generates and updates the title for a thread based on a user query.
- **Auth:** Required (user must own the thread).
- **Request Body:**
  ```json
  {
    "user_query": "Content of the first user message or relevant query"
  }
  ```
- **Response:** `200 OK` with the updated thread object.

---

## Messages

### `/api/threads/:thread_id/messages`

#### `POST /api/threads/:thread_id/messages`
- **Description:** Creates a new message within a specific thread.
- **Auth:** Required (user must have access to the thread).
- **Request Body:**
  ```json
  {
    "role": "user", // or "assistant"
    "content": "Message content", // Optional
    "parts": {}, // JSON value for complex parts, matches original schema
    "model": "optional_model_name_if_assistant_msg",
    "status": "done", // Optional, e.g., "pending", "streaming", "done", "error", "stopped"
    "annotations": {} // Optional JSON value
  }
  ```
- **Response:** `201 CREATED` with the new message object.

#### `GET /api/threads/:thread_id/messages`
- **Description:** Lists all messages for a specific thread.
- **Auth:** Required (user must have access to the thread).
- **Response:** `200 OK` with an array of message objects, sorted by creation time.

### `/api/messages/:message_id`

#### `PUT /api/messages/:message_id`
- **Description:** Updates a specific message (e.g., its content or status).
- **Auth:** Required (user must own the thread the message belongs to).
- **Request Body:**
  ```json
  {
    "content": "Updated message content", // Optional
    "parts": {}, // Optional, new parts
    "status": "done", // Optional
    "error_message": "Optional error message if status is error"
  }
  ```
- **Response:** `200 OK` with the updated message object.

#### `DELETE /api/messages/:message_id`
- **Description:** Deletes a specific message.
- **Auth:** Required (user must own the thread).
- **Response:** `204 No Content`.

#### `POST /api/messages/:message_id/delete-trailing`
- **Description:** Deletes all messages in the same thread that were created *after* the specified message.
- **Auth:** Required (user must own the thread).
- **Response:** `200 OK` with `{ "deleted_count": number, "message": "..." }`.

#### `POST /api/messages/:message_id/delete-inclusive-trailing`
- **Description:** Deletes the specified message AND all messages in the same thread created after it.
- **Auth:** Required (user must own the thread).
- **Response:** `200 OK` with `{ "deleted_count": number, "message": "..." }`.

---

## Shares (`/api/shares`)

### `POST /api/shares`
- **Description:** Creates a new partial share link for a thread up to a specific message.
- **Auth:** Required.
- **Request Body:**
  ```json
  {
    "thread_id": "thread_id_string",
    "shared_up_to_message_id": "message_id_string",
    "token": "optional_client_suggested_token" // Server generates if not provided
  }
  ```
- **Response:** `201 CREATED` with the partial share object.
  ```json
  {
    "token": "share_token",
    "threadId": "thread_id_string",
    "userId": "user_id_string",
    "sharedUpToMessageId": "message_id_string",
    "createdAt": "iso_timestamp"
  }
  ```

### `GET /api/shares`
- **Description:** Gets all partial share links created by the authenticated user.
- **Auth:** Required.
- **Response:** `200 OK` with an array of partial share objects.

### `DELETE /api/shares/:token`
- **Description:** Deletes a specific partial share link.
- **Auth:** Required (user must own the share link).
- **Response:** `204 No Content`.

### `GET /api/shares/:token/data`
- **Description:** Publicly retrieves the shared thread data (thread details and messages up to the shared limit) for a given share token.
- **Auth:** None.
- **Response:** `200 OK` with `{ "thread": { ...thread_object... }, "messages": [ ...message_objects... ] }`.

---

## Attachments (`/api/attachments`)

### `POST /api/attachments/delete`
- **Description:** Deletes an attachment from Vercel Blob storage.
- **Auth:** Required.
- **Request Body:**
  ```json
  {
    "url": "url_of_the_blob_to_delete"
  }
  ```
- **Response:** `204 No Content`.

---

## Voice (`/api/voice`)

### `POST /api/voice/client-token`
- **Description:** Generates a temporary client token for OpenAI Realtime API (for voice transcription).
- **Auth:** Required.
- **Request Body:**
  ```json
  {
    "apiKey": "optional_user_provided_openai_api_key"
  }
  ```
- **Response:** `200 OK` with `{ "session_id": "...", "client_secret": "...", "expiry": timestamp, "model_name": "..." }`.
- **Rate Limiting:** Applied if server's OpenAI key is used.

### `POST /api/voice/tts`
- **Description:** Converts text to speech using OpenAI or Google Gemini TTS.
- **Auth:** Required.
- **Request Body:**
  ```json
  {
    "text": "Text to convert to speech",
    "voice": "alloy", // Optional, provider-specific voice
    "model": "gpt-4o-mini-tts", // Optional, e.g., "tts-1", "gemini-2.5-flash-preview-tts"
    "speed": 1.0, // Optional, 0.25 to 4.0
    "apiKey": "user_provided_api_key_for_provider", // Required for this endpoint
    "provider": "openai" // Optional, "openai" or "google", defaults to "openai"
  }
  ```
- **Response:** `200 OK` with `{ "audio": "base64_encoded_audio_string", "format": "mp3_or_wav", "voice": "...", "text_length": number }`.

---

## OpenAI Compatible Endpoints (`/v1`)

### `POST /v1/chat/completions`
- **Description:** OpenAI API compatible endpoint for chat completions. Routes to various LLM providers based on the `model` field. Saves conversation to internal DB if `X-Thread-ID` and/or `X-User-ID` headers are provided.
- **Auth:** Uses Bearer token in `Authorization` header (this is the LLM provider API key).
- **Custom Headers for DB Interaction:**
  - `X-Thread-ID` (Optional): To continue an existing conversation in the database.
  - `X-User-ID` (Optional): To associate a new thread with a user in the database if `X-Thread-ID` is not provided.
- **Request Body:** Standard OpenAI ChatCompletion request format.
  ```json
  {
    "model": "provider_prefix/model_name", // e.g., "openai/gpt-4o", "openrouter/anthropic/claude-3-opus"
    "messages": [
      { "role": "system", "content": "You are helpful." },
      { "role": "user", "content": "Hello!" }
    ],
    "stream": false, // Optional, true for streaming
    "temperature": 0.7, // Optional
    "max_tokens": 150 // Optional
  }
  ```
- **Response (Non-Streaming):** Standard OpenAI ChatCompletion response format.
- **Response (Streaming):** Standard OpenAI SSE format for chat completions.
- **Note on DB Saving (Streaming):** For streaming responses, only the user message is currently saved reliably before streaming begins. Full assistant message saving during streaming via this specific endpoint is complex and might be deferred or simplified. Non-streaming responses save both user and assistant messages.

---
