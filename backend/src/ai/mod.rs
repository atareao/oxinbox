use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("API returned error: {status} {body}")]
    Api { status: u16, body: String },

    #[error("not configured: {0}")]
    NotConfigured(String),
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    async fn chat(&self, system: &str, messages: &[ChatMessage]) -> Result<String, AiError>;
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AiError>;
    async fn transcribe(&self, audio_data: &[u8], mime_type: &str) -> Result<String, AiError>;
}

pub fn create_provider() -> Result<Arc<dyn AiProvider>, AiError> {
    let provider = std::env::var("AI_PROVIDER").unwrap_or_else(|_| "openrouter".into());
    match provider.as_str() {
        "openai" | "openrouter" => {
            let base_url =
                std::env::var("AI_BASE_URL").unwrap_or_else(|_| "https://openrouter.ai/api/v1".into());
            let api_key = std::env::var("AI_API_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .or_else(|_| std::env::var("OPENROUTER_API_KEY"))
                .map_err(|_| {
                    AiError::NotConfigured(
                        "set AI_API_KEY, OPENAI_API_KEY, or OPENROUTER_API_KEY".into(),
                    )
                })?;
            let model = std::env::var("AI_MODEL").unwrap_or_else(|_| "deepseek/deepseek-v4-flash".into());
            let embedding_model = std::env::var("AI_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "baai/bge-m3".into());
            let whisper_model =
                std::env::var("AI_WHISPER_MODEL").unwrap_or_else(|_| "qwen/qwen3-asr-flash-2026-02-10".into());

            let client =
                OpenAiProvider::new(base_url, api_key, model, embedding_model, whisper_model);
            tracing::info!(provider = %provider, model = %client.model, "AI provider configured");
            Ok(Arc::new(client))
        }
        other => Err(AiError::NotConfigured(format!(
            "unknown AI provider: {other}"
        ))),
    }
}

pub struct OpenAiProvider {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    embedding_model: String,
    whisper_model: String,
}

impl OpenAiProvider {
    pub fn new(
        base_url: String,
        api_key: String,
        model: String,
        embedding_model: String,
        whisper_model: String,
    ) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_mins(2))
                .build()
                .expect("failed to create HTTP client"),
            base_url,
            api_key,
            model,
            embedding_model,
            whisper_model,
        }
    }
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    #[instrument(skip(self, system, messages), fields(model = %self.model))]
    async fn chat(&self, system: &str, messages: &[ChatMessage]) -> Result<String, AiError> {
        let mut api_messages: Vec<serde_json::Value> =
            vec![serde_json::json!({"role": "system", "content": system})];
        for msg in messages {
            api_messages.push(serde_json::json!({"role": msg.role, "content": msg.content}));
        }

        let body = serde_json::json!({
            "model": self.model,
            "messages": api_messages,
        });

        let resp = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            return Err(AiError::Api {
                status: status.as_u16(),
                body: text,
            });
        }

        let json: serde_json::Value = serde_json::from_str(&text)?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| AiError::Api {
                status: status.as_u16(),
                body: "unexpected response format".into(),
            })?;

        Ok(content.to_string())
    }

    #[instrument(skip(self), fields(model = %self.embedding_model))]
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AiError> {
        let body = serde_json::json!({
            "model": self.embedding_model,
            "input": text,
        });

        let resp = self
            .http
            .post(format!("{}/embeddings", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let resp_text = resp.text().await?;

        if !status.is_success() {
            return Err(AiError::Api {
                status: status.as_u16(),
                body: resp_text,
            });
        }

        let json: serde_json::Value = serde_json::from_str(&resp_text)?;
        #[expect(clippy::cast_possible_truncation)]
        let embedding: Vec<f32> = json["data"][0]["embedding"]
            .as_array()
            .ok_or_else(|| AiError::Api {
                status: status.as_u16(),
                body: "no embedding in response".into(),
            })?
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();

        Ok(embedding)
    }

    #[instrument(skip(self, audio_data), fields(model = %self.whisper_model))]
    async fn transcribe(&self, audio_data: &[u8], mime_type: &str) -> Result<String, AiError> {
        let ext = mime_to_ext(mime_type);
        let part = reqwest::multipart::Part::bytes(audio_data.to_vec())
            .file_name(format!("audio.{ext}"))
            .mime_str(mime_type)
            .map_err(|e| AiError::Api {
                status: 0,
                body: e.to_string(),
            })?;

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", self.whisper_model.clone());

        let resp = self
            .http
            .post(format!("{}/audio/transcriptions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            return Err(AiError::Api {
                status: status.as_u16(),
                body: text,
            });
        }

        let json: serde_json::Value = serde_json::from_str(&text)?;
        let transcription = json["text"].as_str().unwrap_or_default().to_string();
        Ok(transcription)
    }
}

fn mime_to_ext(mime: &str) -> &'static str {
    match mime {
        "audio/wav" | "audio/wave" => "wav",
        "audio/mp3" | "audio/mpeg" => "mp3",
        "audio/mp4" => "mp4",
        "audio/x-m4a" => "m4a",
        "audio/flac" => "flac",
        _ => "webm",
    }
}
