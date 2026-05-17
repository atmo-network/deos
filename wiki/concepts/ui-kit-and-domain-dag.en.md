---
page_type: concept
title: UI Kit and Domain DAG
summary: The DEOS web client keeps reusable presentation controls in UI Kit and uses Domain DAG checks to keep ownership boundaries explicit across widgets, layout, domains, adapters, and system wiring.
locale: en
canonical_page_id: ui-kit-and-domain-dag
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/web-client.architecture.en.md
  - ../../web-client/README.md
  - ../../web-client/src/lib/ui/README.md
status: active
audience: newcomer
tags:
  - web-client
  - ui-kit
  - domain-dag
  - frontend-architecture
related:
  - Reference Client
  - Read-Model Split
  - Repository Structure
  - Contributing Guidelines
last_compiled: 2026-05-17
confidence: 0.92
---

# UI Kit and Domain DAG

## Summary

The DEOS web client uses two complementary architecture tools:

- `UI Kit` owns reusable presentation primitives.
- `Domain DAG` owns local dependency and ownership discipline.

Together they prevent raw-control duplication, vague `shared/` buckets, hidden adapter reach-through, and unclear boundaries between widgets, layout, domains, adapters, and system wiring.

## UI Kit Contract

UI Kit lives under `web-client/src/lib/ui/`. It owns presentation-only primitives and interaction wrappers: buttons, cards, notices, badges, detail rows, form fields, select fields, textareas, popovers, side panels, read-model badges, presentation formatting, and class merging.

Its job is to make the safe path the default path:

- Reusable buttons default to `type="button"`;
- Form primitives own label/control wiring and hydration-safe ids;
- Class merging is centralized;
- Widgets pass domain labels, values, and callbacks into primitives instead of rebuilding primitive behavior locally.

UI Kit must stay foundation-only. It should not import market, governance, wallet, adapter, or other product slices.

## Domain DAG Contract

Domain DAG is the client-local ownership gate configured by `web-client/domain-dag.json`.

It checks that:

- Imports stay acyclic;
- Source files keep ownership headers;
- Generic shared buckets do not reappear;
- Widgets do not import concrete adapter internals;
- UI Kit does not import product domains;
- Domain slices do not hide ownership by reaching around public entrypoints;
- Widget size and callback/function surfaces remain visible warning signals.

The goal is not more folders. The goal is that existing folders tell the truth.

## Placement Rule

Use this routing rule before adding a helper:

- Reusable visual primitive: `src/lib/ui/`;
- Browser/session infrastructure: `src/lib/system/`;
- Wallet account or signer concern: `src/lib/wallet/`;
- Governance labels, payload helpers, or projections: `src/lib/governance/`;
- Transport implementation: `src/lib/adapters/`;
- Economic/product composition: `src/lib/widgets/`;
- Broad cross-cutting contract: root foundation files such as `read-model.ts` or `economics.ts`.

If none fits, a generic `shared/` folder is still the wrong default. First identify the owner domain.

## Newcomer Map

Read the client tree this way: UI Kit is reusable presentation infrastructure, widgets are visible product actions, domain slices own state and contracts, adapters talk to outside systems, and system composes shell/session wiring.

## Related

- [Reference Client](../overview/reference-client.en.md)
- [Read-Model Split](read-model-split.en.md)
- [Repository Structure](../implementation/repository-structure.en.md)
- [Contributing Guidelines](../community/contributing.en.md)
