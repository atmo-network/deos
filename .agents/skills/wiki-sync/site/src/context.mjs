/*
Domain: Wiki reader context
Owns: Layer URL state, canonical filtered chapter order and native context asset bindings.
Excludes: Membership inference, Markdown rendering, DOM presentation and parent-frame access.
*/
import { selectLayer } from './layers.mjs';

export function readLayerSelection(href, layers) {
  const values = new URL(href).searchParams.getAll('layer');
  if (values.length === 0) return 'all';
  return values.length === 1 && (values[0] === 'all' || layers.some(layer => layer.id === values[0])) ? values[0] : null;
}

export function withLayer(href, layer) {
  const url = new URL(href);
  if (layer === 'all') url.searchParams.delete('layer');
  else url.searchParams.set('layer', layer);
  return url.href;
}

export function chapterOrder(graph, locale, layer) {
  const selected = new Set(selectLayer(graph, layer).linear_order);
  const byId = new Map(graph.nodes.map(node => [node.id, node]));
  const result = [`/${locale}/index.html`];
  for (const plane of graph.layers) {
    for (const id of graph.linear_order) {
      const node = byId.get(id);
      if (node.plane === plane.id && selected.has(id)) result.push(node.url[locale]);
    }
    if (!graph.nodes.some(node => node.plane === plane.id) && (layer === 'all' || layer === plane.id)) result.push(`/${locale}/${plane.id}/index.html`);
  }
  return [...result, `/${locale}/graph.html`];
}

export function contextScript(graph, locale, indexes) {
  const { layers, locales, default_locale, edges, linear_order } = graph;
  const nodes = graph.nodes.map(({ id, plane, layers, title, url }) => ({ id, plane, layers, title, url }));
  const data = JSON.stringify({ graph: { layers, locales, default_locale, nodes, edges, linear_order }, locale, indexes });
  const methods = [selectLayer, readLayerSelection, withLayer, chapterOrder].map(fn => fn.toString()).join('\n');
  return `(() => {\n${methods}\nconst context = JSON.parse(${JSON.stringify(data)});\ncontext.selection = readLayerSelection(location.href, context.graph.layers);\ncontext.view = context.selection === null ? null : selectLayer(context.graph, context.selection);\ncontext.withLayer = withLayer;\ncontext.chapterOrder = chapterOrder;\nif (context.locale && context.selection !== null) {\n  context.indexAsset = new URL(context.indexes[context.selection].asset, new URL(path_to_root, location.href)).href;\n  window.path_to_searchindex_js = context.indexAsset;\n}\nwindow.wikiContext = context;\n})();\n`;
}

export function bindLayerContext(html, asset) {
  let bindings = 0;
  const content = html.replace(/<script>([\s\S]*?)<\/script>/g, (script, body) => {
    if (!body.includes('const path_to_root = ')) return script;
    const binding = body.match(/window\.path_to_searchindex_js = "([^"]+)";/);
    if (!binding) return script;
    const path = binding[1].slice(0, binding[1].lastIndexOf('/') + 1) + asset;
    bindings++;
    return script.replace('</script>', 'window.wikiRequiresContext = true;\n document.documentElement.dataset.wikiLayer = "pending";\n</script>') + `<script src="${path}"></script>`;
  });
  return { content, bindings };
}
