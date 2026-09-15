//! Anomaly scoring: converts a set of deterministic rule triggers into a
//! single numeric score and severity band for the alert record.
//!
//! Deliberately simple and deterministic (weighted sum + diminishing
//! returns for stacked triggers) rather than a learned model — the score
//! must be explainable and reproducible from `triggered_rules` alone,
//! same as the rules themselves.

use serde::{Deserialize, Serialize};
use stellartrace_common::{Severity, TriggeredRule};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub low: f64,
    pub medium: f64,
    pub high: f64,
    pub critical: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self { low: 1.0, medium: 3.0, high: 6.0, critical: 12.0 }
    }
}

fn weight_for(weights: &ScoringWeights, severity: Severity) -> f64 {
    match severity {
        Severity::Low => weights.low,
        Severity::Medium => weights.medium,
        Severity::High => weights.high,
        Severity::Critical => weights.critical,
    }
}

/// Computes a 0..=100 anomaly score from the triggered rules. Each
/// additional trigger contributes with mildly diminishing weight (so ten
/// low-severity rules don't automatically outrank one critical rule),
/// implemented as `weight / sqrt(rank)`.
pub fn compute_score(triggered_rules: &[TriggeredRule], weights: &ScoringWeights) -> f64 {
    if triggered_rules.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<&TriggeredRule> = triggered_rules.iter().collect();
    sorted.sort_by(|a, b| weight_for(weights, b.severity)
        .partial_cmp(&weight_for(weights, a.severity))
        .unwrap());

    let raw: f64 = sorted
        .iter()
        .enumerate()
        .map(|(i, t)| weight_for(weights, t.severity) / ((i as f64 + 1.0).sqrt()))
        .sum();

    // Normalize against a saturation point rather than clamping hard,
    // so scores stay comparable across alerts with different rule counts.
    let saturation = weights.critical * 3.0;
    ((raw / saturation) * 100.0).min(100.0)
}

/// Maps a numeric score to a severity band for the alert record.
pub fn severity_for_score(score: f64) -> Severity {
    if score >= 75.0 {
        Severity::Critical
    } else if score >= 45.0 {
        Severity::High
    } else if score >= 20.0 {
        Severity::Medium
    } else {
        Severity::Low
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn rule(severity: Severity) -> TriggeredRule {
        TriggeredRule {
            rule_id: "r".into(),
            rule_name: "r".into(),
            severity,
            reason: "because".into(),
            evidence: HashMap::new(),
        }
    }

    #[test]
    fn no_triggers_score_zero() {
        assert_eq!(compute_score(&[], &ScoringWeights::default()), 0.0);
    }

    #[test]
    fn more_severe_triggers_score_higher() {
        let weights = ScoringWeights::default();
        let low_score = compute_score(&[rule(Severity::Low)], &weights);
        let critical_score = compute_score(&[rule(Severity::Critical)], &weights);
        assert!(critical_score > low_score);
    }

    #[test]
    fn stacking_triggers_increases_score_with_diminishing_returns() {
        let weights = ScoringWeights::default();
        let one = compute_score(&[rule(Severity::Medium)], &weights);
        let two = compute_score(&[rule(Severity::Medium), rule(Severity::Medium)], &weights);
        let three = compute_score(
            &[rule(Severity::Medium), rule(Severity::Medium), rule(Severity::Medium)],
            &weights,
        );
        assert!(two > one);
        assert!(three > two);
        // diminishing: each additional increment is smaller than the last
        assert!((three - two) < (two - one));
    }

    #[test]
    fn single_critical_does_not_get_dwarfed_by_many_lows() {
        let weights = ScoringWeights::default();
        let critical = compute_score(&[rule(Severity::Critical)], &weights);
        let many_lows = compute_score(
            &vec![rule(Severity::Low); 10],
            &weights,
        );
        assert!(critical > many_lows, "critical={critical} many_lows={many_lows}");
    }

    #[test]
    fn score_maps_to_expected_severity_bands() {
        assert_eq!(severity_for_score(0.0), Severity::Low);
        assert_eq!(severity_for_score(25.0), Severity::Medium);
        assert_eq!(severity_for_score(50.0), Severity::High);
        assert_eq!(severity_for_score(90.0), Severity::Critical);
    }
}
