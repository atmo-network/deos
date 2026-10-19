# Wiki mdBook Website

This is the Wiki Sync Skill's optional knowledge-to-website compiler, theme and test surface. It reads the layered repository corpus under `/wiki`, produces one composed graph, and prepares a metadata-ordered Markdown book for each locale. **mdBook alone renders HTML.** There is no independent application renderer here and no dependency from the web-client or project CI into this directory.

The public integration boundary is the [Wiki Publication and Embedding Contract](../../../../docs/deos/wiki-publication.contract.en.md). The [routing/projection contract](docs/routing.contract.en.md) owns graph identity, metapriority and native mdBook URLs. The [architecture map](docs/architecture.en.md) describes the implementation.

## Build and Validate

From the repository root:

```sh
npm run site:prepare --prefix .agents/skills/wiki-sync
npm run site:validate --prefix .agents/skills/wiki-sync
npm run site:browser --prefix .agents/skills/wiki-sync
```

Use Node 22.22 or newer. Preparation installs the Skill's locked YAML dependency and the checksum-pinned official mdBook release declared in `mdbook.json`. Supported binary platforms are Linux x86_64/aarch64 and macOS x86_64/arm64. Local execution evidence is Linux-only. An existing renderer may be selected through `MDBOOK`; its reported version must match the pin. Builds do not fetch tools implicitly.

The owning Bash entrypoint is `../scripts/build-site.sh`; `--help` documents modes and side effects, and `--self-test` checks projection/tool contracts and CLI refusals without a download, renderer build or server. Browser journeys require an installed Chrome (`CHROME_BIN` may select it), serve the real built artifact on a temporary loopback port and drive isolated profiles through an owned CDP pipe. They exercise actual search, keyboard, graph, locale and fragment navigation, then tear down their own browser process groups and server. They do not inject replacement pages, disable browser security or certify iframe behavior.

Generated files stay under ignored `target/` and `dist/`. Browser screenshots under `target/browser-smoke/` include EN/RU reading (`en.png`, `ru.png`), word search (`en-search.png`, `ru-search.png`), punctuation search (`en-search-url.png`, `ru-search-url.png`), layer views (`*-layer-empty.png`, `*-layer-shared.png`) and native reader controls (`*-reader-help.png`, `*-reader-search.png`, `*-reader-large-text.png`). Empty/shared-view screenshots use test-only classification of DEOS documents, not published downstream knowledge. `dist/` is the independently deployable website artifact: copy its contents to a static document root. No deployment is performed by these commands. No Skill, source repository, wallet or chain is required to serve the artifact.

## Current Capability Boundary

The compiler preserves the DEOS corpus, both locales, typed graph edges, explicit navigation order, provenance and authored Markdown. It generates locale/plane entry links, a graph chapter and per-page graph transitions. `wiki/_meta/planes.json` owns optional reviewed membership overrides; one identity may belong to several semantic layers without duplicating its article. Only the DEOS plane is declared in this repository. Other projections remain a generic metadata-driven extension for an explicitly configured corpus or downstream fork; no placeholder product layer, domain or knowledge node is invented.

Each locale's `search-layers.json` describes fingerprinted native indices and its reader-context asset. `?layer=deos` selects the declared DEOS view; absent or `all` selects the whole graph. Other layer IDs are valid only when explicitly declared in that corpus. The same selection drives the native search index, page list, sidebar, graph rows and desktop/mobile/keyboard chapter transitions. Sparse section IDs and destinations remain intact. The default artifact exposes only DEOS. Empty and shared views are exercised through explicit synthetic classification fixtures, not published downstream knowledge.

Native mdBook supplies the reader, table of contents, heading navigation, themes and Elasticlunr search UI. A post-render data step rebuilds its stored sections with Unicode token boundaries so Cyrillic search actually works; it retains the native engine, English stop-word/stemming steps and section destinations. This is Unicode matching and native prefix expansion, not Russian morphology. The small theme extension controls readable overflow, safe external tabs and an action that opens the website's actual current page. It does not render Markdown, parse a browser-side OKF bundle or communicate with a parent application.

Direct EN/RU journeys cover search/URL history, actual layer controls, synthetic empty/shared views, induced graph rows, both chapter-button variants, keyboard order, locale identity, labeled cross-layer exits and 360/768/1280px layouts. Unsupported/repeated selections and missing context data show explicit recovery rather than an unfiltered view under a false label. Native no-script reading remains unfiltered with a visible notice.

Russian native navigation, theme choices, shortcut help, search counts/teasers and clipboard feedback are localized without translating canonical theme identities or article text. Reader-control journeys exercise native theme persistence, help shortcuts, sidebar/search state and clipboard success/controlled refusal. A narrow-layout check doubles Chrome's default text size; this is text-resizing evidence, not browser page-zoom acceptance. These implemented reading/navigation/search/graph flows form the DEOS EN/RU MVP. Full keyboard/focus/zoom and accessibility qualification, optional embedding and production publication are not claimed and do not prolong the MVP pass. Direct-site evidence is not embedded or production evidence. Root `BACKLOG.md` owns deferred follow-ups and higher-priority work; do not create a site-local roadmap.

## Ownership

`src/corpus.mjs` owns plane and page identity, consumed manifests and provenance reads. `src/layers.mjs` validates semantic membership and derives induced graph/linear views without changing the full graph. `src/context.mjs` owns URL selection, canonical chapter order and early native context bindings. `src/projection.mjs` derives that composed graph, Markdown/SUMMARY and native theme inputs. `scripts/compile.mjs` materializes those structural outputs.

`src/search.mjs` prepares Unicode-capable native data with pre-index section eligibility. `src/search-url.mjs` adapts the pinned reader's URL/history helpers without replacing its engine or rendering. `src/locale.mjs` owns guarded native chrome/feedback localization and accessible-label corrections, preserving article bytes. `scripts/prepare-search.mjs` syntax-checks and fingerprints adapted native data/reader assets and rebinds them inside the owned stage. `scripts/check-output.mjs` checks local links, headings, assets, native search bindings, scope manifests and graph destinations after mdBook renders them. `test/native-search.test.mjs` checks shipped scope contents and a synthetic below-limit witness in the bundled engine.

`theme/layers.js` adapts native DOM controls without rendering articles; `test/layer-browser.test.mjs` exercises production empty/error views and isolated mixed-membership data against the real built documents. No-script chrome belongs to mdBook's native `header.hbs` partial, not searchable Markdown.

`src/frontmatter.mjs` is shared by this Skill's corpus checks through its existing parser entrypoint. The reviewed-Markdown guard likewise has one implementation. `src/relations.mjs` temporarily consumes the project's pure relation-vocabulary contract while the legacy client renderer remains installed; it initializes no web-client or wallet runtime. Renderer retirement must preserve that vocabulary before removing its old owner.
