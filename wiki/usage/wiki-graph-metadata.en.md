---
page_type: usage
title: Wiki Graph Metadata
summary: How to read the generated wiki metadata files, including navigation, state, graph, aliases, and locale mappings.
locale: en
canonical_page_id: wiki-graph-metadata
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../wiki/_meta/navigation.json
  - ../../wiki/_meta/state.json
  - ../../wiki/_meta/graph.json
  - ../../wiki/_meta/aliases.json
  - ../../wiki/_meta/locales.json
  - ../../.agents/skills/wiki-sync/SKILL.md
status: active
audience: developer
tags:
  - usage
  - wiki
  - metadata
  - graph
related:
  - Generated Wiki
  - Agent Coordination
  - Reference Client
  - Validation Troubleshooting
last_compiled: 2026-05-17
confidence: 0.86
---

# Wiki Graph Metadata

## Summary

The wiki is not only markdown pages. It also has shared metadata under `/wiki/_meta` so the reference client, agents, and validation scripts can read the same domain graph.

The markdown pages are the human explanation layer. The metadata files are the machine-readable map.

## Main Files

- `Navigation.json`: ordered reading/navigation sections for UI and human entrypoints.
- `State.json`: page registry with status, audience, confidence, paths, and provenance.
- `Graph.json`: nodes and typed edges between wiki pages.
- `Aliases.json`: lookup aliases that route alternate names to canonical page ids.
- `Locales.json`: locale map from page ids to localized markdown paths.

## How to Read a Page Id

A page id is the stable wiki identity. Locale-specific files are just renderings of that identity.

```text
page id: token-surfaces
  en -> concepts/token-surfaces.en.md
  ru -> concepts/token-surfaces.ru.md
```

Links in prose can be local markdown paths, but metadata should keep page ids stable so graph tools do not break when wording changes.

## Graph Edges

`graph.json` describes relationships such as `uses`, `extends`, `frames`, `guides`, or `recommends`. These are not runtime dependencies. They are reading and concept relationships.

Use edges to answer questions like:

- Which page owns this concept?
- Which pages should a newcomer read next?
- Which implementation pages depend on this domain idea?
- Which pages should be checked when a concept changes?

## Provenance and Confidence

`state.json` keeps provenance and confidence. Provenance points back to authoritative sources, while confidence tells readers and agents how mature the projection is.

A wiki page can be clear and useful while still pointing to `/docs` as its source of truth. That is the intended split.

## Related

- [Generated Wiki](../concepts/generated-wiki.en.md)
- [Agent Coordination](agent-coordination.en.md)
- [Reference Client](../overview/reference-client.en.md)
- [Validation Troubleshooting](validation-troubleshooting.en.md)
