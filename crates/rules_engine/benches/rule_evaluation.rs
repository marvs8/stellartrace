//! Throughput benchmark for the rules engine: how many transactions per
//! second can be evaluated against the full default rule set with a
//! realistic amount of account history in context.

use chrono::{Duration, Utc};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::collections::{HashMap, HashSet};
use stellartrace_common::{Asset, NormalizedTransaction};
use stellartrace_rules_engine::{RuleContext, RulesConfig, RulesEngine};

fn make_tx(offset_secs: i64, amount: &str) -> NormalizedTransaction {
    NormalizedTransaction {
        tx_hash: format!("bench-{offset_secs}"),
        ledger_sequence: 1,
        source_account: "GBENCHACCOUNT".into(),
        destination_account: Some("GBENCHDEST".into()),
        asset: Asset::Native,
        amount: amount.into(),
        timestamp: Utc::now() + Duration::seconds(offset_secs),
        is_soroban_invocation: false,
        contract_id: None,
        metadata: HashMap::new(),
    }
}

fn bench_rule_evaluation(c: &mut Criterion) {
    let engine = RulesEngine::with_default_rules();
    let config = RulesConfig::default();

    // A moderately busy account: 100 prior transactions spread over the
    // last hour, so window-filtering rules have realistic work to do.
    let history: Vec<NormalizedTransaction> = (0..100).map(|i| make_tx(-(i * 30), "100")).collect();
    let ctx = RuleContext::new(history, HashSet::new());
    let tx = make_tx(0, "25000");

    c.bench_function("evaluate_default_rules_with_100_tx_history", |b| {
        b.iter(|| {
            let result = engine.evaluate(black_box(&tx), black_box(&ctx), black_box(&config));
            black_box(result);
        });
    });

    let empty_ctx = RuleContext::default();
    c.bench_function("evaluate_default_rules_no_history", |b| {
        b.iter(|| {
            let result = engine.evaluate(black_box(&tx), black_box(&empty_ctx), black_box(&config));
            black_box(result);
        });
    });
}

criterion_group!(benches, bench_rule_evaluation);
criterion_main!(benches);
