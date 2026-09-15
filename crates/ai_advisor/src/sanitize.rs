//! Treats every AI response as untrusted input.
//!
//! Two independent layers of defense are applied before a recommendation
//! is stored or displayed:
//! 1. The `label` field is always overwritten to the canonical advisory
//!    label — the model's own claim about what it is doing is never
//!    trusted.
//! 2. Free-text fields are stripped of markup, capped in length, and
//!    scanned for language that reads as a direct action instruction
//!    (e.g. "approve this transaction", "freeze the account"). Such
//!    phrases are neutralized in place rather than removed silently,
//!    so an investigator can still see that the model said something
//!    inappropriate — it just cannot be mistaken for an executed action,
//!    since nothing in this codebase ever executes text from this field.

use stellartrace_common::{AiRecommendation, AI_ADVISORY_LABEL};

const MAX_FIELD_LEN: usize = 4000;
const MAX_LIST_ITEMS: usize = 20;

/// Phrases that look like an attempt to get a reader (human or another
/// automated system) to treat the AI's text as an executable decision.
/// This is a defense-in-depth heuristic, not the primary control — the
/// primary control is that `AiRecommendation` has no field capable of
/// changing alert state in the first place.
const ACTION_INSTRUCTION_MARKERS: &[&str] = &[
    "approve this transaction",
    "reject this transaction",
    "freeze the account",
    "freeze this account",
    "reverse this transaction",
    "block this transaction",
    "set status to",
    "mark as false positive",
    "ignore previous instructions",
    "ignore prior instructions",
    "dismiss and close",
    "automatically",
];

fn strip_markup(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for c in input.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn neutralize_action_language(input: &str) -> String {
    let lower = input.to_lowercase();
    let mut result = input.to_string();
    for marker in ACTION_INSTRUCTION_MARKERS {
        if lower.contains(marker) {
            // Case-insensitive replace by rebuilding via lowercase search.
            // Simple and sufficient for this bounded marker list.
            let mut rebuilt = String::new();
            let mut rest = result.as_str();
            let lower_rest_owned;
            loop {
                let lower_rest = rest.to_lowercase();
                if let Some(pos) = lower_rest.find(marker) {
                    rebuilt.push_str(&rest[..pos]);
                    rebuilt.push_str("[neutralized: non-actionable AI text removed]");
                    rest = &rest[pos + marker.len()..];
                } else {
                    rebuilt.push_str(rest);
                    break;
                }
            }
            lower_rest_owned = rebuilt;
            result = lower_rest_owned;
        }
    }
    result
}

fn clean_field(input: &str) -> String {
    let stripped = strip_markup(input);
    let neutralized = neutralize_action_language(&stripped);
    neutralized.chars().take(MAX_FIELD_LEN).collect()
}

/// Sanitizes an AI recommendation in place. Called on every response
/// before it is stored or returned from the API, regardless of source.
pub fn sanitize_recommendation(rec: &mut AiRecommendation) {
    rec.label = AI_ADVISORY_LABEL.to_string();
    rec.summary = clean_field(&rec.summary);
    rec.relevant_history_context = clean_field(&rec.relevant_history_context);
    rec.risk_assessment = clean_field(&rec.risk_assessment);
    rec.explanation = clean_field(&rec.explanation);

    rec.suspicious_signals = rec
        .suspicious_signals
        .iter()
        .take(MAX_LIST_ITEMS)
        .map(|s| clean_field(s))
        .collect();
    rec.recommended_next_steps = rec
        .recommended_next_steps
        .iter()
        .take(MAX_LIST_ITEMS)
        .map(|s| clean_field(s))
        .collect();

    if !rec.confidence.is_finite() {
        rec.confidence = 0.0;
    }
    rec.confidence = rec.confidence.clamp(0.0, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn rec_with_summary(summary: &str) -> AiRecommendation {
        AiRecommendation {
            alert_id: Uuid::new_v4(),
            label: "whatever the model said".into(),
            summary: summary.into(),
            suspicious_signals: vec![],
            relevant_history_context: "".into(),
            risk_assessment: "".into(),
            recommended_next_steps: vec![],
            confidence: 2.5,
            explanation: "".into(),
            generated_at: Utc::now(),
            model_available: true,
        }
    }

    #[test]
    fn strips_script_tags() {
        let mut rec = rec_with_summary("hello <script>alert(1)</script> world");
        sanitize_recommendation(&mut rec);
        assert_eq!(rec.summary, "hello alert(1) world");
    }

    #[test]
    fn neutralizes_action_instructions() {
        let mut rec = rec_with_summary("You should APPROVE THIS TRANSACTION right now.");
        sanitize_recommendation(&mut rec);
        assert!(!rec.summary.to_lowercase().contains("approve this transaction"));
        assert!(rec.summary.contains("neutralized"));
    }

    #[test]
    fn forces_canonical_label() {
        let mut rec = rec_with_summary("fine");
        sanitize_recommendation(&mut rec);
        assert_eq!(rec.label, AI_ADVISORY_LABEL);
    }

    #[test]
    fn clamps_confidence() {
        let mut rec = rec_with_summary("fine");
        rec.confidence = 99.0;
        sanitize_recommendation(&mut rec);
        assert_eq!(rec.confidence, 1.0);

        rec.confidence = f64::NAN;
        sanitize_recommendation(&mut rec);
        assert_eq!(rec.confidence, 0.0);
    }

    #[test]
    fn truncates_oversized_fields() {
        let mut rec = rec_with_summary(&"a".repeat(10_000));
        sanitize_recommendation(&mut rec);
        assert_eq!(rec.summary.len(), MAX_FIELD_LEN);
    }
}
