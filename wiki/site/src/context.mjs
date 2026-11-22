/*
Domain: Wiki reader context
Owns: Layer fragment state, canonical filtered chapter order and native context asset bindings.
Excludes: Membership inference, Markdown rendering, DOM presentation and parent-frame access.
*/
import { readHashState, writeHashState } from './hash-state.mjs';
import { selectLayer } from './layers.mjs';

export function readLayerSelection(href, layers) {
  const { layer } = readHashState(href);
  if (layer === null) return null;
  return layer === 'all' || layers.some(candidate => candidate.id === layer) ? layer : null;
}

export function withLayer(href, layer) {
  return writeHashState(href, { layer });
}

export function chapterOrder(graph, locale, layer) {
  const selected = new Set(selectLayer(graph, layer).linear_order);
  const byId = new Map(graph.nodes.map(node => [node.id, node]));
  const base = graph.base_path ?? '/';
  const result = [`${base}${locale}/index.html`];
  for (const plane of graph.layers) {
    for (const id of graph.linear_order) {
      const node = byId.get(id);
      if (node.plane === plane.id && selected.has(id)) result.push(node.url[locale]);
    }
    if (!graph.nodes.some(node => node.plane === plane.id) && (layer === 'all' || layer === plane.id)) result.push(`${base}${locale}/${plane.id}/index.html`);
  }
  return result;
}

export function contextScript(graph, locale, indexes) {
  const { base_path, layers, locales, default_locale, edges, linear_order, chrome } = graph;
  // The reader needs the page glyph next to the title, so the embedded graph keeps it; description,
  // provenance and route metadata stay out of the page.
  const nodes = graph.nodes.map(({ id, plane, layers, title, url, icon }) => ({ id, plane, layers, title, url, icon }));
  const data = JSON.stringify({ graph: { base_path, layers, locales, default_locale, chrome, nodes, edges, linear_order }, locale, indexes });
  const methods = [readHashState, writeHashState, selectLayer, readLayerSelection, withLayer, chapterOrder].map(fn => fn.toString()).join('\n');
  return `(() => {\n${methods}\nconst context = JSON.parse(${JSON.stringify(data)});\ncontext.selection = readLayerSelection(location.href, context.graph.layers);\ncontext.view = context.selection === null ? null : selectLayer(context.graph, context.selection);\ncontext.readState = readHashState;\ncontext.withState = writeHashState;\ncontext.withLayer = withLayer;\ncontext.chapterOrder = chapterOrder;\n// mdBook's path_to_root locates this book from the document base, whatever URL style served it.\nconst bookRoot = new URL(path_to_root, document.baseURI);\ncontext.siteRoot = new URL(context.locale ? '../' : './', bookRoot).href;\ncontext.page = typeof window.wikiSource === 'string' ? (context.locale ? context.locale + '/' : '') + window.wikiSource.replace(/\\.md$/, '.html') : null;\n// A stateful fragment carries its section, so the browser's own anchor lookup cannot reach it.\ncontext.section = readHashState(location.href).at;\nif (context.section !== null) document.addEventListener('DOMContentLoaded', () => {\n  const target = document.getElementById(context.section);\n  if (target) target.scrollIntoView();\n}, { once: true });\nif (context.locale && context.selection !== null) {\n  context.indexAsset = new URL(context.indexes[context.selection].asset, bookRoot).href;\n  window.path_to_searchindex_js = context.indexAsset;\n}\nwindow.wikiContext = context;\n})();\n`;
}

export function bindNativeSearchContext(html, previousIndex, indexAsset, contextAsset) {
  let bindings = 0;
  const content = html.replace(/<script>([\s\S]*?)<\/script>/g, (script, body) => {
    if (!body.includes('const path_to_root = ')) return script;
    let directory;
    const rebound = body.replace(/(window\.path_to_searchindex_js = ")([^"]+)(";)/g, (_assignment, prefix, path, suffix) => {
      if (directory !== undefined) throw new Error('Duplicate native search asset binding');
      if (path !== previousIndex && !path.endsWith('/' + previousIndex)) throw new Error('Unexpected native search asset binding');
      directory = path.slice(0, -previousIndex.length);
      return prefix + directory + indexAsset + suffix;
    });
    if (directory === undefined) return script;
    bindings++;
    return `<script>${rebound}window.wikiRequiresContext = true;\n document.documentElement.dataset.wikiLayer = "pending";\n</script><script src="${directory}${contextAsset}"></script>`;
  });
  return { content, bindings };
}
