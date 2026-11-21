/*
Domain: Published Wiki route history
Owns: Article paths that were published once and moved when corpus directories were flattened.
Excludes: Corpus projection, HTML rendering, crawler metadata emission and hosting configuration.
*/

// A directory that held fewer pages than the navigation minimum no longer groups anything, so its
// articles moved to the plane root. Each former path is still in the wild as a published URL, so the
// artifact keeps answering it with a redirect stub instead of a 404. `pageId` resolves the current
// route from the projected graph, which keeps the destination out of this ledger.
export const movedRoutes = [
  { pageId: 'contributing', from: 'community/contributing' },
  { pageId: 'physics-vs-politics', from: 'comparisons/physics-vs-politics' },
  { pageId: 'status', from: 'development/status' },
  { pageId: 'three-layer-validation', from: 'development/three-layer-validation' },
  { pageId: 'newcomer-faq', from: 'faq/newcomer-faq' },
  { pageId: 'core-terms', from: 'glossary/core-terms' },
  { pageId: 'repository-structure', from: 'implementation/repository-structure' },
  { pageId: 'tech-stack', from: 'implementation/tech-stack' },
  { pageId: 'tmctol-formulas', from: 'math/tmctol-formulas' },
  { pageId: 'forking-deos', from: 'usage/forking-deos' },
  { pageId: 'scripts-layer', from: 'usage/scripts-layer' },
];

// Retired articles are not redirected: their content was merged into a page with a different reading
// purpose, so a redirect would promise a continuation that does not exist. `consolidation.json`
// records each retirement with its `merged_into` successor.
export function resolveMovedRoutes(graph) {
  // One stub per ledger entry per locale; a caller that owns the real corpus asserts this count.
  const base = graph.base_path ?? '/';
  const locales = graph.locales ?? [];
  const resolved = [];
  for (const route of movedRoutes) {
    // Resolution follows the projected graph, so a probe artifact that declares no pages resolves no
    // stubs. The producer owns completeness: it requires one stub per ledger entry per locale.
    const node = graph.nodes.find(candidate => candidate.canonical_page_id === route.pageId);
    if (!node) continue;
    for (const locale of locales) {
      const url = node.url?.[locale];
      if (typeof url !== 'string' || !url.startsWith(base)) throw new Error(`Moved route ${route.pageId} has no route for ${locale}`);
      // The current route supplies its own directory, so the ledger never repeats the layer name.
      const to = url.slice(base.length);
      const directory = to.slice(0, to.lastIndexOf('/') + 1);
      resolved.push({ locale, from: `${directory}${route.from}.html`, to, pageId: route.pageId });
    }
  }
  return resolved;
}
