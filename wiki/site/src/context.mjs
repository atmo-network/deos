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

export function bindNativeSearchContext(html, { index, nextIndex, context, ui, book }) {
  const scripts = [...html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/g)];
  const assignments = [];
  const resources = new Map();
  for (const script of scripts) {
    const [, attrs, body] = script;
    for (const assignment of body.matchAll(/\bwindow\.path_to_searchindex_js\s*=/g)) assignments.push({ script, assignment });
    const src = attrs.match(/\bsrc="([^"]+)"/)?.[1];
    const kind = src?.split('/').at(-1).match(/^(searcher|book|layer-context)(?:-[a-z0-9]+)+\.js$/)?.[1];
    if (kind) {
      if (resources.has(kind)) throw new Error(`Duplicate native ${kind} asset binding`);
      resources.set(kind, { script, src });
    }
  }
  // mdBook's standalone TOC intentionally has no article chrome or search assets.
  if (!html.includes('<main>') && !assignments.length && !resources.size) return { content: html, bindings: 0 };
  if (assignments.length !== 1) throw new Error('Expected exactly one native search index assignment');
  const [{ script, assignment }] = assignments;
  const head = html.match(/<head\b[^>]*>[\s\S]*?<\/head>/);
  const body = html.match(/<body\b[^>]*>[\s\S]*?<\/body>/);
  const inside = (element, tag) => element && tag.index > element.index && tag.index + tag[0].length < element.index + element[0].length;
  if (!inside(head, script) || script[1] !== '') throw new Error('Native search index must be assigned by an inline script in the head');
  const root = script[2].match(/\bconst path_to_root = "((?:\.\.\/)*)";/);
  const binding = script[2].slice(assignment.index).match(/^window\.path_to_searchindex_js = "([^"]+)";/);
  if (!root || root.index >= assignment.index || binding?.[1] !== root[1] + index) throw new Error('Unexpected native search asset binding');
  if (resources.has('layer-context')) throw new Error('Unexpected pre-existing native search context');
  const directory = root[1], search = resources.get('searcher'), reader = resources.get('book');
  if (!search || !reader || search.src !== directory + ui || reader.src !== directory + book) throw new Error('Missing or unexpected native search UI/book reader companion');
  if (!inside(body, search.script) || !inside(body, reader.script) || script.index >= search.script.index || search.script.index >= reader.script.index) throw new Error('Expected native search context before search UI and book reader in the body');

  const rebound = script[2].replace(binding[0], `window.path_to_searchindex_js = "${directory}${nextIndex}";`);
  const replacement = `<script>${rebound}window.wikiRequiresContext = true;\n document.documentElement.dataset.wikiLayer = "pending";\n</script><script src="${directory}${context}"></script>`;
  const content = html.slice(0, script.index) + replacement + html.slice(script.index + script[0].length);
  return { content, bindings: 1 };
}
