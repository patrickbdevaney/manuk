//! Groq [`InferenceBackend`] — an OpenAI-compatible chat-completions client.
//!
//! The agent loop depends only on the [`InferenceBackend`] trait; this is one
//! implementation. It reuses `manuk-net` (hyper + rustls) for the HTTPS POST, so
//! outbound LLM traffic goes through the same pure-Rust stack as page loads — no
//! separate HTTP client, no OpenSSL.
//!
//! Multimodal: image content is sent as OpenAI-style `image_url` data URIs.
//! Reasoning models (qwen/DeepSeek) wrap chain-of-thought in `<think>…</think>`;
//! callers strip that via [`crate::strip_think`].

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use bytes::Bytes;
use serde_json::{json, Value};

use crate::{Content, InferenceBackend, Message};

const GROQ_ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";

/// A Groq chat-completions backend.
pub struct GroqBackend {
    api_key: String,
    model: String,
    endpoint: String,
    max_tokens: u32,
    temperature: f32,
}

impl GroqBackend {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        GroqBackend {
            api_key: api_key.into(),
            model: model.into(),
            endpoint: GROQ_ENDPOINT.to_string(),
            max_tokens: 2048,
            temperature: 0.2,
        }
    }

    /// Backend from a single key, using `GROQ_MODEL` (or the default).
    pub fn from_key(api_key: impl Into<String>) -> Self {
        Self::new(api_key, crate::env::model())
    }

    pub fn with_max_tokens(mut self, n: u32) -> Self {
        self.max_tokens = n;
        self
    }

    /// Serialize our messages into OpenAI/Groq chat format.
    fn body_json(&self, messages: &[Message]) -> Value {
        let msgs: Vec<Value> = messages.iter().map(message_json).collect();
        json!({
            "model": self.model,
            "messages": msgs,
            "temperature": self.temperature,
            "max_tokens": self.max_tokens,
        })
    }
}

fn message_json(m: &Message) -> Value {
    // A single text part serializes as a plain string; anything else (or an image)
    // uses the content-array form.
    let single_text = matches!(m.content.as_slice(), [Content::Text(_)]);
    if single_text {
        if let Content::Text(t) = &m.content[0] {
            return json!({ "role": m.role.as_str(), "content": t });
        }
    }
    let parts: Vec<Value> = m
        .content
        .iter()
        .map(|c| match c {
            Content::Text(t) => json!({ "type": "text", "text": t }),
            Content::ImagePng(bytes) => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
                json!({
                    "type": "image_url",
                    "image_url": { "url": format!("data:image/png;base64,{b64}") }
                })
            }
        })
        .collect();
    json!({ "role": m.role.as_str(), "content": parts })
}

#[async_trait::async_trait]
impl InferenceBackend for GroqBackend {
    async fn complete(&self, messages: &[Message]) -> Result<String> {
        let body = serde_json::to_vec(&self.body_json(messages)).context("serializing request")?;
        let auth = format!("Bearer {}", self.api_key);

        let resp = manuk_net::request(
            "POST",
            &self.endpoint,
            &[
                ("Authorization", auth.as_str()),
                ("Content-Type", "application/json"),
            ],
            Bytes::from(body),
        )
        .await
        .context("POST to Groq")?;

        if resp.status != 200 {
            bail!("Groq HTTP {}: {}", resp.status, truncate(&resp.text(), 400));
        }

        let v: Value = serde_json::from_slice(&resp.body).context("parsing Groq response")?;
        v["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| {
                anyhow!(
                    "no choices[0].message.content in response: {}",
                    truncate(&resp.text(), 300)
                )
            })
    }

    fn name(&self) -> String {
        format!("groq:{}", self.model)
    }
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Role;

    #[test]
    fn single_text_message_is_a_string() {
        let m = Message::text(Role::User, "hi");
        let j = message_json(&m);
        assert_eq!(j["content"], json!("hi"));
        assert_eq!(j["role"], "user");
    }

    #[test]
    fn image_message_uses_content_array() {
        let m = Message {
            role: Role::User,
            content: vec![
                Content::Text("look".into()),
                Content::ImagePng(vec![1, 2, 3]),
            ],
        };
        let j = message_json(&m);
        assert!(j["content"].is_array());
        let url = j["content"][1]["image_url"]["url"].as_str().unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn body_has_model_and_messages() {
        let b = GroqBackend::new("k", "qwen/qwen3.6-27b");
        let body = b.body_json(&[Message::text(Role::User, "hi")]);
        assert_eq!(body["model"], "qwen/qwen3.6-27b");
        assert!(body["messages"].is_array());
    }
}
