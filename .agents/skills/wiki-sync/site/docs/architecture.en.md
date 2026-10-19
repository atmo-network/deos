# Wiki mdBook Implementation

## Boundary

This optional website pipeline lives entirely under the Wiki Sync Skill. Its Bash entrypoint composes local preparation, structural compilation, the pinned mdBook executable, validation and optional bounded browser smoke. Project packages, runtime, CI and release validation do not invoke it. The resulting static artifact is independent of the Skill at runtime.

## Data Flow

`wiki/_meta/planes.json` declares plane order and required/optional roots. `src/corpus.mjs` loads each declared bundle, locale/navigation/graph manifests, canonical page identities and structured provenance. It rejects missing required planes, malformed present planes, missing mirrors, identity/path drift and unsafe reviewed Markdown. The existing strict OKF method checks remain separately available through the Skill.

`src/projection.mjs::unifiedGraph` qualifies nodes by plane, preserves every authored typed edge and records localized URLs/provenance. `linearOrder` uses the plane index and authored section/item order, deduplicating references by first encounter. A graph page without a position fails; graph degree and edge iteration do not affect chapter order. `test/projection.test.mjs` reverses graph order and metadata order independently to falsify accidental coupling.

`projectBooks` produces Markdown bodies, SUMMARY files, TOML configuration and one `projection.json`. It retains authored bodies and source-relative paths. The generated EN/RU books and language landing share the same mdBook engine and graph; synthetic navigation/empty-state pages are not added as knowledge nodes. Authored graph cycles remain valid.

## Semantic Layer Views

`src/corpus.mjs` carries the container's optional `memberships` overrides into composition. `src/layers.mjs::assignMemberships` validates known node/layer IDs, nonempty unique sets and the reserved virtual `all` name, then serializes each node's membership in declaration order. The default is its storage plane; explicit overrides replace that default. `graph.json` contains declared `layers` and each node's `layers`, independent of locale and with unchanged identity/URLs/provenance.

`selectLayer` returns an induced view with the matching linear subsequence, retaining the original node objects and authored edge directions/types. The complete graph remains intact. Projection regressions cover ownership defaults, mixed and exclusive overrides, invalid metadata, a synthetic empty optional plane, cycles and order conservation. These same views supply the reader context; membership is never inferred again in the theme.

## Reader Layer Context

`src/context.mjs` admits one exact `layer` query value, composes the filtered native chapter sequence and serializes a public graph subset with the shared pure view functions. This context omits provenance copies; `graph.json` retains the full structured record. `prepare-search.mjs` fingerprints the locale context and binds it in the native head after root variables but before search initialization. The language entry book uses the same context through a normal additional script.

The context selects its native index synchronously. A required-context marker gates article display until the controller settles; unsupported selection or unavailable metadata produces an explicit error and recovery, never an unqualified `all` fallback. The native search adapter disables initialization safely in that state, including the focus contract used by native chapter keys. Reading a URL never writes history; selecting a layer follows a normal link.

`theme/layers.js` owns accessible layer/locale links and adapts native sidebar items, compiler-marked page lists and induced graph rows. It rewrites both `.nav-chapters` and `.mobile-nav-chapters`; native arrow keys consume the corrected wide anchors. A visual mobile witness exposed the separate footer links, so browser tests assert both variants. Authored tables and lists outside generated graph/index regions are not filtered or parsed as metadata.

Eligible links retain the layer. Authored out-of-layer links explicitly expose an `all` exit without changing the article's claims. Ineligible direct URLs retain their location and show a notice with recovery; selection from an ineligible page opens the locale index. Locale links use canonical identity, preserve layer/search context and clear untranslated heading/highlight state. Whole-book print is unavailable in a filtered view rather than misrepresented as filtered output.

The no-script baseline is the native full reader. Its explicit unfiltered notice is a generated `header.hbs` theme partial, outside searchable Markdown. This avoids indexing control labels/fallback instructions as article knowledge. mdBook renders that partial and every article; the theme neither renders Markdown nor supplies a second frontend.

## Rendering and Publication Boundary

`mdbook.json` pins the release and official archive SHA-256 values by platform. `scripts/toolchain.mjs` performs exact structural selection; the Skill's [`build-site.sh`](../../scripts/build-site.sh) owns download, checksum verification and executable-version checks. Unsupported platforms fail rather than selecting approximate binaries. Preparation and build are distinct modes.

The Bash build renders into a unique owned staging directory, invokes `check-output.mjs` before replacing `dist`, and restores the prior artifact on a failed replacement. Failed compilation/rendering never promotes a partial website. Temporary books, renderer binaries and screenshots stay under ignored `target`; no source content is deleted or deployed.

mdBook owns HTML, heading IDs, navigation and search assets. Executable default preprocessors, automatic missing-chapter creation and runnable playgrounds are disabled in generated TOML. The structural compiler emits no HTML files. Native output validation checks local document/asset existence, heading fragments, literal native search-asset bindings and every published graph URL. It is not a Markdown renderer or an untrusted-HTML sanitizer.

## Native Unicode Search

The pinned renderer's [search indexer](https://github.com/rust-lang/mdBook/blob/v0.5.4/crates/mdbook-html/src/html_handlebars/search.rs) uses the default English Elasticlunr pipeline. Its `elasticlunr-rs` dependency's [trimmer](https://docs.rs/crate/elasticlunr-rs/3.0.2/source/src/lang/en.rs) admits ASCII digits/letters and underscores, dropping Cyrillic; the bundled browser trimmer is likewise ASCII-oriented. Setting the HTML language to `ru` does not prove usable Russian search. The real-browser `наблюдения` witness exposed this failure.

`src/search.mjs` reuses the renderer's stored section text, field configuration, reference IDs and destination list. It rebuilds data through that same bundled Elasticlunr JavaScript library, replacing only token-boundary trimming with a registered Unicode step. Native stop-word filtering, English stemming, search options and section URLs remain. The build-time first step preserves the native 80-UTF-8-byte token admission bound without replacing the tokenizer object, which owns separator state. No second search engine, forked renderer or new dependency is introduced. Remove this adapter when an upgraded pinned renderer passes the same Cyrillic/browser witnesses natively.

The final JSONP asset registers the identical Unicode boundary function before native `Index.load` runs, including direct search-URL initialization. `scripts/prepare-search.mjs` fingerprints that asset and changes only the native head's search-asset assignment, not authored document HTML. It removes the superseded index. The script admits only regular generated inputs inside the owned stage. `node:vm` evaluates trusted pinned-renderer assets only; it is not a sandbox for remote scripts or a Markdown execution facility.

`test/projection.test.mjs` checks ASCII boundary parity, Cyrillic/Unicode preservation, source-text-preserving rebinding and broken search-asset rejection. `test/browser.test.mjs` exercises the actual rebuilt indices and native UI in both locales, rather than inferring search support from an index file or `lang` attribute.

## Scoped Native Search Data

`prepare-search.mjs` consumes the owned projection and an explicit locale. It prepares one native JSONP asset for `all` and each declared layer, recorded in that locale's `search-layers.json` as asset name and stored-section count. `all` preserves the complete native book index; layer indices include only matching knowledge pages, not synthetic navigation. The manifest also binds its fingerprinted `context_asset`. Output validation checks manifest coverage, same-locale asset names, section-count shape and local file existence.

`src/search.mjs::nativeSections` matches original section destinations to eligible localized page paths before `index.addDoc`. Ranking and the native result cap therefore operate only on eligible documents. Reference IDs remain sparse and destination lists/options unchanged; no hidden-row post-filter or second engine exists. An empty selected view has zero indexed sections. The default artifact declares only DEOS; optional-plane construction is tested with synthetic metadata. The early context binds the selected scope before native search starts. Its guarded native fallback uses that same selection even when the ordinary index binding is absent.

`test/native-search.test.mjs` loads shipped indices through the renderer's actual bundled engine and checks exact section admission, stored text, references, options and destinations. Its isolated synthetic mixed-member witness places an eligible result below the native unfiltered cap, then proves scoped admission retains it. It does not insert fixture content into `/wiki` or measure production ranking/performance. EN/RU browser journeys use actual URL selection and native lazy loading. `layer-browser.test.mjs` overlays only test classification/context/index data for a mixed-membership witness; HTML, controller code and native assets remain the real built site. It does not publish fixture membership or downstream content. Browser classification fixtures reuse actual DEOS documents; optional empty-plane Markdown construction is checked separately in projection tests.

## Native Search URL State

`src/search-url.mjs` owns pure WHATWG URL transitions and their guarded integration into the pinned searcher. It replaces only the manual URL codec/restoration/writer spans and resets the native query cache when results are cleared. Opaque punctuation remains one `search` value; repeated unrelated parameters survive. Reads decode once, tolerate malformed escapes and never push or replace browser history.

The reader restores empty/nonempty search state on Back/Forward without reloading the document or growing history. Highlight dismissal uses one delegated content-root listener because native highlighting can replace transient mark nodes. One cancellable fade timer cannot clear a subsequently restored highlight. Dismissal removes only `highlight`, preserving active search and the heading.

`prepare-search.mjs` syntax-checks and fingerprints the adapted reader, rebinds only its script resources and the native index assignment, and updates its fallback to the same Unicode index. Both superseded assets are removed. No authored document text is rewritten. Structural drift in the pinned helper spans fails the build rather than silently applying a partial patch. Remove this adapter when an upgraded native reader passes the same URL/history regressions without it.

## Native Reader Localization

`src/locale.mjs` adapts the pinned native reader rather than replacing its controls. `localizeReaderChrome` isolates the single `<main>` article slice and preserves it byte-for-byte, localizing only surrounding Russian navigation, themes, help and search chrome. Native standalone TOC documents have no article chrome. Boundary/string-count drift fails the build. Both locales receive the native search-description ID correction and a titled no-script TOC iframe.

`localizeSearchScript` retains the native formatter interface and plain-text insertion, using Russian plural rules for counts and a localized teaser label. `localizeBookScript` translates hidden-line and clipboard labels/feedback while preserving their state comparisons. Both locales take hidden-line accessible labels from the owning button, not a nested SVG event target. Theme keys and canonical `Rust`/`Ayu` names remain unchanged.

`prepare-search.mjs` syntax-checks adapted native scripts before content fingerprinting, rebinds script resources and removes the superseded searcher/book assets inside the owned stage. It checks aggregate index/context/book binding totals; those totals alone do not prove per-document cardinality. `test/locale.test.mjs` covers plural families, opaque terms, article preservation and native-contract refusal. No authored concept page is translated by this adapter. Remove it when a pinned native reader supplies equivalent localization/accessibility behavior under the same regressions.

## Shared Contracts

`src/frontmatter.mjs` owns strict YAML parsing, duplicate keys, alias bounds and serialization. The Skill's existing `scripts/okf-frontmatter.mjs` delegates to it. `src/trust.mjs` owns the reviewed-content guard, reused by the compiler and `scripts/check-trust.mjs`; the Skill's trust route invokes that same check rather than duplicating it.

The site consumes the project's pure typed relation-vocabulary contract through `src/relations.mjs`. This is a Skill-to-project dependency only; the client formatter does not import the Skill. It preserves the established direction-explicit Russian labels without initializing client stores or wallet/chain adapters. The client renderer remains transitional until verified iframe replacement.

## Browser Realization and Evidence

`theme/wiki.css` is a narrow mdBook reading/overflow extension. `theme/wiki.js` adds no content renderer: it handles external tabs and reads the site's own location for the current-page action. It refreshes the destination on focus/pointer/activation as well as navigation events, because native search changes the URL through `pushState` without a navigation event. It neither patches global history nor reads/messages a parent frame. Its menu anchor is bound to the pinned renderer's actual DOM and covered by browser smoke.

`test/browser.test.mjs` serves actual `dist` on loopback. `test/chrome.mjs` owns an isolated Chrome process group and CDP pipe with bounded requests and teardown. EN wide/RU narrow journeys exercise native text search, Cyrillic words, direct search URLs, keyboard result opening, absent queries, section refresh, directed graph links, locale identity, heading anchors and safe current-page actions. They inspect failed assets/runtime errors and close browser profiles and server. Screenshots support direct visual inspection; no security bypass flags or replacement pages are used.

URL journeys cover punctuation/escaping, refresh, clear/retype, read-only Back/Forward, highlight dismissal and native fallback loading. Layer journeys additionally cover empty and shared views, early scoped search, language entry, induced graph rows, both navigation variants, explicit exits, invalid/missing metadata and no-script reading.

`test/reader-browser.test.mjs` exercises actual native themes and persistence, help keyboard shortcuts, sidebar state, localized search metrics/teasers and clipboard success plus controlled refusal. It checks real code pages at narrow width after CDP `Page.setFontSizes` doubles the default text size, including the native theme menu. This qualifies text resizing, not browser page zoom, complete keyboard/focus behavior, all control geometry or full accessibility. This is sufficient for the stated DEOS EN/RU MVP, not exhaustive accessibility qualification. Further reader polish and optional embedding/publication do not delay higher-priority framework work. This evidence cannot close iframe, live-hosting or production acceptance.
