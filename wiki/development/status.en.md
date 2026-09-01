---
type: status
title: Development Status
description: Current capabilities and present limitations of the DEOS framework and reference client.
locale: en
canonical_page_id: status
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../CHANGELOG.md
  - resource: ../../web-client/README.md
  - resource: ../../web-client/docs/architecture.en.md
status: stable
audience: newcomer
tags:
  - development
  - status
related:
  - Three-Layer Validation
  - Reference Client
  - Generated Wiki
---

# Development Status

## Summary

DEOS combines a FRAME runtime, a browser reference client, development tools, and documentation. This page describes current capabilities and their limits; it does not certify a release or a deployed network.

## Current capabilities

- **Economic mechanisms**: TMCTOL issuance, routing, treasury-owned liquidity, and Actor-mediated economic flows form the reference composition.
- **DEOS Actors**: deterministic Actor Contract execution uses a bounded scheduler. Mutable Actors can retry Temporary Step failures while preserving committed prefixes without whole-contract rollback.
- **Staking and governance**: multi-asset share accounting and bounded governance tracks provide staking, typed proposals, and protection mechanisms.
- **Reference client**: the SvelteKit client provides wallet/account selection, tracked-asset balances and transfers, swap, staking, governance, automation, wiki browsing, and transaction feedback.
- **Development tools**: project scripts, benchmarks, metadata export, and package/runtime/client checks support development and validation.

[Domain Map](../concepts/domain-map.en.md) explains how these areas relate.

## Present limits

The client is a reference interface, not the source of protocol truth. Its canonical chain views expose bounded state. Wallet transfers cover tracked assets; this is not an exhaustive asset-discovery claim.

Browser session history and charts are not an archive. Indexed history, search, and analytics are materialized data, distinct from current canonical chain state.

The presence of a feature or a validation command does not by itself establish production readiness, network deployment, or validation of the current source tree.

## Related

- [Domain Map](../concepts/domain-map.en.md)
- [Three-Layer Validation](three-layer-validation.en.md)
- [Reference Client](../overview/reference-client.en.md)
- [Generated Wiki](../concepts/generated-wiki.en.md)
