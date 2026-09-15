# ADR 0001: Deterministic, Explainable Rules Engine Over a Learned Model

## Status
Accepted

## Context
Fraud/anomaly detection is often built on ML models (supervised classifiers, anomaly-detection models, embeddings + nearest-neighbor). These can capture subtler patterns than hand-written rules, but they are harder to explain, harder to audit, and their output can drift as the underlying model or training data changes — none of which sit well with a compliance-facing triage system where an investigator (and potentially a regulator) needs to know *exactly* why a transaction was flagged.

## Decision
The primary detection layer (`stellartrace-rules-engine`) is a set of independent, pure functions (`Rule::evaluate(transaction, context, config) -> Option<TriggeredRule>`), each producing a structured, human-readable `reason` and a machine-readable `evidence` map. Given the same inputs, a rule always produces the same output — no hidden state, no randomness, no model weights to version separately from code.

Anomaly scoring (`stellartrace-scoring`) is likewise a deterministic weighted aggregation over triggered rules, not a learned scoring function.

## Consequences
- **Positive**: every flag is explainable without reverse-engineering a model; rules are trivially unit-testable at their exact boundary conditions; behavior is reproducible for audit and dispute resolution; new detection logic ships as ordinary code review, not a model retraining/redeployment pipeline.
- **Negative**: the engine cannot learn subtle, high-dimensional patterns a trained model might catch, and thresholds require deliberate tuning (see [configuration.md](../configuration.md)) rather than adapting automatically.
- **Mitigation for the negative**: the AI advisory layer (see [ADR 0002](0002-ai-advisory-only-boundary.md)) is where softer, pattern-based judgment lives — deliberately kept advisory rather than promoted into the deterministic detection path, so its inherent variability never becomes the thing that decides whether a transaction gets flagged in the first place.
