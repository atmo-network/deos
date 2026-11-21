/*
Domain: Wiki math asset placement
Owns: The vendored KaTeX stylesheet, the build-time typesetter module and the font files it references.
Excludes: Formula rendering decisions, authoring conventions and browser behaviour.
*/
import { readFileSync, readdirSync } from 'node:fs';

// `theme/katex/` holds the vendored distribution. The stylesheet and fonts are published; the module
// is build-time only and never reaches a browser, because the projection typesets ahead of rendering.
const root = new URL('../theme/katex/', import.meta.url);
const read = name => readFileSync(new URL(name, root), 'utf8');

export const katexAssets = [['theme/katex/katex.min.css', read('katex.min.css')]];

export const katexFonts = readdirSync(new URL('fonts/', root))
  .filter(name => name.endsWith('.woff2'))
  .sort()
  .map(name => [`theme/katex/fonts/${name}`, readFileSync(new URL(`fonts/${name}`, root))]);
