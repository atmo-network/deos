# Vendored Mermaid

`mermaid.esm.min.js`, `chunks/mermaid.esm.min/**` and `LICENSE` come from the published `mermaid` npm
package, version `12.3.0` (tarball SHA256 `be0684e8d20bfc874df12c0f04927a5c482f57f2200eec54cca52a4583fd4040`),
`dist/` directory only. Mermaid is MIT licensed; `LICENSE` is copied verbatim.

## One deliberate change

Upstream ships the split build as `.mjs`, and a static host is free to serve an unknown extension with
`application/octet-stream`, which makes a module import fail before any code runs. Every file was
therefore renamed to `.js` and every relative `./chunks/... .mjs` specifier inside them was rewritten to
`.js`. No code, name or version string was otherwise edited. Reproduce it by renaming the files and
rewriting the `./...mjs"` specifiers, nothing else.

Source maps (`.map`) are not vendored: they are developer artefacts, they doubled the directory size, and their embedded sources are not part of the shipped behaviour.

## What is published and what is not

`build-site.sh` publishes the distribution **once at the site root** under `theme/mermaid/`, because
the renderer resolves it from there; copying it into each locale book would duplicate 25 MB per locale.
Only the files a rendered diagram type actually imports are fetched by a reader: a flowchart page
fetches the 31 KB entry plus its transitive chunks, roughly 0.75 MB raw, not the whole 5.2 MB
directory, and every file is immutable and cacheable.

## Why the browser and not the build

Mermaid measures real text metrics, so it needs a layout engine. Rendering it at build time would put
a headless browser into the build and the published artifact would carry baked colours, one variant per
theme. Rendering in the reader's browser keeps authored Markdown authoritative, follows the live theme,
and costs nothing on a page without a diagram — `wiki-mermaid.js` is listed only in a book that contains
one, and a failed render leaves the diagram source visible as readable text.

## Custom infographics

A diagram Mermaid cannot express stays an authored sibling `.svg`, validated by `src/trust.mjs` against
its closed geometry subset. `renderDiagrams` admits only the syntaxes declared in `src/mermaid.mjs`, so
an unvendored diagram type fails the build rather than a reader's browser.

## Updating

Replace the entry and `chunks/mermaid.esm.min/`, repeat the rename and specifier rewrite, refresh the
version and tarball digest here, then rebuild and confirm that a page with a diagram still renders in
both themes. `check-output.mjs` fails when a diagram page loses the loader or the distribution.
