---
page_type: overview
title: Reference Client
summary: The DEOS web client is an on-chain-first reference UI for live protocol flows. It separates product widgets from layout infrastructure, centralizes reusable UI primitives, gates ownership with Domain DAG, and keeps data provenance visible.
locale: en
canonical_page_id: reference-client
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/web-client.architecture.en.md
  - ../../web-client/README.md
  - ../../web-client/src/lib/ui/README.md
  - ../../docs/read-model.contract.en.md
status: active
audience: newcomer
tags:
  - overview
  - web-client
  - product
  - ui-kit
  - domain-dag
related:
  - First Steps
  - Read-Model Split
  - Newcomer FAQ
  - Core Terms
last_compiled: 2026-07-24
confidence: 0.9
---

# Reference Client

## Summary

The repository-local web client is the browser-facing DEOS reference client. It is `on-chain-first`: its main live product flows should rely on bounded canonical runtime surfaces rather than quiet off-chain reconstruction.

The ownership model is explicit: widgets express product actions, layout owns pane and lane mechanics, UI Kit owns reusable presentation primitives, system owns browser/session wiring, and adapters remain transport boundaries.

## Product and Layout Contract

Current product surfaces include balances, route previews, governance views, automation status, staking state, session-bounded charting, and wiki reading.

The automation surface reads known System actors directly from bounded `ActorHot`, `ActorProgram`, and sparse `ContinuationState`. It shows the current logical-run nonce, suspended attempt, unresolved step, and last-attempt block. This is live canonical state, not an attempt-history archive. The authoring contract permits `RetryLater` only for Mutable plans.

The client keeps economic functions separate from layout infrastructure:

- Widgets are visible product surfaces such as swap, wallet, governance, charts, staking, automation, and wiki;
- Layout is pane, tile, split, tab, footer, header, sidebar, and reserved-lane machinery;
- Reserved edge lanes are developer-configured shell zones, not user-reorderable economic panes.

Widgets adapt to pane width and height instead of assuming one desktop-only stack. The reusable backbone owns task hierarchy, minimum phases, accessibility, provenance, and safety; downstream forks own brand, palette, typography, effects, terminology, and enabled product policy.

## Responsive and Disconnected Behavior

The desktop workspace persists a bounded tile tree with adjacent-only resizing. Below the mobile breakpoint, the client projects the same panels into a one-dimensional, single-expanded accordion with a separate persisted order; moving or expanding a mobile task does not rewrite desktop topology. The account/settings lane becomes a focus-contained bottom sheet instead of displacing the workspace.

Chain absence never becomes a fabricated zero. Widgets distinguish loading, live snapshots, retained stale evidence, explicit preview data, unconfigured providers, and errors. Local account selection, signer discovery, address handling, drafts, Wiki reading, and receipts may remain useful offline, while balance-derived actions, quotes, and execution require a live canonical snapshot.

## Ownership and Feedback

The client uses UI Kit and Domain DAG discipline to keep repeated controls and structural boundaries in owner layers. Widgets should express product intent, not rebuild primitive controls or reach through adapter internals.

Execution feedback is centralized: `LogWidget` is the main transaction/progress surface, while action widgets stay focused on initiating actions. This follows the same anti-duplication rule as UI primitives and provenance badges.

## Data and Wiki Boundaries

The client must label both protocol provenance and browser realization honestly. Session-built views should not pretend to be retained archive truth. Long-range analytics and archives belong to indexed or materialized providers, not to direct chain truth.

Use [Read-Model Split](../concepts/read-model-split.en.md) for the canonical data model.

The web client renders generated wiki content as trusted repo-local markdown and uses compiled metadata for navigation, aliases, graph links, state, and provenance. Use [Generated Wiki](../concepts/generated-wiki.en.md) for the trust boundary and wiki-evolution rules.

## Related

- [First Steps](../getting-started/first-steps.en.md)
- [Read-Model Split](../concepts/read-model-split.en.md)
- [Generated Wiki](../concepts/generated-wiki.en.md)
- [Newcomer FAQ](../faq/newcomer-faq.en.md)
- [Core Terms](../glossary/core-terms.en.md)
