//! Minimal in-process metrics: plain atomic counters exposed as
//! Prometheus text exposition format at `GET /metrics`. No external
//! metrics crate dependency — kept simple for the reference
//! implementation. See `docs/observability.md`.

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct Metrics {
    pub transactions_ingested_total: AtomicU64,
    pub alerts_created_total: AtomicU64,
    pub ai_recommendations_total: AtomicU64,
    pub ai_fallback_total: AtomicU64,
    pub investigator_decisions_total: AtomicU64,
}

impl Metrics {
    pub fn record_transaction_ingested(&self) {
        self.transactions_ingested_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_alert_created(&self) {
        self.alerts_created_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_ai_recommendation(&self, model_available: bool) {
        self.ai_recommendations_total
            .fetch_add(1, Ordering::Relaxed);
        if !model_available {
            self.ai_fallback_total.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_investigator_decision(&self) {
        self.investigator_decisions_total
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Renders current counter values as Prometheus text exposition
    /// format (version 0.0.4), suitable for a scrape target.
    pub fn render_prometheus_text(&self) -> String {
        let mut out = String::new();
        push_counter(
            &mut out,
            "stellartrace_transactions_ingested_total",
            "Transactions received via the ingest/evaluate endpoints",
            self.transactions_ingested_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "stellartrace_alerts_created_total",
            "Alerts created by the rules engine",
            self.alerts_created_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "stellartrace_ai_recommendations_total",
            "AI advisory recommendations generated",
            self.ai_recommendations_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "stellartrace_ai_fallback_total",
            "AI advisory recommendations that were fallback responses (AI unavailable)",
            self.ai_fallback_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "stellartrace_investigator_decisions_total",
            "Final human decisions submitted",
            self.investigator_decisions_total.load(Ordering::Relaxed),
        );
        out
    }
}

fn push_counter(out: &mut String, name: &str, help: &str, value: u64) {
    out.push_str(&format!("# HELP {name} {help}\n"));
    out.push_str(&format!("# TYPE {name} counter\n"));
    out.push_str(&format!("{name} {value}\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_start_at_zero() {
        let metrics = Metrics::default();
        let text = metrics.render_prometheus_text();
        assert!(text.contains("stellartrace_transactions_ingested_total 0"));
    }

    #[test]
    fn recording_events_increments_expected_counters() {
        let metrics = Metrics::default();
        metrics.record_transaction_ingested();
        metrics.record_alert_created();
        metrics.record_ai_recommendation(true);
        metrics.record_ai_recommendation(false);
        metrics.record_investigator_decision();

        let text = metrics.render_prometheus_text();
        assert!(text.contains("stellartrace_transactions_ingested_total 1"));
        assert!(text.contains("stellartrace_alerts_created_total 1"));
        assert!(text.contains("stellartrace_ai_recommendations_total 2"));
        assert!(text.contains("stellartrace_ai_fallback_total 1"));
        assert!(text.contains("stellartrace_investigator_decisions_total 1"));
    }

    #[test]
    fn output_is_valid_prometheus_text_shape() {
        let metrics = Metrics::default();
        let text = metrics.render_prometheus_text();
        for line_group in text.trim_end().split('\n').collect::<Vec<_>>().chunks(3) {
            assert!(line_group[0].starts_with("# HELP "));
            assert!(line_group[1].starts_with("# TYPE "));
            assert!(!line_group[2].starts_with('#'));
        }
    }
}
