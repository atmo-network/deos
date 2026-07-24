---
page_type: glossary
title: Core Terms
summary: A compact glossary for the most important DEOS and TMCTOL terms. Use this page first when project vocabulary, abbreviations, framework-versus-standard naming, frontend architecture terms, wiki roles, or status surfaces feel ambiguous.
locale: en
canonical_page_id: core-terms
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../docs/README.md
  - ../../BACKLOG.md
  - ../../CHANGELOG.md
  - ../../docs/tmctol.specification.en.md
  - ../../docs/core.architecture.en.md
  - ../../docs/aaa.specification.en.md
  - ../../docs/governance.specification.en.md
  - ../../docs/governance.architecture.en.md
  - ../../docs/read-model.contract.en.md
  - ../../docs/web-client.architecture.en.md
  - ../../web-client/README.md
status: active
audience: newcomer
tags:
  - glossary
  - terminology
related:
  - DEOS Framework Overview
  - TMCTOL Standard
  - Token-Driven Automation
  - Governance Overview
  - Governance Domains
  - $BLDR Builder Economy
  - Read-Model Split
  - UI Kit and Domain DAG
  - Generated Wiki
  - Reading Paths
  - Development Status
  - Newcomer FAQ
last_compiled: 2026-07-20
confidence: 0.9
---

# Core Terms

## Summary

This glossary is a lookup surface, not a second explanation layer. Each term stays short and points outward through related pages for full context.

## Terms

### DEOS

`Deterministic Economic Operating System`. The framework identity for the repository and reference stack.

### TMCTOL

`Token Minting Curve + Treasury-Owned Liquidity`. The current flagship economic standard running on DEOS.

### TMC

`Token Minting Curve`. The mint-only linear issuance mechanism that defines the current price ceiling for new supply.

### TOL

`Treasury-Owned Liquidity`. Protocol-controlled liquidity segmented into bucket domains.

### AAA

`Account Abstraction Actors`. In DEOS, this names the full runtime system: the pallet, scheduler, lifecycle rules, and execution environment.

### AA-Actor

One concrete bounded runtime instance inside the broader AAA system.

### Axial Router

The bounded route-selection mechanism for protocol trades and fee flow.

### Balance Ingress

The token-driven trigger where assets arriving in the system can drive the next deterministic state transition.

### Governance Domain

One typed governance cell binding subject, power surface, payload family, cadence, and execution authority.

### Primary Track

The proposal-decision lane, either binary or invoice-shaped depending on domain and payload.

### Protection Track

The constitutional `Veto / Pass` lane, separate from ordinary proposal approval.

### Proposal Payload Kind

The typed category of action a governance proposal wants to authorize.

### Execution Authority

The authority level a successful governance payload may reach when enacted.

### GovXP

A bounded governance-participation signal for reward or reputation logic.

### Canonical Projection

A bounded on-chain view intended for direct client consumption as part of the live protocol contract.

### Materialized View

An indexed or externally reconstructed view used for archive, search, dashboards, or analytics rather than bounded consensus truth.

### Native / `$NTVE`

The sovereign base token of the current reference line.

### `$VETO`

The protection token used for the strategic constitutional surface on the current line.

### `$BLDR`

The flagship tactical governance and builder-coordination token on the current line. See [$BLDR Builder Economy](../concepts/builder-economy.en.md).

### `stXXX`

Transferable staking receipts representing share-based ownership in staking pools.

### UI Kit

The repository-local frontend presentation kit for reusable UI primitives.

### Domain DAG

The web-client ownership discipline and validator for import direction, headers, and boundary drift.

### Widget

A browser-facing product surface such as swap, wallet, staking, governance, chart, automation, log, account, settings, status, or wiki reading.

### Layout

The web-client subsystem that arranges panes, tabs, reserved lanes, header, footer, and sidebar without becoming a product domain.

### Generated Wiki

The self-contained `/wiki` interpretation layer for onboarding, navigation, metadata consumption, and frontend rendering.

### Reading Path

A role-based wiki route for a specific task, such as economics, runtime, governance, client, status, or tooling work.

### Architecture Document

A release-agnostic implementation mirror for a shipped subsystem.

### `BACKLOG.md`

The canonical open-work file for closable deliverables, explicit gates, and bounded epics.

### `CHANGELOG.md`

The canonical completed-delivery and release-history file.

### Development Status

The wiki current-state map for baseline domains, active focus, open boundaries, and gated work.

## Related

- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [TMCTOL Standard](../concepts/tmctol-standard.en.md)
- [Token-Driven Automation](../concepts/token-driven-automation.en.md)
- [Governance Overview](../overview/governance-overview.en.md)
- [Governance Domains](../concepts/governance-domains.en.md)
- [$BLDR Builder Economy](../concepts/builder-economy.en.md)
- [Read-Model Split](../concepts/read-model-split.en.md)
- [Generated Wiki](../concepts/generated-wiki.en.md)
- [Reading Paths](../getting-started/reading-paths.en.md)
- [Development Status](../development/status.en.md)
- [Newcomer FAQ](../faq/newcomer-faq.en.md)
