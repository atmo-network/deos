# Wiki mdBook Routing and Projection Contract

## Rendering Authority

mdBook is the only Markdown-to-HTML renderer. The Wiki Sync Skill compiles reviewed OKF metadata and Markdown into temporary book sources and `SUMMARY.md`; it must not implement another HTML/Markdown rendering engine. One composed graph supplies all locales and planes. Different language builds use the same renderer, graph identities and linear-order contract, not independent knowledge graphs.

## Graph and Linear View

Graph nodes are qualified as `{plane}/{canonical_page_id}`. Authored typed edges retain their exact direction and type. Linear adjacency, chapter groups and previous/next links never create semantic graph edges, and semantic cycles do not require removal or topological sorting.

The current metapriority is the explicit metadata order: plane declaration order, then the plane's index, then `_meta/navigation.json` sections/items in their authored array order. First occurrence selects a node's linear position; later navigation references do not duplicate the chapter. Every knowledge node must have an explicit position. Missing positions fail compilation rather than acquiring guessed priority from filenames, degree, dates or arbitrary numeric weights.

The generated `SUMMARY.md` is a disposable linear projection. It is not a source of knowledge, graph structure or priority. Locale mirrors retain the same sequence of canonical identities. A different metapriority rule requires an explicit metadata-contract change, not a hidden sort in the renderer.

## Semantic Layer Membership

A storage plane owns a canonical identity and its source paths; a semantic layer selects knowledge for a reader. Declared plane IDs also name the available layers. The reserved virtual selection `all` means the complete composed graph and cannot name a storage plane. Membership never changes identity, provenance, locale, authored relations or metadata priority.

`wiki/_meta/planes.json` may contain `memberships`, an object keyed by qualified node ID. An absent override assigns the node to its owning plane. An explicit value replaces that default with a nonempty set of declared layer IDs; it may assign one layer or several. Unknown nodes/layers, empty values and duplicates fail compilation. Membership is locale-independent, and its serialized order follows plane declaration order rather than creating a second priority.

Overrides are reviewed classification metadata, not inferred from words, tags, links or an assumed downstream dependency. Shared membership includes the same node in several views without copying the article or graph. An explicitly declared optional plane with no corpus or shared membership produces an empty view; no membership is invented to populate it. The repository's default declaration contains only DEOS, with no downstream placeholder plane.

A selected view retains matching nodes, authored edges whose two endpoints both match, and the matching subsequence of the canonical linear order. Filtering cannot add, reverse or reclassify an edge. The complete graph remains the authority; a selected view is only its projection.

## Reader Layer State

`layer` is a single query parameter: absent or `all` selects the complete graph; one declared layer ID selects its view. Repeated or unknown values are unsupported, never silently interpreted as `all`. Layer links perform ordinary document navigation; reading/restoring a URL does not write history. No parent-frame state or messaging is required.

Selecting a layer retains the current page when eligible; otherwise it opens the locale's knowledge index, preserving search/context parameters but clearing a stale heading/highlight. A direct URL to an ineligible page shows an explicit notice with links to that layer's index and the same page in `all`, not an unrelated article or a silent redirect. Locale switches preserve canonical page identity and layer; translated heading IDs are not guessed.

Navigation, page lists and previous/next follow the matching canonical linear subsequence. Graph tables show only authored edges whose two endpoints match. Authored links to knowledge outside the view remain available as visibly labeled exits to `all`; same-document fragments retain current query state. Native search selects its prepared layer index before initialization and carries layer/context in result destinations. Whole-book print is available only in `all`, not mislabeled as a filtered print view.

Layer metadata is required for a filtered reader. Missing metadata or unsupported selections show a recoverable error and no unqualified filtered content/search. Without JavaScript, the native full-document reader remains available with an explicit unfiltered-reading notice; no active layer is claimed.

## Native Static Routes

A document URL is `/{locale}/{plane}/{localized-source-path-with-.html}`. Source-relative paths come from the locale manifest; only `.md` changes to `.html`. Example: `/ru/deos/overview/actor-system.ru.html`. This native mdBook convention preserves authored document links without a custom Markdown link rewriter. Stable graph IDs resolve to those URLs through the generated graph manifest; they are not filesystem paths.

`/` selects a language. `/{locale}/` opens that language's composed book. The DEOS plane landing uses its localized index, such as `/en/deos/index.en.html`. A future explicitly declared empty optional plane uses `/{locale}/{plane}/index.html`; that route is not emitted for undeclared projections. `/{locale}/graph.html` is a generated graph-navigation view, not an authored knowledge node. Physical HTML files serve every generated route; no application-server fallback is required. Native `404.html` is provided, while the selected host owns the HTTP status for unknown paths.

Published source-path changes require explicit routing reconciliation. No aliases for unpublished draft route designs are implied. Heading slugs, Markdown link rendering and ordinary chapter navigation belong to the pinned mdBook renderer. Search keeps the renderer's section identities, fields and destinations while using consistent Unicode word boundaries in data and queries.

## Search Semantics

Full-text search must accept Cyrillic words without transliteration or a different Markdown renderer. Keep the bundled Elasticlunr engine, native result UI and section/heading destinations. Unicode token matching and native prefix expansion are not a promise of Russian morphological stemming. A direct search URL, keyboard result opening, empty results and refresh must obey the same behavior as opening the search control manually.

`search` and `highlight` are owned query parameters. Treat search text as one opaque value through `URL`/`URLSearchParams`, including percent signs, literal plus signs, ampersands, equals signs and hash characters. Decode values once; malformed escapes follow the browser URL parser rather than throwing. Preserve unrelated parameters and their repeated values. Typing a new search may push one history entry; later edits replace it. Restoring history must not write history or reload the document. Dismissing a highlight removes only `highlight`, retaining search and the intended heading.

Layer eligibility must be applied before the native result limit. Hiding ineligible rows after taking the first results can conceal eligible matches and is not a correct filtered search. Layer selection must preserve graph identity and the intended locale/page context; an empty selected view must never manufacture knowledge matches.

Each locale exposes `search-layers.json` with `schema_version: 1`, `locale` and `indexes` keyed by `all` plus every declared layer ID. Each entry contains a local fingerprinted native JSONP `asset` and its indexed `sections` count. `context_asset` names the local fingerprinted reader-context script; it binds the same graph selection and index before native search initializes. `all` retains native book search, including synthetic navigation; layer indices admit knowledge pages only. Retain original section reference IDs, field configuration, native options and destination mapping. These public classification views are not access control.

## Native Reader Locale

Each localized book presents native navigation, search feedback and help in its own language without translating authored article text or canonical control identifiers. Russian result counts use grammatical count forms and treat the query as opaque text. Canonical theme identities such as `Rust` and `Ayu` remain unchanged. Narrow adapters may correct pinned native labels and helper strings, but must preserve article bytes, native controls/search semantics and fail closed when their expected structure changes.

## Planes, Locales and Provenance

DEOS is the required and sole default plane. Additional planes are generic opt-in metadata, not a particular downstream brand. An explicitly declared optional plane with an absent content directory yields an honest empty landing and zero invented graph nodes; a present but malformed plane fails closed. The same composed graph supplies the EN/RU books; language switches use canonical page identity to find the real localized destination. Plane entry links open their landing rather than assuming equal IDs mean equivalent knowledge.

A page includes its authored Markdown body, compiled locale/plane links, explicit status, recorded-source count and direction-preserving graph links. Source-document provenance remains structured non-rendered metadata in `graph.json`, not a repository-path panel. Compilation adds no verification or freshness claims.

## Trust and Isolation

The reviewed-content guard, strict YAML parsing and provenance checks run before materialization. Native executable include/default preprocessors, automatic missing chapters and runnable Rust playgrounds are disabled. The corpus is reviewed repository content, not an untrusted Markdown service; these checks are not a generic sanitizer.

The generated website is static and needs no Skill files, wallet, RPC, parent window or authenticated session at runtime. Source compilation is an optional capability under this Skill, not a project build/CI dependency. Cross-origin iframe permissions, response CSP and production hosting remain separate integration contracts; successful static output does not certify them.
