//! Groq — a **preset** of [`OpenAiCompatBackend`], not its own type (INFERENCE.MD §1).
//!
//! Groq speaks OpenAI's `/v1/chat/completions`, so it needs no bespoke client: it is the
//! generic backend with a fixed endpoint, the `GROQ_MODEL` default, and a required key.
//! Keeping it as a preset rather than a parallel implementation is the whole point — every
//! fix to the generic path (model resolution, retries, error reporting) reaches Groq for
//! free, and there is one request/response shape to keep correct instead of two.
//!
//! The CLI surface is unchanged: `agent-run` still reads `GROQ_API_KEY` / `.env`.
//!
//! Multimodal image content is sent as OpenAI-style `image_url` data URIs. Reasoning models
//! (qwen/DeepSeek) wrap chain-of-thought in `<think>…</think>`; callers strip that via
//! [`crate::strip_think`].

pub use crate::local::OpenAiCompatBackend;

/// The Groq preset: fixed endpoint + `GROQ_MODEL` default + the given key.
pub fn groq(api_key: impl Into<String>) -> OpenAiCompatBackend {
    OpenAiCompatBackend::groq(api_key)
}

/// The Groq preset with an explicit model, overriding `GROQ_MODEL`.
pub fn groq_with_model(api_key: impl Into<String>, model: impl Into<String>) -> OpenAiCompatBackend {
    OpenAiCompatBackend::groq(api_key).with_model(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InferenceBackend;

    #[test]
    fn the_groq_preset_is_the_generic_backend_with_a_fixed_endpoint() {
        let b = groq_with_model("k", "qwen/qwen3.6-27b");
        assert_eq!(b.endpoint(), "https://api.groq.com/openai/v1/chat/completions");
        assert_eq!(b.name(), "groq:qwen/qwen3.6-27b");
        assert_eq!(b.model().as_deref(), Some("qwen/qwen3.6-27b"));
    }
}
