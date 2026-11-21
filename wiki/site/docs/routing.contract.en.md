# Wiki mdBook Routing and Projection Contract

## Rendering Authority

mdBook is the only Markdown-to-HTML renderer. The Wiki domain compiles reviewed OKF metadata and Markdown into temporary book sources and `SUMMARY.md`; it must not implement another HTML/Markdown rendering engine. One composed graph supplies all locales and planes. Different language builds use the same renderer, graph identities and linear-order contract, not independent knowledge graphs.

## Graph and Linear View

Graph nodes are qualified as `{plane}/{canonical_page_id}`. Authored typed edges retain their exact direction and type. Linear adjacency, chapter groups and previous/next links never create semantic graph edges, and semantic cycles do not require removal or topological sorting.

The current metapriority is the explicit metadata order: plane declaration order, then the plane's index, then `_meta/navigation.json` sections/items in their authored array order. First occurrence selects a node's linear position; later navigation references do not duplicate the chapter. Every knowledge node must have an explicit position. Missing positions fail compilation rather than acquiring guessed priority from filenames, degree, dates or arbitrary numeric weights.

A titled navigation section is a group and must hold at least three distinct pages; compilation fails otherwise. A section without a title is an ungrouped run: its pages appear at plane level in authored order, with no sidebar node or contents heading. Ungrouping changes neither identity, route nor position. Prefer an existing titled group that already owns the topic and reaches three pages over keeping those pages as a flat tail, so the sidebar keeps one foldable engineering section; the ungrouped form stays correct for pages that no such group owns. The same minimum applies to storage directories under a plane: a directory with fewer than three distinct pages is not a group, so its pages live at the plane root. Because the directory is a route segment, moving a page there is a published source-path change and reconciles routes explicitly; order stays in navigation metadata, with the plane index always first.

The generated `SUMMARY.md` is a disposable linear projection. It is not a source of knowledge, graph structure or priority. Locale mirrors retain the same sequence of canonical identities. A different metapriority rule requires an explicit metadata-contract change, not a hidden sort in the renderer.

## Semantic Layer Membership

A storage plane owns a canonical identity and its source paths; a semantic layer selects knowledge for a reader. Declared plane IDs also name the available layers. The reserved virtual selection `all` means the complete composed graph and cannot name a storage plane. Membership never changes identity, provenance, locale, authored relations or metadata priority.

`wiki/layers/_meta/planes.json` may contain `memberships`, an object keyed by qualified node ID. An absent override assigns the node to its owning plane. An explicit value replaces that default with a nonempty set of declared layer IDs; it may assign one layer or several. Unknown nodes/layers, empty values and duplicates fail compilation. Membership is locale-independent, and its serialized order follows plane declaration order rather than creating a second priority.

Overrides are reviewed classification metadata, not inferred from words, tags, links or an assumed downstream dependency. Shared membership includes the same node in several views without copying the article or graph. An explicitly declared optional plane with no corpus or shared membership produces an empty view; no membership is invented to populate it. The repository's default declaration contains only DEOS, with no downstream placeholder plane.

A selected view retains matching nodes, authored edges whose two endpoints both match, and the matching subsequence of the canonical linear order. Filtering cannot add, reverse or reclassify an edge. The complete graph remains the authority; a selected view is only its projection.

## Client-Side State Model

All reader state lives in the URL fragment, never in the query string: a static artifact has no server to read a query, so nothing the reader changes may look like a server request. A stateful fragment starts with `#!` and carries only non-default keys: `layer`, `search`, `highlight` and `at` (the section). A plain fragment (`#section`) keeps native anchor semantics and means default state, and a bare page means default state without a section. Keys unknown to this contract are preserved across updates but never interpreted.

State writes never touch the query string, path or origin. Opening or restoring a page with a stateful fragment reproduces exactly the state it records: the layer view, the search overlay with its text, the highlight and the section. A stateful fragment cannot use the browser's own anchor lookup, so the reader scrolls to `at` after the article exists.

## Reader Layer State

`layer` is a single fragment key: absent or `all` selects the complete graph; one declared layer ID selects its view. Repeated or unknown values are unsupported, never silently interpreted as `all`. A layer view, the filtered sidebar and the prepared native search index are derived once per document, so a layer change loads a document rather than re-rendering in place; reading or restoring a URL does not write history. No parent-frame state or messaging is required.

Selecting a layer retains the current page when eligible; otherwise it opens the locale's knowledge index, preserving unrelated fragment context but clearing a stale section/highlight. A direct URL to an ineligible page shows an explicit notice with links to that layer's index and the same page in `all`, not an unrelated article or a silent redirect. Locale switches preserve canonical page identity and layer; translated heading IDs are not guessed.

Navigation, the contents list, the sidebar tree and previous/next follow the matching canonical linear subsequence. Article headings stay in their documents; the sidebar does not repeat them. The composed graph stays behind the reader UI: no graph page or relation table is rendered. Authored links to knowledge outside the view remain available as visibly labeled exits to `all`; same-document anchor clicks merge the section into the current fragment instead of replacing it. If the search panel is open, the outside-click dismissal also applies. Native search selects its prepared layer index before initialization and carries layer/context in result destinations. Print output is disabled.

Layer metadata is required for a filtered reader. Missing metadata or unsupported selections show a recoverable error and no unqualified filtered content/search. Without JavaScript, the native full-document reader remains available with an explicit unfiltered-reading notice; no active layer is claimed.

## Native Static Routes

A document URL is `/{locale}/{plane}/{book-path-with-.html}`. Book paths come from the locale manifest with the source file's own locale suffix dropped, because the language is already the first URL segment; only `.md` changes to `.html`. Example: source `overview/actor-system.ru.md` is served as `/ru/deos/overview/actor-system.html`. This native mdBook convention preserves authored document links without a custom Markdown link rewriter. Stable graph IDs resolve to those URLs through the generated graph manifest; they are not filesystem paths.

`/` is not a language chooser: it redirects to the default plane's landing in the reader's last language, English by default, such as `/en/deos/index.html`. With `layer` in the fragment state it opens that layer's contents instead. `/{locale}/` opens that language's Contents page. A future explicitly declared empty optional plane uses `/{locale}/{plane}/index.html`; that route is not emitted for undeclared projections. Physical HTML files serve every generated route; no application-server fallback is required. Native `404.html` is provided, while the selected host owns the HTTP status for unknown paths.

Every route above is relative to the site root wherever it is served. Generated links are relative, and the reader resolves graph routes against the root it derives from `path_to_root`. The same artifact therefore serves from a host root, a GitHub Pages project prefix or any directory, and a directory index without its trailing slash still resolves. The deployment base path, `WIKI_BASE_PATH` (default `/`, `/<repository>/` on a Pages project site), is recorded as the graph's `base_path`. It is used in rendered output only by the native 404 page, which a host serves at arbitrary depth, and by crawler metadata. A same-origin destination outside the prefix is a build failure. Storage paths and graph identities never depend on it.

Published source-path changes require explicit routing reconciliation. No aliases for unpublished draft route designs are implied. A published article whose directory stopped grouping moves to the plane root, and its former path keeps working through one redirect stub per locale that forwards to the current owner, preserves the fragment and stays out of the sitemap; `src/routes.mjs` is the only ledger of such paths.

A retired article is not redirected. Its content was merged into a page with a different reading purpose, so forwarding would promise a continuation that does not exist; a retired URL answers with the native bilingual 404. `_meta/consolidation.json` records each retirement with its `merged_into` successor, and search aliases are not redirects either. The current retirements are `getting-started/first-steps` (successor `overview/deos-framework`) and `getting-started/reading-paths` (successor `concepts/domain-map`), in both locales. Heading slugs, Markdown link rendering and ordinary chapter navigation belong to the pinned mdBook renderer. Search keeps the renderer's section identities, fields and destinations while using consistent Unicode word boundaries in data and queries.

## Search Semantics

The search backdrop blocks pointer interaction with the page behind it. Clicking the backdrop closes search through the same URL transition as the search toggle. Clicking inside the panel does not dismiss it. While search is open, the article and sidebar retain their scroll positions; only the search panel scrolls. Editing the search field, including clearing it with the native clear button, updates the results and fragment immediately. An empty field removes the results and their count but leaves the overlay open.

Full-text search must accept Cyrillic words without transliteration or a different Markdown renderer. Keep the bundled Elasticlunr engine, native result UI and section/heading destinations. Unicode token matching and native prefix expansion are not a promise of Russian morphological stemming. A direct search URL, keyboard result opening, empty results and refresh must obey the same behavior as opening the search control manually.

`search` and `highlight` are fragment keys of the client-side state model. `search` presence is the open search overlay, not only a query: opening records it with an empty value, typing replaces that value, and only an explicit close removes it. Restoring a URL or a history entry therefore reproduces the open panel, its text and its empty state exactly, and the article's own section survives every search transition. Treat search text as one opaque value through `URLSearchParams` over the fragment, including percent signs, literal plus signs, ampersands, equals signs and hash characters. Decode values once; malformed escapes follow the URL parser rather than throwing. Preserve unrelated fragment keys and their repeated values. Opening pushes one history entry; later edits replace it. Restoring history must not write history or reload the document. Dismissing a highlight removes only `highlight`, retaining search and the intended section. A search result destination carries the reader's layer and unrelated fragment context, drops the search text and records the target section.

Layer eligibility must be applied before the native result limit. Hiding ineligible rows after taking the first results can conceal eligible matches and is not a correct filtered search. Layer selection must preserve graph identity and the intended locale/page context; an empty selected view must never manufacture knowledge matches.

Each locale exposes `search-layers.json` with `schema_version: 1`, `locale` and `indexes` keyed by `all` plus every declared layer ID. Each entry contains a local fingerprinted native JSONP `asset` and its indexed `sections` count. `context_asset` names the local fingerprinted reader-context script; it binds the same graph selection and index before native search initializes. `all` retains native book search, including synthetic navigation; layer indices admit knowledge pages only. Retain original section reference IDs, field configuration, native options and destination mapping. These public classification views are not access control.

## Native Reader Locale

Each localized book presents native navigation, search feedback and help in its own language without translating authored article text or canonical control identifiers. Russian result counts use grammatical count forms and treat the query as opaque text. Canonical theme identities such as `Rust` and `Ayu` remain unchanged. Narrow adapters may correct pinned native labels and helper strings, but must preserve article bytes, native controls/search semantics and fail closed when their expected structure changes.

## Planes, Locales and Provenance

DEOS is the required and sole default plane. Additional planes are generic opt-in metadata, not a particular downstream brand. An explicitly declared optional plane with an absent content directory yields an honest empty landing and zero invented graph nodes; a present but malformed plane fails closed. The same composed graph supplies the EN/RU books; language switches use canonical page identity to find the real localized destination. Plane entry links open their landing rather than assuming equal IDs mean equivalent knowledge.

A page includes its authored Markdown body and compiled locale/plane links; the direction-preserving graph links, explicit status and recorded-source count stay structured metadata in `graph.json` and are never rendered as a page footer, because a closing status line invites a freshness claim the corpus does not make. Source-document provenance remains structured non-rendered metadata in `graph.json`, not a repository-path panel. Compilation adds no verification or freshness claims.

## Trust and Isolation

The reviewed-content guard, strict YAML parsing and provenance checks run before materialization. Native executable include/default preprocessors, automatic missing chapters and runnable Rust playgrounds are disabled. The corpus is reviewed repository content, not an untrusted Markdown service; these checks are not a generic sanitizer.

A diagram uses a stand-alone Markdown image line with localized alternative text and a lowercase, hyphenated sibling SVG filename. Write a text equivalent in the article. The corpus loader admits regular sibling files only and verifies the closed inert geometry subset in `src/trust.mjs`; scripts, styles, external references and XML declarations are outside that subset. The projection stages each referenced asset beside its locale chapter, and mdBook emits it beside the rendered HTML. Static validation fails if an image target is missing from either locale under the selected site prefix.

The generated website is static and needs no build tooling, wallet, RPC, parent window or authenticated session at runtime. Source compilation runs in the optional project Wiki profile, not in the framework build. Cross-origin iframe permissions, response CSP and production hosting remain separate integration contracts; successful static output does not certify them.
