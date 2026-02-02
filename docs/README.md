# Documentation Index

## Core Documents

| Document | Purpose | Audience |
|----------|---------|----------|
| [DESIGN.md](DESIGN.md) | System architecture and component design | Developers |
| [ASSESSMENT.md](ASSESSMENT.md) | Quantitative and qualitative system evaluation | Stakeholders |
| [ISSUES.md](ISSUES.md) | Bug tracking and issue triage | Developers |
| [ARCHITECTURE_SPECULATION.md](ARCHITECTURE_SPECULATION.md) | Future directions and design alternatives | Architects |
| [CRITIQUE.md](CRITIQUE.md) | Adversarial review and limitations | Risk Managers |

## Quick Links

### For Developers
- System design: [DESIGN.md](DESIGN.md)
- Open issues: [ISSUES.md](ISSUES.md)
- Architecture decisions: [../ARCHITECTURE.md](../ARCHITECTURE.md)

### For Risk Managers
- Risk assessment: [ASSESSMENT.md](ASSESSMENT.md)
- Known limitations: [CRITIQUE.md](CRITIQUE.md)
- Backtest results: [../BASELINE.md](../BASELINE.md)

### For Operators
- Configuration: [DESIGN.md#6-configuration](DESIGN.md#6-configuration)
- Deployment checklist: [DESIGN.md#8-deployment](DESIGN.md#8-deployment)

## Document Status

| Document | Last Updated | Status |
|----------|--------------|--------|
| DESIGN.md | 2026-02-01 | Current |
| ASSESSMENT.md | 2026-02-01 | Current |
| ISSUES.md | 2026-02-01 | Active |
| ARCHITECTURE_SPECULATION.md | 2026-02-01 | Exploratory |
| CRITIQUE.md | 2026-02-01 | Current |

## Contributing

When updating documentation:
1. Update the "Last Updated" date
2. Change status if needed (Draft → Current → Archived)
3. Cross-reference related documents
4. Run `cargo test` to ensure code examples still work
