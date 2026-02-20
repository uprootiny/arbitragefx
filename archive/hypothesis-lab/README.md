# hypothesis-lab (archived)

Hypothesis-driven research framework for strategy development.

## What's here

- **hypothesis.rs** (828 lines) — Hypothesis data model with development
  lifecycle: Proposed → Testable → Testing → Supported/Refuted/Inconclusive.
  Market regime classification. Evidence tracking. Development action
  suggestions.
- **research_lab.rs** (678 lines) — Interactive CLI for hypothesis management:
  list hypotheses, run targeted backtests, collect evidence, generate reports.

## Why archived

The hypothesis tracking has been superseded by `hypothesis_ledger.edn` with
Bayesian truth values (stv strength confidence) and the automated
`update_ledger` binary. The ledger approach is more formal — hypotheses
get Bayesian updates from walk-forward validation results rather than
manual evidence collection.

## Spin-off potential

The hypothesis lifecycle model (Proposed → Testable → Testing → verdict)
and the research_lab interactive CLI could form a general-purpose
"hypothesis-driven development" tool, not limited to trading.

## Quality

Compiles. Has tests. Clean data model. The research_lab CLI is
well-structured with subcommands.
