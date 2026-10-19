# Wiki Publication and Embedding Contract

## Purpose and Authority

One knowledge graph. One website renderer. The independently deployable DEOS Wiki is the website rendering authority. If embedding is selected later, its host is only an iframe window into that website, never a second Wiki website implementation. This framework repository selects no downstream product identity or production hosting origin.

This contract supersedes any instruction to implement Wiki Markdown/OKF rendering, manifest parsing, navigation, search, localization, graph browsing or semantic-layer controls inside the web-client. Compatible content-compilation, strict OKF, provenance, localization, graph-validation and static-publication requirements remain in force. It does not authorize publication, DNS/account changes or deployment.

Project specifications and package-owned documentation retain their source authority. `/wiki` is the derived, reviewed knowledge bundle, not chain state or an independent protocol specification. The declared corpus contains reusable DEOS knowledge in English and Russian only. Generic metadata may support additional or mixed projections without duplicating the graph, but downstream instance/product content, names and hosting policy belong in the corresponding fork. Do not predeclare an empty downstream product plane in this repository.

The existing DEOS content is stored under `/wiki/deos/` as the base plane. Preserve that user-selected placement rather than recreating the former flat tree. The outer `/wiki` remains the layered knowledge container; directory boundaries must not create independent renderers or duplicate knowledge graphs. Each plane is validated at its declared strict OKF bundle root; the DEOS declaration is `/wiki/deos/index.md`. The outer `/wiki/index.md` is plain directory navigation, not a concept or a second bundle declaration. Plane metadata paths remain plane-relative, while repository provenance must resolve to the same authoritative sources after relocation. Content storage paths and public website routes are distinct contracts.

## Ownership

| Owner | Responsibility | Excluded responsibility |
| --- | --- | --- |
| `wiki-sync` workflow | Project knowledge → layered `/wiki` → mdBook website, including compiler/theme/tests under the Skill → publication preparation and authorized publication | A second renderer inside the web-client; unattended deployment authority |
| Standalone Wiki website | Markdown/OKF rendering, declared-layer navigation/filtering, EN/RU localization, search, graph traversal, provenance presentation, routes, deep links and responsive document layout | Wallets, chain access, authenticated application sessions; downstream product assumptions |
| Web-client | Trusted iframe configuration, mount/load/fallback/retry lifecycle and sizing within the existing tiling layout | Wiki content state, manifests, rendering, search, locale/layer controls or graph interpretation |
| Project-owned tooling | Core project and web-client build, iframe lifecycle/security tests and public development entrypoints | Importing or invoking the optional Skill website pipeline from project build/CI |

The optional website publication capability lives under `.agents/skills/wiki-sync/site/`, with its owning Bash pipeline in that Skill's `scripts/build-site.sh`. mdBook is the sole renderer; the Skill compiles structural metadata and Markdown inputs rather than an independent HTML frontend. There is no separate repository-root website workspace or root command alias into the Skill. The generated static artifact is independently deployable and requires no Skill runtime.

Removing `.agents/skills` must leave core project packages, the web-client, their builds, tests, CI and release validation unchanged. Project code must not import or invoke this optional website capability; the Skill may consume project-owned pure contracts and public development tools. Keep existing OKF/provenance/link-graph/localization audits intact. Any check required by core project acceptance needs a project owner, not a dependency on Skill internals.

Provenance presentation belongs to the website and must preserve the repository's epistemic rules: no invented freshness, verification or confidence. Source-document provenance remains structured non-rendered metadata, not a second repository-document navigation panel. Published articles contain established facts and current limitations, not implementation plans or backlog tasks.

## MVP and Priority Boundary

The DEOS-only EN/RU MVP requires working static reading, navigation/deep links, locale switching, search and graph traversal, with preserved identities, typed edges and provenance. Local generated-site browser evidence decides this boundary; no downstream projection, production domain or iframe is required.

Once those checks pass, continue with higher-priority work in `BACKLOG.md`. Full page-zoom/accessibility qualification, optional client embedding, renderer retirement and production publication remain separately scoped follow-ups. Do not turn them into an open-ended Wiki campaign or prerequisites for unrelated framework work. Retain the existing client reader until an explicitly prioritized replacement is verified.

## Static Website and URL Contract

The same static artifact must operate directly in a normal browser and inside a cross-origin iframe. Search, navigation, locale/layer selection and graph traversal must not require a backend, RPC, wallet, parent window or `postMessage` handshake. Embedded mode must not be a separate content implementation.

The [mdBook routing/projection contract](../../.agents/skills/wiki-sync/site/docs/routing.contract.en.md) owns the composed graph, metadata-defined linear branch, native locale/plane `.html` routes, headings and entrypoints. `SUMMARY.md` is a generated view, never the graph or its priority source. Static output or explicitly documented host behavior must serve every supported deep link without a development-server fallback. Knowledge identities and typed edges survive linearization and localization.

Locale and layer selections should be representable in stable paths or compatible query parameters when useful. Internal links, heading anchors, search results, graph links and locale switches must preserve the intended page/layer context under the chosen route mapping. Both standalone and embedded contexts must use that same mapping.

The site's responsive layout must support narrow widget containers, including document reading, navigation, filters and search. Its viewport is the iframe's available size, not the outer browser's desktop width. The site owns document scrolling and bounded overflow for wide tables/code; it must not require a full-screen desktop layout to remain usable.

## Iframe Configuration and Lifecycle

No production origin is selected by the framework. Before an optional embed is implemented, its deployment owner must explicitly configure the exact trusted HTTPS Wiki origin and intended frontend ancestors; missing configuration fails closed. Local integration may use explicitly configured development loopback origins. Configuration must be validated before assigning `src`: allow only explicitly trusted, normalized origins; reject credentials, unexpected schemes, protocol-relative destinations, deceptive host suffixes and any initial page URL resolving outside that origin set. A query string, saved widget state or arbitrary user input must never expand the trust set.

Production embeds use HTTPS. Plain HTTP development origins are restricted to explicitly configured loopback hosts in local development, with separate ports for the site and web-client. An HTTPS parent must not embed an HTTP child; use HTTPS locally when needed rather than disabling mixed-content protections. Review canonical redirects as well as the initial source, and do not let a trusted Wiki origin act as an open redirect to arbitrary framed content.

The widget may initialize one validated page/locale/layer URL. It must not parse manifests to construct a parallel navigation model or repeatedly synchronize the iframe URL with unrelated application state. Keep the iframe element, browsing context and `src` stable while the widget remains mounted; resizing, wallet/account updates and unrelated tiling-state changes must not recreate or reload it. Explicit retry, configured source changes and actual unmount are separate lifecycle transitions; preserving state across unmount is not implied.

Use the existing tiling host with a bounded full-width/full-height iframe, an accessible title and only minimal loading/fallback/open controls. The parent must not add a second scroll container around the document. Any host overflow adjustment must be narrowly scoped so unrelated widgets keep their existing scrolling behavior. The Wiki site supplies its own navigation, language selection, search and layer controls.

Provide loading feedback, a bounded load-wait indication, explicit retry and an always reachable external-open fallback. Cross-origin iframe `error` is unreliable, and `load` can fire for a browser error or blocked document; neither is a universal health signal. A timeout may report that loading could not be confirmed, not diagnose DNS, CSP or server failure. Keep fallback access available after `load`; never replace the iframe with an empty success state or use unauthenticated cross-origin fetch as invented availability evidence.

The parent cannot read the iframe's current cross-origin location. Its ordinary external-open action may open the validated initial URL, accurately labeled rather than claiming it is the current page. Prefer a website-owned action using its own current location for opening the actual current page in a new tab. Do not introduce a location-tracking protocol merely to relabel the initial URL.

## Embedding and Security Boundary

Review actual response headers, redirect destinations, TLS and browser behavior for the published Wiki and every intended frontend origin. The deployment configuration must enumerate those exact frontend origins; do not infer them from a domain name or widen them to arbitrary subdomains. Public inspection is evidence, not authorization to modify hosting.

A website response CSP `frame-ancestors` must allow the intended ancestors and no unnecessary others. This directive must come from response headers, not a meta tag. Review every ancestor when the frontend is itself embedded. `X-Frame-Options: DENY` or `SAMEORIGIN` is incompatible with the intended cross-origin embedding; obsolete `ALLOW-FROM` is not a substitute for a correct CSP. Review the frontend's own CSP `frame-src` separately. Successful CORS configuration is neither required for ordinary iframe navigation nor proof of framing permission.

Do not assume GitHub Pages accepts arbitrary response headers or treats a checked-in `_headers` file as configuration. If a required ancestry policy cannot be expressed by the chosen static host, retain the static artifact and use the smallest header-capable CDN/edge or static host that can enforce the policy at the explicitly configured Wiki origin. Record this as a hosting prerequisite; do not pretend that the static build or meta CSP configures it.

Choose and document the smallest iframe sandbox/Permissions Policy capabilities that the real site requires. Check scripts, same-origin behavior, storage, navigation and external links in a browser. If scripts and same-origin are both necessary, preserve a distinct site origin as the isolation boundary; do not grant a same-origin-with-parent sandbox escape. External links/new tabs must not gain an opener or navigate the parent implicitly; justify any popup/escape permissions against that requirement. Do not grant top-navigation, wallet, blockchain-provider, camera, microphone or unrelated application capabilities merely to make the site work.

No parent-to-frame DOM access or shared authenticated session is permitted. Normal links and URLs come first; `postMessage` is not required for basic Wiki functionality. Introduce it only for an evidenced integration need normal URLs/site-owned controls cannot satisfy. Any later protocol must validate exact `origin`, expected window/source, bounded message schema and destinations in both directions, use exact target origins, and expose no wallet, application secrets or signing capabilities.

## Legacy Renderer Retirement

Keep the legacy implementation until the standalone website and iframe replacement work. Then inspect all consumers before removing obsolete Markdown loaders/renderers, OKF/graph/navigation parsing, duplicated localization, content-level state, renderer-only trust/sanitization helpers, dependencies and tests from the web-client. Preserve general-purpose components and non-Wiki consumers.

Move reusable rendering/localization coverage to the standalone site rather than discarding its tested semantics. Replace retired widget rendering tests with iframe configuration, security, sizing and lifecycle tests. Preserve underlying OKF, provenance, link-graph and locale checks owned by `wiki-sync`; no renderer retirement authorizes their deletion. Update package locks, validation compositions, architecture boundaries and human entrypoints together.

## Optional Embedding Acceptance Evidence

Use a Skill-owned deterministic local integration test with the actual static site's built output served on one configured loopback origin and the public web-client development environment on another. The owning test route must start bounded servers, wait for readiness, run real browser assertions and always tear down its own processes. Core project build/CI must not invoke the Skill's pipeline to obtain that evidence. It must not require a live chain, wallet or public DNS. Do not substitute a dummy iframe fixture for the real-site compatibility test or disable web security, CSP or mixed-content protections.

| Check | Required evidence |
| --- | --- |
| Direct website | Built static pages work directly; deep-link navigation and refresh resolve on the selected static host |
| Same website embedded | The widget displays that same site's content from a different origin, not duplicated rendering or a same-origin proxy |
| Layers | Every declared selection yields identical content semantics standalone and embedded; test-only extension fixtures do not publish downstream knowledge |
| Site features | Navigation, localization, search, graph traversal, document links and heading anchors work inside the iframe |
| Narrow dimensions | Resize the real tile to narrow width/height; reading and site controls remain usable, without parent document scroll duplication |
| Stable lifecycle | Retain the same iframe element/browsing context and in-frame navigation after unrelated web-client updates and resizing; no new document request |
| Unavailable origin | A controlled unavailable origin produces bounded loading/fallback/retry behavior without falsely reporting confirmed content readiness |
| Security | Validated origins, denied untrusted initial URLs, distinct origins, minimum sandbox/permissions, safe external links and compatible actual headers; no direct cross-origin DOM access |
| Single renderer | Consumer/dependency inspection and focused regression show no remaining Wiki content renderer, manifest navigation or locale/layer/search implementation in the client |
| Static independence | Site builds and serves independently without web-client, Skill runtime, backend, wallet or authenticated session |

Local browser evidence does not certify production DNS, TLS, headers or ancestry configuration. Publication acceptance additionally requires actual standalone and embedded browser checks through the configured production origins after separately authorized deployment. Record each missing external gate explicitly rather than presenting all ten checks as passed from a build, source inspection or an iframe `load` event.
