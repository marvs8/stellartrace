//! Claude-backed advisor implementation.
//!
//! Calls the Anthropic Messages API with a system prompt that repeatedly
//! and explicitly constrains the model to an advisory role, and expects a
//! structured JSON response matching `ExpectedModelOutput`. Any response
//! that fails to parse, times out, or the request itself fails is turned
//! into `AiAdvisorError` so the caller (`AdvisorService`) can fall back —
//! this file never silently invents a recommendation for a failed call.

use crate::context::AiContext;
use crate::{AiAdvisor, AiAdvisorError};
use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;
use stellartrace_common::{AiRecommendation, AI_ADVISORY_LABEL};

const SYSTEM_PROMPT: &str = r#"You are a fraud-triage assistant for the StellarTrace system. You analyze a single flagged Stellar transaction and produce a STRICTLY ADVISORY assessment for a human investigator.

You must NEVER approve, reject, freeze, reverse, block, or otherwise state that a transaction has been actioned. You have no ability to change any system state, and any text that reads as an instruction to do so will be stripped before a human sees it. Only a human investigator decides final status.

Respond with ONLY a single JSON object with exactly these fields:
{
  "summary": "concise summary of the transaction",
  "suspicious_signals": ["signal 1", "signal 2"],
  "relevant_history_context": "relevant context from the provided history",
  "risk_assessment": "your assessment of risk level and why",
  "recommended_next_steps": ["step 1", "step 2"],
  "confidence": 0.0,
  "explanation": "brief explanation of your reasoning and confidence"
}
"confidence" must be a number between 0 and 1. Do not include any text outside the JSON object."#;

#[derive(Debug, Deserialize)]
struct ExpectedModelOutput {
    summary: String,
    #[serde(default)]
    suspicious_signals: Vec<String>,
    #[serde(default)]
    relevant_history_context: String,
    #[serde(default)]
    risk_assessment: String,
    #[serde(default)]
    recommended_next_steps: Vec<String>,
    #[serde(default)]
    confidence: f64,
    #[serde(default)]
    explanation: String,
}

#[derive(Debug, serde::Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage>,
}

#[derive(Debug, serde::Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(default)]
    text: String,
}

pub struct ClaudeAdvisor {
    api_key: String,
    model: String,
    http: reqwest::Client,
    api_base: String,
}

impl ClaudeAdvisor {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "claude-sonnet-5".to_string(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .expect("failed to build http client"),
            api_base: "https://api.anthropic.com/v1/messages".to_string(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Reads `ANTHROPIC_API_KEY` from the environment. Returns
    /// `AiAdvisorError::NotConfigured` if unset, so `AdvisorService` can
    /// fall back cleanly rather than panicking at startup.
    pub fn from_env() -> Result<Self, AiAdvisorError> {
        let key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| AiAdvisorError::NotConfigured)?;
        Ok(Self::new(key))
    }

    fn build_user_prompt(context: &AiContext) -> String {
        // serde_json::to_string on a plain struct of already-validated
        // domain data; no free-form user text is interpolated directly
        // into the prompt as instructions, only as inert JSON fields.
        format!(
            "Analyze this flagged transaction and respond with the JSON object described in the system prompt.\n\nTransaction context:\n{}",
            serde_json::to_string_pretty(context).unwrap_or_default()
        )
    }
}

#[async_trait]
impl AiAdvisor for ClaudeAdvisor {
    async fn investigate(&self, context: &AiContext) -> Result<AiRecommendation, AiAdvisorError> {
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 1024,
            system: SYSTEM_PROMPT.to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: Self::build_user_prompt(context),
            }],
        };

        let response = self
            .http
            .post(&self.api_base)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| AiAdvisorError::RequestFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(AiAdvisorError::RequestFailed(format!(
                "anthropic api returned status {}",
                response.status()
            )));
        }

        let parsed: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| AiAdvisorError::RequestFailed(e.to_string()))?;

        let text = parsed
            .content
            .first()
            .map(|b| b.text.clone())
            .ok_or_else(|| AiAdvisorError::UnparseableResponse("empty content".into()))?;

        let model_output: ExpectedModelOutput = extract_json(&text)
            .ok_or_else(|| {
                AiAdvisorError::UnparseableResponse("no JSON object found in response".into())
            })
            .and_then(|json_str| {
                serde_json::from_str(&json_str)
                    .map_err(|e| AiAdvisorError::UnparseableResponse(e.to_string()))
            })?;

        Ok(AiRecommendation {
            alert_id: context.alert_id,
            label: AI_ADVISORY_LABEL.to_string(),
            summary: model_output.summary,
            suspicious_signals: model_output.suspicious_signals,
            relevant_history_context: model_output.relevant_history_context,
            risk_assessment: model_output.risk_assessment,
            recommended_next_steps: model_output.recommended_next_steps,
            confidence: model_output.confidence,
            explanation: model_output.explanation,
            generated_at: chrono::Utc::now(),
            model_available: true,
        })
    }
}

/// Best-effort extraction of the first top-level JSON object in a string,
/// in case the model wraps it in prose or code fences despite
/// instructions not to.
fn extract_json(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end > start {
        Some(text[start..=end].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_pulls_object_out_of_prose() {
        let text =
            "Sure, here you go:\n```json\n{\"summary\":\"ok\",\"confidence\":0.5}\n```\nDone.";
        let extracted = extract_json(text).unwrap();
        let parsed: ExpectedModelOutput = serde_json::from_str(&extracted).unwrap();
        assert_eq!(parsed.summary, "ok");
        assert_eq!(parsed.confidence, 0.5);
    }

    #[test]
    fn missing_api_key_yields_not_configured() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        assert!(matches!(
            ClaudeAdvisor::from_env(),
            Err(AiAdvisorError::NotConfigured)
        ));
    }
}
