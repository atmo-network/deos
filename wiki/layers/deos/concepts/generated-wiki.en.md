---
type: concept
title: Generated Wiki
description: The DEOS wiki is a self-contained interpretation product derived from project truth and shaped as a dense domain graph for readers and the reference client.
locale: en
canonical_page_id: generated-wiki
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../../../docs/deos/README.md
  - resource: ../../../../web-client/docs/architecture.en.md
  - resource: ../../../../web-client/README.md
  - resource: ../../../../docs/deos/wiki-publication.contract.en.md
  - resource: ../_meta/navigation.json
  - resource: ../_meta/state.json
  - resource: ../_meta/graph.json
  - resource: ../_meta/aliases.json
  - resource: ../_meta/locales.json
status: stable
audience: newcomer
tags:
  - wiki
  - documentation
  - onboarding
  - web-client
related:
  - Domain Map
  - Reference Client
  - First Steps
  - Core Terms
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

Together, the website compiler, the reference client, agents, and validation scripts use each plane's compiled graph, under `wiki/layers/deos/_meta/` for DEOS; no single consumer needs to load every manifest:

- `navigation.json` orders sections and frontend summaries;
- `state.json` records explicit page status, audience, paths, and provenance;
- `graph.json` stores nodes and typed reading relations;
- `aliases.json` routes search terms to canonical page ids;
- `locales.json` maps each page id to localized Markdown paths.

A page id is the stable identity; locale files are renderings of it:

```text
page id: token-surfaces
  en -> concepts/token-surfaces.en.md
  ru -> concepts/token-surfaces.ru.md
```

Graph edges such as `uses`, `extends`, `guides`, and `recommends` describe conceptual or reading relationships, not runtime dependencies. Structured provenance points back to authoritative project sources.

These manifests support browsing, search, and graph traversal by agents. The prose still needs to stand on its own.

## Layers, Website and Publication

`wiki/layers/` contains the knowledge planes. `wiki/layers/_meta/planes.json` declares its knowledge planes, and each plane is a self-contained bundle in its own directory. DEOS ships one base plane, `wiki/layers/deos/`. A fork that launches its own network adds an instance plane beside it and leaves the DEOS plane untouched, so every instance change stays visibly outside the base plane.

One static website compiles all declared planes into a single graph. The graph stays behind the reader interface: it fixes page identity, order and layers, while readers see a contents page, a foldable sidebar tree and a layer tag on every page. mdBook renders the HTML, the reader switches language and light or dark theme, and search runs in the browser without a backend. Every link is relative, so the same site works from any address, including a GitHub Pages project path. Pages also carry descriptions, language alternates and a sitemap for search engines.

A dedicated publication workflow validates and builds the static site on Node and commits it to the `gh-pages` branch. GitHub Pages serves that branch once the repository enables it. The workflow belongs to the wiki tooling, not to project validation.

## Evidence Signals

The wiki uses verifiable signals rather than page-level freshness dates or subjective confidence scores. Each page declares structured source provenance, explicit lifecycle status, locale identity, and related concepts. Shared manifests preserve source lists, graph reachability, aliases, and locale parity.

These signals prove structure and traceability, not semantic correctness. Contradictions, missing evidence, stale claims, and supersession must be stated explicitly and repaired against the owning sources.

## Trust Boundary and Evolution

The website and the transitional reader in the web client render repo-local wiki Markdown directly because the wiki is trusted reviewed repository content, not user input. Safety belongs to repository validation: reject raw HTML blocks, dangerous URL schemes, inline DOM event handlers, and malformed YAML frontmatter. YAML block scalars and quoted punctuation remain valid; ambiguous plain scalars such as an unquoted `description: Topic: details` do not.

When evolving the wiki, update the owner page first, replace duplicated explanations elsewhere with owner links, keep provenance in metadata, and validate the trust contract plus link shape.

## Related

- [Domain Map](domain-map.en.md)
- [Reference Client](../overview/reference-client.en.md)
- [First Steps](../getting-started/first-steps.en.md)
- [Core Terms](../glossary/core-terms.en.md)
