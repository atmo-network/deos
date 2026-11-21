# Vendored KaTeX

`katex.min.css`, `katex.mjs`, `fonts/*.woff2` and `LICENSE` come from the published `katex` npm
package, version `0.19.1` (tarball SHA256 `24da5a599eb2fcede2e270b99a6ff48bca816beecc95feeb84e8cccbd1bb34fd`),
`dist/` directory only. KaTeX is MIT licensed; `LICENSE` is copied verbatim.

## One deliberate change

Upstream `@font-face` rules ship each face three times (`.woff2`, `.woff` and `.ttf`). Only the
`.woff2` files are vendored, so every `src:` list was reduced to its `.woff2` entry. Nothing else in
the file changed: no rule, metric, class or version string was edited. Reproduce the reduction when
updating by keeping, per `src:` list, only `url(fonts/<name>.woff2) format('woff2')`.

## What is published and what is not

`katex.min.css` and the fonts are published into a book that typesets notation, and a browser fetches
only the faces the displayed formulas reference. `katex.mjs` is build-time only: `src/math.mjs`
typesets `$...$` and `$$...$$` during projection, so no KaTeX JavaScript reaches a reader and no
formula is laid out in the browser.

## Updating

Replace the four assets from the new `dist/`, repeat the `src:` reduction, refresh the version and
tarball digest here, then rebuild and check that a page with display math still renders its fractions
and large operators. The build fails on a missing font, so a forgotten file is not a silent
degradation.
