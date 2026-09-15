---
name: Feature request
about: Propose a new capability or fraud detection rule
title: "[Feature] "
labels: enhancement
---

## Problem

What gap or need does this address?

## Proposed solution

Describe the change. If proposing a new fraud rule, please include:
- What pattern it detects and why it's a fraud/anomaly signal.
- What configurable thresholds it needs (see `docs/configuration.md`).
- Whether it needs new fields on `NormalizedTransaction`/`RuleContext`, or works with existing ones.

## Design constraints to keep in mind

Please review [CONTRIBUTING.md](../../CONTRIBUTING.md#design-constraints-that-pull-requests-must-not-violate) — in particular, this proposal must not create a path for AI output to change alert status, and any new rule must be a pure, deterministic function.

## Alternatives considered

