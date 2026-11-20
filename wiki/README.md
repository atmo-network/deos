# DEOS Wiki

The Wiki is an optional monorepo domain: it takes reviewed knowledge under [`layers/`](layers/index.md) and produces one static bilingual mdBook site. The [publication and embedding contract](../docs/deos/wiki-publication.contract.en.md) owns the integration boundary. The Wiki is not a prerequisite for the framework source package, and no Wiki command requires agent Skills.

## Read and contribute

- [Read the public DEOS Wiki](https://atmo-network.github.io/deos/).
- [English knowledge base](layers/deos/index.en.md) and [Русская база знаний](layers/deos/index.ru.md).
- [`layers/_meta/planes.json`](layers/_meta/planes.json) declares the planes. `layers/deos/` is the strict OKF v0.2 DEOS base plane. Downstream forks add their own plane beside it instead of copying or overriding the base plane.
- [Site README](site/README.md) explains renderer inputs, reader capabilities and output; [`scripts/`](scripts/) owns domain commands. The repository-wide [contributor protocol](../AGENTS.md) also applies here.

## Maintain the corpus

- Treat the Wiki as a reviewed projection of root and package-owned documentation, not a new specification or implementation diary. Preserve each source's evidence scope and limitations; keep plans and release work outside the published corpus.
- Keep EN/RU mirrors under one canonical page identity with equivalent claims. Use concise summaries, peer links and structured `sources[].resource` provenance pointing to the actual source owner. Keep navigation, locale, alias, graph and provenance manifests consistent, without rendering source-document paths as article links.
- Prefer updates or consolidation over new pages that duplicate existing knowledge. When retiring a page, reconcile localized mirrors, aliases, graph links and navigation reachability. Source storage, plane-qualified identities and public routes are separate contracts.
- Read the [adopted OKF reference](references/okf-reference.md) before changing schema or validation. Use the [native Russian guide](references/native-russian-style.md) for Russian prose; automated checks do not replace independent human review of substantive Russian text.
- After preparation, use `wiki/scripts/validate-wiki-trust.sh` for corpus/schema/trust changes, `wiki/scripts/audit-wiki-consolidation.sh` for structural growth and `wiki/scripts/audit-native-russian.sh` for Russian changes. The optional upstream OKF freshness check belongs to reference adoption, not routine builds.

## Validate and publish

- Run `./scripts/validate-local.sh wiki` from the repository root. It prepares pinned Node/npm and mdBook, validates the corpus, builds `wiki/site/dist/` with structural output checks and verifies the shipped native search data, all on Node without a browser; the check does not publish.
- Set `WIKI_BASE_PATH=/deos/ WIKI_SITE_URL=https://atmo-network.github.io/deos/` to validate the reference Pages deployment shape. `WIKI_BASE_PATH` selects the deployment prefix; `WIKI_SITE_URL` adds absolute canonical and alternate links and a sitemap. Neither changes page identity or source storage.
- Pull-request CI reports the Wiki profile separately from client and framework validation. The Wiki tooling does not read client implementation files or use client npm packages. Corpus provenance still resolves declared repository inputs, including the client package manifest and documentation. The [Pages workflow](../.github/workflows/wiki-pages.yml) runs the same profile before its authorized `gh-pages` update. `wiki/scripts/publish-pages.sh` is a dry run without `--push`; the reference site already uses Pages from `gh-pages` / root.
