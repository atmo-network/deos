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
  - Core Terms
last_compiled: 2026-05-17
confidence: 0.9
---

# Generated Wiki

## Summary

The DEOS wiki is a self-contained interpretation product. It is grounded in repository truth, but readers should not need to leave the wiki to understand a page.

Its job is to turn DEOS from a file tree into a semantic domain graph for humans, agents, and the reference client: economic physics, autonomous actors, routing, governance, staking, read models, client, tooling, and future gates.

## Page Contract

A good wiki page:

- Explains its local concept directly;
- Names the domain it belongs to;
- Links to neighboring wiki pages;
- Avoids repeating full explanations owned by another page;
- Keeps source provenance in metadata rather than making source documents the reading path.

The wiki may synthesize multiple source concepts into one page when that creates a clearer domain boundary. Use [Domain Map](domain-map.en.md) as the top-level owner of domain topology.

## Metadata and Client Use

The reference client consumes compiled manifests from `wiki/_meta/`:

- `navigation.json` for sections and summaries;
- `aliases.json` for search terms;
- `graph.json` for typed page relations;
- `state.json` for status, confidence, sources, and audience;
- `locales.json` for supported locales and paths.

These manifests support browsing and search. The prose still needs to stand on its own.

## Trust Boundary and Evolution

The web client renders repo-local wiki markdown directly because the wiki is trusted reviewed repository content, not user input. Safety belongs to repository validation: reject raw HTML blocks, dangerous URL schemes, inline DOM event handlers, and frontmatter summary lines with extra value-side colons.

When evolving the wiki, update the owner page first, replace duplicated explanations elsewhere with owner links, keep provenance in metadata, and validate the trust contract plus link shape.

## Related

- [Domain Map](domain-map.en.md)
- [Reference Client](../overview/reference-client.en.md)
- [Wiki Graph Metadata](../usage/wiki-graph-metadata.en.md)
- [UI Kit and Domain DAG](ui-kit-and-domain-dag.en.md)
- [First Steps](../getting-started/first-steps.en.md)
- [Core Terms](../glossary/core-terms.en.md)
