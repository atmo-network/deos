#!/usr/bin/env node
/*
Domain: Native search artifact preparation
Owns: Native search data/UI fingerprints and exact asset bindings inside an owned stage.
Excludes: Markdown rendering, source mutation, process orchestration and untrusted input execution.
*/
import { createHash } from 'node:crypto';
import { lstatSync, readFileSync, readdirSync, realpathSync, unlinkSync, writeFileSync } from 'node:fs';
import { join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Script } from 'node:vm';
import { rebuildNativeSearch, rebindSearchAsset } from '../src/search.mjs';
import { rewriteNativeSearchUrls, rebindScriptAsset } from '../src/search-url.mjs';
import { selectLayer } from '../src/layers.mjs';
import { bindLayerContext, contextScript } from '../src/context.mjs';
import { localizeBookScript, localizeReaderChrome, localizeSearchScript } from '../src/locale.mjs';

function prepareSearch(directory, projection, locale) {
  const root = realpathSync(directory);
  const target = realpathSync(fileURLToPath(new URL('../target/', import.meta.url)));
  if (!root.startsWith(target + sep)) throw new Error('Search preparation requires an owned build stage beneath site/target');
  if (!lstatSync(projection).isFile() || !realpathSync(projection).startsWith(target + sep)) throw new Error('Search projection must be a regular file in the owned build stage');
  const graph = JSON.parse(readFileSync(projection, 'utf8'));
  if (!graph.locales?.includes(locale) || !Array.isArray(graph.layers) || graph.layers.some(layer => !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(layer.id) || layer.id === 'all') || new Set(graph.layers.map(layer => layer.id)).size !== graph.layers.length) throw new Error('Unsupported layer search projection');
  const files = readdirSync(root);
  function one(pattern) {
    const matches = files.filter(name => pattern.test(name));
    if (matches.length !== 1) throw new Error(`Expected one pinned native search asset: ${pattern}`);
    if (!lstatSync(join(root, matches[0])).isFile()) throw new Error('Native search inputs must be regular generated files');
    return matches[0];
  }
  const previous = one(/^searchindex-[a-f0-9]{8}\.js$/);
  const library = one(/^elasticlunr-[a-f0-9]{8}\.min\.js$/);
  const librarySource = readFileSync(join(root, library), 'utf8');
  const nativeIndex = readFileSync(join(root, previous), 'utf8');
  const rebuilt = rebuildNativeSearch(librarySource, nativeIndex);
  const next = `searchindex-unicode-${createHash('sha256').update(rebuilt.script).digest('hex').slice(0, 8)}.js`;
  const indexes = { all: { asset: next, sections: rebuilt.sections } };
  for (const layer of graph.layers) {
    const documents = new Set(selectLayer(graph, layer.id).nodes.map(node => {
      const url = node.url[locale];
      if (!url?.startsWith(`/${locale}/`)) throw new Error(`Missing localized search destination: ${node.id}/${locale}`);
      return url.slice(locale.length + 2);
    }));
    const scoped = rebuildNativeSearch(librarySource, nativeIndex, documents);
    const asset = `searchindex-layer-${layer.id}-${createHash('sha256').update(scoped.script).digest('hex').slice(0, 8)}.js`;
    writeFileSync(join(root, asset), scoped.script);
    indexes[layer.id] = { asset, sections: scoped.sections };
  }
  const context = contextScript(graph, locale, indexes);
  const contextAsset = `layer-context-${createHash('sha256').update(context).digest('hex').slice(0, 8)}.js`;
  writeFileSync(join(root, contextAsset), context);
  writeFileSync(join(root, 'search-layers.json'), JSON.stringify({ schema_version: 1, locale, indexes, context_asset: contextAsset }, null, 2) + '\n');
  const previousUi = one(/^searcher-[a-f0-9]{8}\.js$/);
  let reader = localizeSearchScript(rewriteNativeSearchUrls(readFileSync(join(root, previousUi), 'utf8')), locale);
  const fallback = `path_to_root + '${previous}'`;
  if (!reader.includes(fallback) || reader.indexOf(fallback) !== reader.lastIndexOf(fallback)) throw new Error('Pinned native search index fallback changed');
  reader = reader.replace(fallback, `(window.wikiContext?.indexAsset || path_to_root + '${next}')`);
  new Script(reader);
  const nextUi = `searcher-urls-${createHash('sha256').update(reader).digest('hex').slice(0, 8)}.js`;
  const previousBook = one(/^book-[a-f0-9]{8}\.js$/);
  const book = localizeBookScript(readFileSync(join(root, previousBook), 'utf8'), locale);
  new Script(book);
  const nextBook = `book-reader-${createHash('sha256').update(book).digest('hex').slice(0, 8)}.js`;
  let bindings = 0, uiBindings = 0, contextBindings = 0, bookBindings = 0;
  function walk(path) {
    for (const entry of readdirSync(path, { withFileTypes: true })) {
      const file = join(path, entry.name);
      if (entry.isSymbolicLink()) throw new Error(`Unexpected output symlink: ${file}`);
      if (entry.isDirectory()) walk(file);
      else if (entry.name.endsWith('.html')) {
        const localized = localizeReaderChrome(readFileSync(file, 'utf8'), locale);
        const rebound = rebindSearchAsset(localized, previous, next);
        const ui = rebindScriptAsset(rebound.content, previousUi, nextUi);
        const book = rebindScriptAsset(ui.content, previousBook, nextBook);
        const contextual = bindLayerContext(book.content, contextAsset);
        bindings += rebound.bindings;
        uiBindings += ui.bindings;
        contextBindings += contextual.bindings;
        bookBindings += book.bindings;
        if (rebound.bindings || ui.bindings) writeFileSync(file, contextual.content);
      }
    }
  }
  writeFileSync(join(root, next), rebuilt.script);
  writeFileSync(join(root, nextUi), reader);
  writeFileSync(join(root, nextBook), book);
  walk(root);
  if (!bindings || !uiBindings || contextBindings !== bindings || bookBindings !== bindings) throw new Error('Native search/context assets had missing HTML bindings');
  unlinkSync(join(root, previous));
  unlinkSync(join(root, previousUi));
  unlinkSync(join(root, previousBook));
  console.log(`Native Unicode search: ${rebuilt.sections} stored sections; ${bindings} index/${uiBindings} UI bindings; ${next}; ${nextUi}`);
  console.log(`Native layer sections: ${JSON.stringify(Object.fromEntries(Object.entries(indexes).map(([id, entry]) => [id, entry.sections])))}`);
}

if (process.argv.includes('--help') || process.argv.includes('-h')) console.log('Usage: prepare-search.mjs OWNED_RENDERED_LOCALE_DIRECTORY OWNED_PROJECTION_JSON LOCALE');
else {
  if (process.argv.length !== 5) throw new Error('Provide a rendered locale directory, owned projection and locale');
  prepareSearch(resolve(process.argv[2]), resolve(process.argv[3]), process.argv[4]);
}
