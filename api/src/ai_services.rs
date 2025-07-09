use reqwest::{Client, RequestBuilder, Body};
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{error, info, warn};
use anyhow::{anyhow, Result, Context};
use futures_util::Stream; // For async streams
use bytes::Bytes;


// --- Common AI Service Structs ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String, // "system", "user", "assistant", "tool"
    pub content: Option<String>,
    // Add tool_calls, tool_call_id if implementing tool use
}

#[derive(Debug, Clone)]
pub struct ChatCompletionRequest {
    pub model: String, // e.g., "openai/gpt-4o", "openrouter/anthropic/claude-3-opus-20240229", "google/gemini-1.5-pro"
    pub messages: Vec<ChatMessage>,
    pub api_key: Option<String>, // User-provided or system key
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: bool, // If true, expect a stream of ChatCompletionChunk
    // Add other common parameters like top_p, presence_penalty, etc.
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatCompletionChunk {
    pub id: String, // Chunk ID or completion ID
    pub model: String, // Model that generated the chunk
    pub created: u64, // Timestamp
    pub choices: Vec<ChatCompletionChunkChoice>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatCompletionChunkChoice {
    pub delta: ChatMessage, // The change in content
    pub index: u32,
    pub finish_reason: Option<String>, // e.g., "stop", "length", "tool_calls"
}

// For non-streaming responses (though we'll primarily focus on streaming)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String, // e.g., "chat.completion"
    pub created: u64,
    pub model: String,
    pub choices: Vec<ResponseMessageChoice>,
    // pub usage: Option<UsageStats>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResponseMessageChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}


// --- Provider Specific Payloads (examples, will grow) ---

// OpenAI ChatCompletion request structure (subset)
#[derive(Serialize)]
struct OpenAIChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    // n: Option<u32>, // Number of completions to generate
    // stop: Option<Vec<String>>,
    // presence_penalty: Option<f32>,
    // frequency_penalty: Option<f32>,
}


// --- AI Service Logic ---

fn determine_provider_and_model(model_id: &str) -> (String, String) {
    if let Some((provider, model_name)) = model_id.split_once('/') {
        (provider.to_lowercase(), model_name.to_string())
    } else {
        // Default to OpenAI if no prefix, or handle as error
        warn!("Model ID '{}' does not specify a provider, defaulting to OpenAI or treating as direct model name.", model_id);
        ("openai".to_string(), model_id.to_string()) // Or, could be ("unknown", model_id.to_string())
    }
}

async fn make_http_request(
    client: &Client,
    method: reqwest::Method,
    url: &str,
    api_key: &str,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    body: Option<impl Serialize>, // Make body generic over Serialize
    is_openrouter: bool,
) -> Result<RequestBuilder> {
    let mut request_builder = client.request(method, url).bearer_auth(api_key);

    if is_openrouter {
        // OpenRouter specific headers (example)
        request_builder = request_builder
            .header("HTTP-Referer", env::var("NEXT_PUBLIC_APP_URL").unwrap_or_else(|_| "http://localhost:3000".to_string())) // Replace with your app URL
            .header("X-Title", env::var("NEXT_PUBLIC_APP_NAME").unwrap_or_else(|_| "My Axum App".to_string())); // Replace with your app name
    }

    if let Some(b) = body {
        request_builder = request_builder.json(&b);
    }

    Ok(request_builder)
}


pub async fn generate_chat_completion_stream(
    request: ChatCompletionRequest,
    http_client: &Client,
) -> Result<impl Stream<Item = Result<ChatCompletionChunk, anyhow::Error>>> {
    let (provider_name, model_name) = determine_provider_and_model(&request.model);

    let api_key_to_use = match request.api_key.as_ref() {
        Some(key) => key.clone(),
        None => { // Fallback to provider-specific server keys if defined, or error
            match provider_name.as_str() {
                "openai" => env::var("OPENAI_API_KEY").map_err(|_| anyhow!("OpenAI API key not configured (server or user)"))?,
                "openrouter" => env::var("OPENROUTER_API_KEY").map_err(|_| anyhow!("OpenRouter API key not configured (server or user)"))?,
                // Add other providers like Anthropic, Google here
                _ => return Err(anyhow!("Unsupported provider '{}' or missing API key.", provider_name)),
            }
        }
    };

    match provider_name.as_str() {
        "openai" => stream_openai_completion(&model_name, request, http_client, &api_key_to_use).await,
        "openrouter" => stream_openrouter_completion(&model_name, request, http_client, &api_key_to_use).await,
        // "anthropic" => stream_anthropic_completion(...).await,
        // "google" => stream_google_completion(...).await,
        _ => Err(anyhow!("Unsupported AI provider: {}", provider_name)),
    }
}

// --- OpenAI Specific Streaming Logic ---
async fn stream_openai_completion(
    model_name: &str,
    common_request: ChatCompletionRequest,
    client: &Client,
    api_key: &str,
) -> Result<impl Stream<Item = Result<ChatCompletionChunk>>> {
    let openai_request = OpenAIChatRequest {
        model: model_name,
        messages: &common_request.messages,
        stream: true,
        temperature: common_request.temperature,
        max_tokens: common_request.max_tokens,
    };

    let request_builder = make_http_request(
        client,
        reqwest::Method::POST,
        "https://api.openai.com/v1/chat/completions",
        api_key,
        Some(&openai_request),
        false, // Not OpenRouter
    ).await?;

    info!("Streaming from OpenAI model: {}", model_name);
    let response = request_builder.send().await.context("Failed to send request to OpenAI")?;

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(anyhow!("OpenAI API request failed: {} - {}", response.status(), error_body));
    }

    // Process the SSE stream from OpenAI
    Ok(response.bytes_stream()
        .map_err(|e| anyhow!("Error reading OpenAI stream: {}", e))
        .flat_map(|bytes_result| { // Use flat_map to handle potential multiple SSE events in one Bytes chunk
            let bytes = match bytes_result {
                Ok(b) => b,
                Err(e) => return futures_util::stream::iter(vec![Err(e)]),
            };

            let content = String::from_utf8_lossy(&bytes).to_string();
            let mut chunks = Vec::new();

            for line in content.lines() {
                if line.starts_with("data: ") {
                    let json_str = &line["data: ".len()..];
                    if json_str.trim() == "[DONE]" {
                        // End of stream
                    } else {
                        match serde_json::from_str::<OpenAICompletionChunk>(json_str) {
                            Ok(parsed_chunk) => {
                                // Transform OpenAI chunk to common ChatCompletionChunk
                                chunks.push(Ok(parsed_chunk.into_common_chunk()));
                            }
                            Err(e) => {
                                warn!("Failed to parse OpenAI SSE chunk: {}. JSON: '{}'", e, json_str);
                                // Optionally push an error or skip
                            }
                        }
                    }
                }
            }
            futures_util::stream::iter(chunks)
        })
    )
}

// --- OpenRouter Specific Streaming Logic ---
async fn stream_openrouter_completion(
    model_name: &str, // This model_name is the part after "openrouter/", e.g., "anthropic/claude-3-opus"
    common_request: ChatCompletionRequest,
    client: &Client,
    api_key: &str,
) -> Result<impl Stream<Item = Result<ChatCompletionChunk>>> {
    // OpenRouter uses OpenAI compatible API structure for many models
    // but the model name passed in the request should be the OpenRouter specific one.
    let openrouter_request = OpenAIChatRequest { // Using OpenAI's request struct
        model: model_name, // Pass the specific model name for OpenRouter
        messages: &common_request.messages,
        stream: true,
        temperature: common_request.temperature,
        max_tokens: common_request.max_tokens,
    };

    let request_builder = make_http_request(
        client,
        reqwest::Method::POST,
        "https://openrouter.ai/api/v1/chat/completions",
        api_key,
        Some(&openrouter_request),
        true, // Is OpenRouter
    ).await?;

    info!("Streaming from OpenRouter model: {}", model_name);
    let response = request_builder.send().await.context("Failed to send request to OpenRouter")?;

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(anyhow!("OpenRouter API request failed: {} - {}", response.status(), error_body));
    }

    // OpenRouter often returns OpenAI-compatible SSE stream
    Ok(response.bytes_stream()
        .map_err(|e| anyhow!("Error reading OpenRouter stream: {}", e))
        .flat_map(|bytes_result| {
            let bytes = match bytes_result {
                Ok(b) => b,
                Err(e) => return futures_util::stream::iter(vec![Err(e)]),
            };
            let content = String::from_utf8_lossy(&bytes).to_string();
            let mut chunks = Vec::new();
            for line in content.lines() {
                if line.starts_with("data: ") {
                    let json_str = &line["data: ".len()..];
                    if json_str.trim() == "[DONE]" {
                        // End of stream
                    } else {
                         match serde_json::from_str::<OpenAICompletionChunk>(json_str) { // Assuming OpenRouter uses OpenAI's chunk format
                            Ok(parsed_chunk) => {
                                chunks.push(Ok(parsed_chunk.into_common_chunk()));
                            }
                            Err(e) => {
                                warn!("Failed to parse OpenRouter SSE chunk: {}. JSON: '{}'", e, json_str);
                            }
                        }
                    }
                }
            }
            futures_util::stream::iter(chunks)
        })
    )
}


// --- Helper Structs for parsing provider-specific chunks (e.g. OpenAI) ---
#[derive(Deserialize)]
struct OpenAICompletionChunk {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<OpenAIChunkChoice>,
}

#[derive(Deserialize)]
struct OpenAIChunkChoice {
    delta: OpenAIDelta,
    index: u32,
    finish_reason: Option<String>,
}

#[derive(Deserialize, Clone)]
struct OpenAIDelta {
    role: Option<String>, // Role usually comes in the first chunk for some models
    content: Option<String>,
    // tool_calls: Option<Vec<ToolCall>>,
}

impl OpenAICompletionChunk {
    // Helper to convert OpenAI specific chunk to the common ChatCompletionChunk
    fn into_common_chunk(self) -> ChatCompletionChunk {
        ChatCompletionChunk {
            id: self.id,
            model: self.model,
            created: self.created,
            choices: self.choices.into_iter().map(|c| {
                ChatCompletionChunkChoice {
                    delta: ChatMessage { // Map OpenAIDelta to ChatMessage
                        role: c.delta.role.unwrap_or_else(|| "assistant".to_string()), // Default role if not present
                        content: c.delta.content,
                        // tool_calls, etc.
                    },
                    index: c.index,
                    finish_reason: c.finish_reason,
                }
            }).collect(),
        }
    }
}


// Title generation can be a simplified version of chat completion
// Or a call to a specific "completion" endpoint if models support it better than chat for titles.
// For now, the existing title generation is separate. If it needs to use this
// new infrastructure, it would construct a ChatCompletionRequest.
// The old `generate_title_for_prompt` is kept for now if it's still used by existing title gen endpoint.

// --- Original Title Generation (kept for reference or separate use) ---
#[derive(Serialize)]
struct OldOpenAIChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>, // Uses common ChatMessage
    max_tokens: u32,
    temperature: f32,
}
#[derive(Deserialize)]
struct OldOpenAIChatCompletionResponse {
    choices: Vec<OldChoice>,
}
#[derive(Deserialize)]
struct OldChoice {
    message: ChatMessage, // Uses common ChatMessage
}

const OLD_OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
const DEFAULT_TITLE_GENERATION_MODEL: &str = "gpt-3.5-turbo";

pub async fn generate_title_for_prompt(prompt_content: &str) -> Result<String> {
    let openai_api_key = env::var("OPENAI_API_KEY")
        .map_err(|_| anyhow!("OPENAI_API_KEY environment variable not set"))?;

    let client = Client::new();
    let request_payload = OldOpenAIChatCompletionRequest {
        model: DEFAULT_TITLE_GENERATION_MODEL.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: Some("You are a helpful assistant. Your task is to generate a concise and relevant title (5 words or less) for the following user query or conversation start. Only output the title itself, nothing else.".to_string()),
            },
            ChatMessage {
                role: "user".to_string(),
                content: Some(prompt_content.to_string()),
            },
        ],
        max_tokens: 20,
        temperature: 0.5,
    };

    info!("Sending title generation request to OpenAI for prompt: {:.50}...", prompt_content);
    let response = client
        .post(OLD_OPENAI_API_URL)
        .bearer_auth(openai_api_key)
        .json(&request_payload)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to send request to OpenAI: {}", e))?;

    if response.status().is_success() {
        let response_body = response.json::<OldOpenAIChatCompletionResponse>().await
            .map_err(|e| anyhow!("Failed to parse OpenAI response: {}", e))?;
        if let Some(choice) = response_body.choices.get(0) {
            let title = choice.message.content.as_deref().unwrap_or("").trim().to_string();
            let cleaned_title = title.trim_matches(|c: char| c == '"' || c == '\'').to_string();
            info!("Generated title: '{}'", cleaned_title);
            Ok(cleaned_title)
        } else {
            Err(anyhow!("OpenAI response did not contain any choices for title generation"))
        }
    } else {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_else(|_| "Unknown error body".to_string());
        error!("OpenAI API request failed with status {}: {}", status, error_body);
        Err(anyhow!("OpenAI API request failed (status: {}): {}", status, error_body))
    }
}
