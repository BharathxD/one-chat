use axum::{
    extract::State, // Will use Extension
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::post,
    Extension, Json, Router,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{error, info};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};


use crate::{
    auth::{auth_middleware, AuthenticatedUser},
    redis_utils::RateLimiter, // For rate limiting
};
// No DBManager needed directly by these handlers, unless storing voice session info etc.

// --- Payloads and Responses ---

#[derive(Deserialize)]
struct GenerateClientTokenPayload {
    #[serde(rename = "apiKey")]
    api_key: Option<String>,
}

#[derive(Serialize)]
struct GenerateClientTokenResponse {
    session_id: String,
    client_secret: String,
    expiry: i64, // Assuming timestamp
    model_name: String,
}

#[derive(Deserialize)]
struct TextToSpeechPayload {
    text: String,
    voice: Option<String>, // Defaulted in original code
    model: Option<String>, // Defaulted in original code
    speed: Option<f32>,   // Defaulted in original code
    #[serde(rename = "apiKey")]
    api_key: Option<String>,
    provider: Option<String>, // Defaulted in original code ("openai")
}

#[derive(Serialize)]
struct TextToSpeechResponse {
    audio: String, // base64 encoded audio
    format: String,
    voice: String,
    text_length: usize,
}


// --- OpenAI Specific Structs (mirroring original code) ---
#[derive(Serialize)]
struct OpenAIRealtimeSessionRequest {
    model: String,
    input_audio_format: String,
    input_audio_transcription: TranscriptionConfig,
    turn_detection: TurnDetectionConfig,
}

#[derive(Serialize)]
struct TranscriptionConfig {
    model: String,
    language: String,
}

#[derive(Serialize)]
struct TurnDetectionConfig {
    #[serde(rename = "type")]
    detection_type: String,
    threshold: f32,
    prefix_padding_ms: u32,
    silence_duration_ms: u32,
}

#[derive(Deserialize)]
struct OpenAIRealtimeSessionResponse {
    id: String,
    client_secret: ClientSecret,
    model: String,
    // other fields if needed
}
#[derive(Deserialize)]
struct ClientSecret {
    value: String,
    expires_at: i64, // Assuming timestamp
}

#[derive(Serialize)]
struct OpenAITtsRequest {
    model: String,
    input: String,
    voice: String,
    response_format: String,
    speed: f32,
}

// --- Google Gemini Specific Structs ---
#[derive(Serialize)]
struct GeminiTtsRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "generationConfig")]
    generation_config: GeminiGenerationConfig,
}
#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}
#[derive(Serialize)]
struct GeminiPart {
    text: String,
}
#[derive(Serialize)]
struct GeminiGenerationConfig {
    #[serde(rename = "responseModalities")]
    response_modalities: Vec<String>,
    #[serde(rename = "speechConfig")]
    speech_config: GeminiSpeechConfig,
}
#[derive(Serialize)]
struct GeminiSpeechConfig {
    #[serde(rename = "voiceConfig")]
    voice_config: GeminiVoiceConfig,
}
#[derive(Serialize)]
struct GeminiVoiceConfig {
    #[serde(rename = "prebuiltVoiceConfig")]
    prebuilt_voice_config: GeminiPrebuiltVoiceConfig,
}
#[derive(Serialize)]
struct GeminiPrebuiltVoiceConfig {
    #[serde(rename = "voiceName")]
    voice_name: String,
}

#[derive(Deserialize)]
struct GeminiTtsResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}
#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiContentResponse>,
}
#[derive(Deserialize)]
struct GeminiContentResponse {
    parts: Option<Vec<GeminiPartResponse>>,
}
#[derive(Deserialize)]
struct GeminiPartResponse {
    #[serde(rename = "inlineData")]
    inline_data: Option<GeminiInlineData>,
}
#[derive(Deserialize)]
struct GeminiInlineData {
    data: String, // base64 encoded PCM16
}


// --- WAV file creation utility (ported from original JS) ---
fn create_wav_file(pcm_data: &[u8], sample_rate: u32, channels: u16) -> Vec<u8> {
    let num_samples = pcm_data.len() / (channels as usize * 2); // 2 bytes per sample
    let byte_rate = sample_rate * channels as u32 * 2; // 16-bit samples
    let block_align = channels * 2;
    let data_size = pcm_data.len() as u32;
    let file_size = 36 + data_size; // RIFF chunk descriptor (8) + WAVE ID (4) + fmt chunk (24) + data chunk header (8) + data_size

    let mut header = Vec::with_capacity(44);

    // RIFF header
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&file_size.to_le_bytes());
    header.extend_from_slice(b"WAVE");

    // fmt chunk
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size (PCM)
    header.extend_from_slice(&1u16.to_le_bytes());  // PCM format
    header.extend_from_slice(&channels.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&byte_rate.to_le_bytes());
    header.extend_from_slice(&block_align.to_le_bytes());
    header.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    // data chunk
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_size.to_le_bytes());

    let mut wav_file = header;
    wav_file.extend_from_slice(pcm_data);
    wav_file
}


// --- Router ---
pub fn voice_router() -> Router {
    Router::new()
        .route("/client-token", post(generate_client_token_handler))
        .route("/tts", post(text_to_speech_handler))
        .route_layer(middleware::from_fn(auth_middleware))
}

// --- Handlers ---
async fn generate_client_token_handler(
    Extension(rate_limiter): Extension<RateLimiter>,
    user: AuthenticatedUser,
    Json(payload): Json<GenerateClientTokenPayload>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} requesting voice client token.", user.id);

    let server_openai_api_key = env::var("OPENAI_API_KEY").ok();
    let effective_api_key = payload.api_key.as_ref().or(server_openai_api_key.as_ref());

    if effective_api_key.is_none() {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "OpenAI API key not configured.".to_string()));
    }
    let api_key_to_use = effective_api_key.unwrap();

    // Rate limit if user is using server's API key
    if payload.api_key.is_none() { // User is relying on server key
        let rate_limit_key = format!("voice_token:{}", user.id);
        match rate_limiter.limit(&rate_limit_key).await {
            Ok(rl_response) => {
                if !rl_response.success {
                    let wait_minutes = (rl_response.reset - (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64)) / 60000 + 1;
                    return Err((StatusCode::TOO_MANY_REQUESTS, format!("Voice limit reached ({}/hour). Try again in {}m or add your API key.", rl_response.limit, wait_minutes)));
                }
            }
            Err(e) => {
                error!("Rate limiting error for user {}: {}", user.id, e);
                return Err((StatusCode::INTERNAL_SERVER_ERROR, "Rate limiting error.".to_string()));
            }
        }
    }

    let realtime_config = OpenAIRealtimeSessionRequest {
        model: "gpt-4o-mini-realtime-preview".to_string(),
        input_audio_format: "pcm16".to_string(),
        input_audio_transcription: TranscriptionConfig {
            model: "whisper-1".to_string(),
            language: "en".to_string(),
        },
        turn_detection: TurnDetectionConfig {
            detection_type: "server_vad".to_string(),
            threshold: 0.7,
            prefix_padding_ms: 300,
            silence_duration_ms: 200,
        },
    };

    let client = Client::new();
    match client.post("https://api.openai.com/v1/realtime/sessions")
        .bearer_auth(api_key_to_use)
        .json(&realtime_config)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<OpenAIRealtimeSessionResponse>().await {
                    Ok(data) => Ok(Json(GenerateClientTokenResponse {
                        session_id: data.id,
                        client_secret: data.client_secret.value,
                        expiry: data.client_secret.expires_at,
                        model_name: data.model,
                    })),
                    Err(e) => {
                        error!("Failed to parse OpenAI session token response: {}", e);
                        Err((StatusCode::INTERNAL_SERVER_ERROR, "Invalid response from OpenAI API.".to_string()))
                    }
                }
            } else {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown OpenAI error".to_string());
                error!("OpenAI API error (session token): {} - {}", status, error_text);
                Err((status, format!("Failed to create transcription session: {}", error_text)))
            }
        }
        Err(e) => {
            error!("Failed to send request for OpenAI session token: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate transcription token.".to_string()))
        }
    }
}

async fn text_to_speech_handler(
    // No rate limiting here as per original, relies on user API key
    user: AuthenticatedUser, // For logging, and if server key fallback was allowed with rate limit
    Json(payload): Json<TextToSpeechPayload>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("User {} requesting TTS for text: {:.30}...", user.id, payload.text);

    let provider = payload.provider.unwrap_or_else(|| "openai".to_string());
    let api_key = match payload.api_key {
        Some(key) => key,
        None => {
            // If allowing server key fallback for TTS, add env var check & rate limit here
            // For now, strictly require user-provided API key for TTS as per original logic for this part
            return Err((StatusCode::BAD_REQUEST, format!("API key for {} is required.", provider)));
        }
    };

    let client = Client::new();

    if provider == "openai" {
        let model = payload.model.unwrap_or_else(|| "gpt-4o-mini-tts".to_string());
        let voice = payload.voice.unwrap_or_else(|| "alloy".to_string());
        let speed = payload.speed.unwrap_or(1.0);
        let tts_request = OpenAITtsRequest {
            model,
            input: payload.text.clone(),
            voice,
            response_format: "mp3".to_string(),
            speed,
        };

        match client.post("https://api.openai.com/v1/audio/speech")
            .bearer_auth(api_key)
            .json(&tts_request)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    match response.bytes().await {
                        Ok(audio_bytes) => {
                            let audio_base64 = BASE64_STANDARD.encode(audio_bytes);
                            Ok(Json(TextToSpeechResponse {
                                audio: audio_base64,
                                format: "mp3".to_string(),
                                voice: tts_request.voice,
                                text_length: payload.text.len(),
                            }))
                        }
                        Err(e) => {
                            error!("Failed to read OpenAI TTS audio bytes: {}", e);
                            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to process TTS audio.".to_string()))
                        }
                    }
                } else {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_else(|_| "Unknown OpenAI TTS error".to_string());
                    error!("OpenAI TTS API error: {} - {}", status, error_text);
                    Err((status, format!("Failed to generate speech (OpenAI): {}", error_text)))
                }
            }
            Err(e) => {
                error!("Failed to send request to OpenAI TTS: {}", e);
                Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate speech.".to_string()))
            }
        }

    } else if provider == "google" {
        let model = payload.model.unwrap_or_else(|| "gemini-2.5-flash-preview-tts".to_string()); // Example default
        let voice = payload.voice.unwrap_or_else(|| "elevenlabs-alloy".to_string()); // Example default, adjust based on Gemini voice names

        let gemini_request = GeminiTtsRequest {
            contents: vec![GeminiContent { parts: vec![GeminiPart { text: payload.text.clone() }] }],
            generation_config: GeminiGenerationConfig {
                response_modalities: vec!["AUDIO".to_string()],
                speech_config: GeminiSpeechConfig {
                    voice_config: GeminiVoiceConfig {
                        prebuilt_voice_config: GeminiPrebuiltVoiceConfig { voice_name: voice.clone() },
                    },
                },
            },
        };

        let gemini_api_url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, api_key);

        match client.post(&gemini_api_url)
            .json(&gemini_request)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<GeminiTtsResponse>().await {
                        Ok(data) => {
                            if let Some(audio_data_base64) = data.candidates.and_then(|c| c.into_iter().next()).and_then(|c| c.content).and_then(|co| co.parts).and_then(|p| p.into_iter().next()).and_then(|pa| pa.inline_data).map(|d| d.data) {
                                match BASE64_STANDARD.decode(audio_data_base64) {
                                    Ok(pcm_buffer) => {
                                        let wav_buffer = create_wav_file(&pcm_buffer, 24000, 1); // 24kHz, mono as per original
                                        let wav_base64 = BASE64_STANDARD.encode(wav_buffer);
                                        Ok(Json(TextToSpeechResponse {
                                            audio: wav_base64,
                                            format: "wav".to_string(),
                                            voice,
                                            text_length: payload.text.len(),
                                        }))
                                    }
                                    Err(e) => {
                                        error!("Failed to decode Gemini base64 audio: {}", e);
                                        Err((StatusCode::INTERNAL_SERVER_ERROR, "Invalid audio data from Gemini.".to_string()))
                                    }
                                }
                            } else {
                                Err((StatusCode::INTERNAL_SERVER_ERROR, "Invalid response structure from Gemini API.".to_string()))
                            }
                        }
                        Err(e) => {
                             error!("Failed to parse Gemini TTS response: {}", e);
                             Err((StatusCode::INTERNAL_SERVER_ERROR, "Invalid response from Gemini API.".to_string()))
                        }
                    }
                } else {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_else(|_| "Unknown Gemini TTS error".to_string());
                    error!("Gemini TTS API error: {} - {}", status, error_text);
                    Err((status, format!("Failed to generate speech (Google): {}", error_text)))
                }
            }
            Err(e) => {
                error!("Failed to send request to Gemini TTS: {}", e);
                Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate speech.".to_string()))
            }
        }
    } else {
        Err((StatusCode::BAD_REQUEST, "Unsupported TTS provider.".to_string()))
    }
}
