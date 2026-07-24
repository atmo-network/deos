---
page_type: concept
title: Generated Wiki
summary: The DEOS wiki is a self-contained interpretation product derived from project truth and shaped as a dense domain graph for humans, agents, and the reference client.
locale: en
canonical_page_id: generated-wiki
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/README.md
  - ../../docs/web-client.architecture.en.md
  - ../../web-client/README.md
  - ../../.agents/skills/wiki-sync/SKILL.md
  - ../_meta/navigation.json
  - ../_meta/state.json
  - ../_meta/graph.json
  - ../_meta/aliases.json
  - ../_meta/locales.json
  - ../_meta/search.json
status: active
audience: newcomer
tags:
  - wiki
  - documentation
  - onboarding
  - web-client
related:
  - Domain Map
  - Reference Client
  - UI Kit and Domain DAG
  - First Steps
  - Agent Coordination
  - Core Terms
last_compiled: 2026-07-21
confidence: 0.9
---

# Generated Wiki

## Summary

The DEOS wiki is a self-contained interpretation product. It is grounded in repository truth, but readers should not need to leave the wiki to understand a page.

Its job is to turn DEOS from a file tree into a semantic domain graph for humans, agents, and the reference client: Economic Physics, autonomous actors, routing, governance, staking, read models, client, tooling, and future gates.

## Page Contract

A good wiki page:

- Explains its local concept directly;
- Names the domain it belongs to;
- Links to neighboring wiki pages;
- Avoids repeating full explanations owned by another page;
- Keeps source provenance in metadata rather than making source documents the reading path.

The wiki may synthesize multiple source concepts into one page when that creates a clearer domain boundary. Use [Domain Map](domain-map.en.md) as the top-level owner of domain topology.

## Metadata, Stable IDs, and Client Use

Together, the reference client, agents, and validation scripts use the compiled graph under `wiki/_meta/`; no single consumer needs to load every manifest:

- `navigation.json` orders sections and frontend summaries;
- `state.json` records page status, audience, confidence, paths, and provenance;
- `graph.json` stores nodes and typed reading relations;
- `aliases.json` routes curated search terms to canonical page ids;
- `locales.json` maps each page id to localized Markdown paths;
- `search.json` stores at most 12,000 plain-text characters per page and locale so client body search can load one bounded manifest instead of every Markdown chunk.

A page id is the stable identity; locale files are renderings of it:

```text
page id: token-surfaces
  en -> concepts/token-surfaces.en.md
  ru -> concepts/token-surfaces.ru.md
```

Graph edges such as `uses`, `extends`, `guides`, and `recommends` describe conceptual or reading relationships, not runtime dependencies. Provenance points back to authoritative project sources, while confidence indicates the maturity of the generated projection rather than protocol truth.

These manifests support browsing, search, and graph traversal. The prose still needs to stand on its own.

## Confidence Bands

Wiki confidence measures evidence maturity for one page, not probability, prose quality, project quality, or expected market behavior. Reviewers score the weakest material claim against source authority, claim coverage, freshness, and contradiction pressure.

The wiki uses conservative `0.05` bands:

- `0.95` — direct, current, nearly complete owner-source coverage;
- `0.90` — strongly grounded synthesis with minor distributed-evidence risk;
- `0.85` — grounded but partial, highly synthetic, or under freshness pressure;
- `0.80` — materially incomplete, stale, indirect, or contradiction-prone;
- `0.75` and below — weak support or known material errors requiring remediation.

Page length, source count, and graph degree do not raise confidence. Shared `state.json` confidence uses the lower locale score, and the consolidation audit reports source revisions newer than `last_compiled`.

## Trust Boundary and Evolution

The web client renders repo-local wiki markdown directly because the wiki is trusted reviewed repository content, not user input. Safety belongs to repository validation: reject raw HTML blocks, dangerous URL schemes, inline DOM event handlers, and frontmatter summary lines with extra value-side colons.

When evolving the wiki, update the owner page first, replace duplicated explanations elsewhere with owner links, keep provenance in metadata, and validate the trust contract plus link shape.

## Related

- [Domain Map](domain-map.en.md)
- [Reference Client](../overview/reference-client.en.md)
- [First Steps](../getting-started/first-steps.en.md)
- [Agent Coordination](../usage/agent-coordination.en.md)
- [Core Terms](../glossary/core-terms.en.md)
