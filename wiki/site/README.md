# Wiki mdBook Website

This is the Wiki domain’s compiler, theme and static-validation workspace. It reads `wiki/layers/`, produces one composed graph and prepares a metadata-ordered Markdown book for each locale. **mdBook renders every document;** the compiler adds only a content-free site-root redirect. The project Wiki validation profile and Pages workflow build this optional workspace independently of the framework and web-client.

The public integration boundary is the [Wiki Publication and Embedding Contract](../../docs/deos/wiki-publication.contract.en.md). The [routing/projection contract](docs/routing.contract.en.md) owns graph identity, metapriority and native mdBook URLs. The [architecture map](docs/architecture.en.md) describes the implementation.

## Build and Validate

From the repository root:

```sh
npm run site:prepare --prefix wiki
npm run site:validate --prefix wiki
```

Use the repository-pinned Node and npm. Preparation installs the Wiki’s locked YAML dependency and the checksum-pinned official mdBook release declared in `mdbook.json`. Supported binary platforms are Linux x86_64/aarch64 and macOS x86_64/arm64. Local execution evidence is Linux-only. An existing renderer may be selected through `MDBOOK`; its reported version must match the pin. Builds do not fetch tools implicitly.

The owning Bash entrypoint is `../scripts/build-site.sh`; `--help` documents modes and side effects, and `--self-test` checks projection/tool contracts and CLI refusals without a download, renderer build or server. `--validate` adds Node-only checks of the shipped native search data and destinations on the built site; no browser, server or network is involved, and there is no automated browser journey.

Generated files stay under ignored `target/` and `dist/`. `dist/` is the independently deployable website artifact: copy its contents to a static document root. The build commands perform no deployment; only `publish-pages.sh --push` updates a branch. No source repository, wallet or chain is required to serve the artifact.

## Serving, Base Path and Publication

Every generated link is relative, and the reader resolves paths against the site root it computes from mdBook's `path_to_root`. One `dist/` therefore serves unchanged from a host root, a GitHub Pages project prefix or any static directory. A directory index delivered without its trailing slash (`/ru` instead of `/ru/`) restores its own base before any relative URL resolves. The site root is not a language chooser: it opens the base plane's landing in the reader's last language (English by default). A `#!layer=…` link opens that layer's contents instead.

`WIKI_BASE_PATH` (default `/`, for example `/deos/` for a Pages project site) is needed only by the native 404 page, which a static host serves at arbitrary depth. The output check refuses any same-origin destination outside it, and `--browser-test` serves the artifact beneath it. `WIKI_SITE_URL`, the absolute public URL of that prefix, adds canonical and `hreflang` links to every page, `sitemap.xml` and a `Sitemap:` line in `robots.txt`. Pages always get their `meta description`. 404 and table-of-contents fragments get `noindex, follow`. The sitemap carries no `lastmod`, because the wiki records no page freshness. `../scripts/publish-pages.sh` commits a validated `dist/` to the `gh-pages` branch. It runs as a dry run unless given `--push`; its `--help` and `--self-test` document the guarantees. The [Wiki domain entrypoint](../README.md) documents the CI and Pages routes.

## Current Capability Boundary

The compiler preserves the DEOS corpus, both locales, typed graph edges, explicit navigation order, provenance and authored Markdown. The composed graph stays behind the reader UI. It defines identity, order, layer membership and `graph.json` for agents, but the site shows no graph page and no per-article relation block. `wiki/layers/_meta/planes.json` owns optional reviewed membership overrides; one identity may belong to several semantic layers without duplicating its article. Only the DEOS plane is declared in this repository. Other projections remain a generic metadata-driven extension for an explicitly configured corpus or downstream fork; no placeholder product layer, domain or knowledge node is invented.

Each locale book opens with a Contents page that lists every article by plane and navigation section, with its summary. The sidebar is a foldable tree: the plane landing, then one node per navigation section, expanded by default. It is open by default from 768px unless the reader closed it. Every article carries its layer tags, for example `#deos`, linking to that layer's contents.

Each locale's `search-layers.json` describes fingerprinted native indices and its reader-context asset. Reader state lives in the fragment and never in the query string: `#!layer=deos` selects the declared DEOS view; absent or `all` selects the whole graph, and `#!search=…` is an open search panel. Other layer IDs are valid only when explicitly declared in that corpus. The same selection drives the native search index, contents list, sidebar tree and desktop/mobile/keyboard chapter transitions. The layer bar appears only when several layers are declared, a filter is active or an error needs recovery. Sparse section IDs and destinations remain intact. The default artifact exposes only DEOS. Empty and shared views are exercised through explicit synthetic classification fixtures, not published downstream knowledge.

Native mdBook supplies the reader, table of contents, heading navigation and Elasticlunr search UI. A post-render data step rebuilds its stored sections with Unicode token boundaries so Cyrillic search actually works; it retains the native engine, English stop-word/stemming steps and section destinations. This is Unicode matching and native prefix expansion, not Russian morphology. The theme extension adds a light/dark switch over mdBook's native themes (a cohesive slate dark palette), a menu-bar language switch that keeps the current page, a search panel that is centred in the viewport on a darker surface, floats over the article instead of pushing it down and closes on an outside click, safe external tabs, a copy tooltip kept inside its code block and an embedded-only current-page action. Print output is disabled. It does not render Markdown, parse a browser-side OKF bundle or communicate with a parent application.

EN/RU reading provides search with URL history, layer controls with synthetic empty and shared views, both chapter-button variants, keyboard order, locale identity, labeled cross-layer exits and responsive layouts. An open search panel is reproduced from its own URL entry, including empty text, and Back/Forward restore open and closed states. Unsupported or repeated selections and missing context data show explicit recovery rather than an unfiltered view under a false label. Native no-script reading remains unfiltered with a visible notice and static links to each landing and the contents.

Russian native navigation, shortcut help, search counts/teasers and clipboard feedback are localized without translating article text. These reading, navigation, search and layer flows form the DEOS EN/RU MVP. Their interaction behavior has no automated browser evidence: only the shared pure modules, native search data and structural output are machine-checked. Full keyboard/focus/zoom and accessibility qualification, optional embedding and production publication are not claimed and do not prolong the MVP pass. Root `BACKLOG.md` owns deferred follow-ups and higher-priority work; do not create a site-local roadmap.

## Ownership

`src/corpus.mjs` owns plane and page identity, consumed manifests and provenance reads. `src/layers.mjs` validates semantic membership and derives induced graph/linear views without changing the full graph. `src/context.mjs` owns URL selection, canonical chapter order, the site root and current page, and early native context bindings. `src/projection.mjs` derives that composed graph, relative-link Markdown, the tree SUMMARY, the site-root redirect and native theme partials. `scripts/compile.mjs` materializes those structural outputs.

`src/hash-state.mjs` owns the `#!` fragment grammar and its merge rules. `src/search.mjs` prepares Unicode-capable native data with pre-index section eligibility. `src/search-url.mjs` adapts the pinned reader's URL/history helpers without replacing its engine or rendering; it makes the open/closed search panel a fragment state and keeps the reading position when the panel opens. `src/locale.mjs` owns guarded native chrome/feedback localization and accessible-label corrections, preserving article bytes. `scripts/prepare-search.mjs` syntax-checks and fingerprints adapted native data/reader assets and rebinds them inside the owned stage. `scripts/prepare-meta.mjs` adds page descriptions, canonical/hreflang links, `robots.txt` and `sitemap.xml`. `scripts/check-output.mjs` checks local links, headings, assets, native search bindings, scope manifests, graph destinations and sitemap entries after mdBook renders them. `tests/native-search.test.mjs` checks shipped scope contents and a synthetic below-limit witness in the bundled engine.

`theme/layers.js` adds the language switch, filters the sidebar tree and contents, keeps state across in-page anchors and reloads on a layer change, and marks the current chapter without rendering articles. `theme/wiki.js` owns the light/dark switch, the search panel's dismissal and site chrome; they have no automated browser test. No-script chrome belongs to mdBook's native `header.hbs` partial, not searchable Markdown.

`src/frontmatter.mjs` is shared by the domain's corpus checks through its existing parser entrypoint. The reviewed-Markdown guard likewise has one implementation.
