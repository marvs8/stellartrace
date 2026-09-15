//! Throughput benchmark for anomaly score computation across a range of
//! triggered-rule-set sizes, since scoring runs once per flagged
//! transaction and its cost should stay negligible relative to the rules
//! engine itself.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::collections::HashMap;
use stellartrace_common::{Severity, TriggeredRule};
use stellartrace_scoring::{compute_score, severity_for_score, ScoringWeights};

fn make_triggers(n: usize) -> Vec<TriggeredRule> {
    (0..n)
        .map(|i| TriggeredRule {
            rule_id: format!("rule-{i}"),
            rule_name: format!("Rule {i}"),
            severity: match i % 4 {
                0 => Severity::Low,
                1 => Severity::Medium,
                2 => Severity::High,
                _ => Severity::Critical,
            },
            reason: "benchmark fixture".into(),
            evidence: HashMap::new(),
        })
        .collect()
}

fn bench_scoring(c: &mut Criterion) {
    let weights = ScoringWeights::default();

    for size in [1usize, 5, 20, 100] {
        let triggers = make_triggers(size);
        c.bench_function(&format!("compute_score_{size}_triggers"), |b| {
            b.iter(|| {
                let score = compute_score(black_box(&triggers), black_box(&weights));
                black_box(severity_for_score(black_box(score)));
            });
        });
    }
}

criterion_group!(benches, bench_scoring);
criterion_main!(benches);
