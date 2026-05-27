# Governance

## Model: BDFL with Maintainers

**BDFL:** John Canady Jr. (john@ai-nhancement.com)

The BDFL has final authority over all project decisions: roadmap, architecture, releases, and contributor access. The BDFL may appoint maintainers to share operational responsibilities.

## Maintainer admission

The BDFL may admit maintainers by public announcement in the repository. A maintainer gains write access to the repository and participates in triage and review. Admission requires a clean contribution history and demonstrated familiarity with the project's workflow discipline.

## Maintainer removal

The BDFL may remove a maintainer at any time. A maintainer may resign by notifying the BDFL. Removal or resignation is recorded in the repository history.

## Conflict of interest

Maintainers must disclose any conflict of interest before participating in a decision where that conflict applies. The BDFL makes the final call when a conflict is disclosed.

## BDFL succession

If the BDFL is permanently unable to fulfill the role, the most senior active maintainer assumes the BDFL role on an interim basis and the community selects a successor by rough consensus within 60 days. "Permanently unable" means: death, permanent incapacitation, or a written resignation with no named successor.

## Emergency freeze

The BDFL may declare an emergency freeze that halts all merges to `main` pending resolution of a named issue (security incident, governance dispute, legal matter). The freeze declaration and the resolution will be recorded as `EmergencyFreezeDeclaration` audit-store records. The `EmergencyFreezeDeclaration` record type is a v1.1 deliverable; in v1, freeze events are recorded as plain `PlanAmendment` records with a `freeze` tag in the metadata until the dedicated record type is available.

An adversarial emergency freeze — a freeze declared by someone other than the BDFL using technical access — is invalid. Any merge blocked by such a freeze is un-blocked by the BDFL's counter-declaration, which also triggers an automatic governance review.

## Amendments

Governance changes require a pull request reviewed by at least one maintainer (if any exist) and approved by the BDFL.
