---
page_type: concept
title: Threat Model
summary: A central map of major DEOS risks, attack shapes, mitigations, and owner surfaces.
locale: en
canonical_page_id: threat-model
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/core.architecture.en.md
  - ../../docs/tmctol.specification.en.md
  - ../../docs/governance.specification.en.md
  - ../../docs/web-client.architecture.en.md
status: active
audience: developer
tags:
  - concept
  - threat-model
  - security
  - governance
related:
  - Invariant Map
  - What DEOS Is Not
  - Economic Claim Levels
  - Governance Domains
last_compiled: 2026-05-17
confidence: 0.82
---

# Threat Model

## Summary

DEOS is not risk-free. It is designed so important risks are named, bounded, and routed to the right owner surface instead of hidden inside product language.

## Central Threats

| Threat                  | Shape                            | Mitigation                   | Owner           |
| ----------------------- | -------------------------------- | ---------------------------- | --------------- |
| Governance drains TOL   | reserve redirect                 | domain payloads + protection | governance/spec |
| Router bypass           | fee/burn avoided                 | router gateway               | router          |
| Bucket misuse           | provenance collapse              | segmented lanes              | TMCTOL + AAA    |
| Indexer truth confusion | archive shown as truth           | provenance badges            | client/docs     |
| Collator trust phase    | trust mistaken as permissionless | launch-line constraint       | runtime/ops     |
| Actor graph stuck       | cooldown/outage/oracle gap       | retry + cooldown             | AAA             |
| Parameter griefing      | params leave assumptions         | bounded settings             | runtime/gov     |
| LP valuation attack     | LP overvalued/double-counted     | conservative custody         | staking/gov     |
| Frontend provenance lie | UI hides data class              | read-model contract          | web client      |

## Reading Rule

A threat is not “solved” just because a page mentions it. It is controlled only when the owner surface has a bounded mechanism and validation route.

## Related

- [Invariant Map](invariant-map.en.md)
- [What DEOS Is Not](what-deos-is-not.en.md)
- [Economic Claim Levels](economic-claim-levels.en.md)
- [Governance Domains](governance-domains.en.md)
