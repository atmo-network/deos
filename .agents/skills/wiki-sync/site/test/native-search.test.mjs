/*
Domain: Native search integration evidence
Owns: Shipped layer-index admission and pre-limit counterexamples in the pinned bundled engine.
Excludes: Published downstream knowledge, browser layer controls, Markdown rendering and performance claims.
*/
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { assignMemberships, selectLayer } from '../src/layers.mjs';
import { rebuildNativeSearch } from '../src/search.mjs';
import { loadNative, nativeLibrary } from './native.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../dist');
const graph = JSON.parse(readFileSync(join(root, 'graph.json'), 'utf8'));

for (const locale of graph.locales) {
  const directory = join(root, locale);
  const library = nativeLibrary(directory);
  const manifest = JSON.parse(readFileSync(join(directory, 'search-layers.json'), 'utf8'));
  const all = loadNative(library, readFileSync(join(directory, manifest.indexes.all.asset), 'utf8'));

  test(`${locale}: shipped native layer indices contain exactly their eligible stored sections`, () => {
    assert.deepEqual(Object.keys(manifest.indexes), ['all', ...graph.layers.map(layer => layer.id)]);
    for (const [layer, entry] of Object.entries(manifest.indexes)) {
      const { data, index } = loadNative(library, readFileSync(join(directory, entry.asset), 'utf8'));
      const documents = new Set(selectLayer(graph, layer).nodes.map(node => node.url[locale].slice(locale.length + 2)));
      const expected = Object.keys(all.data.index.documentStore.docs).filter(ref => layer === 'all' || documents.has(all.data.doc_urls[Number(ref)].split('#')[0]));
      const stored = data.index.documentStore.docs;
      assert.equal(entry.sections, expected.length);
      assert.deepEqual(Object.keys(stored), expected);
      for (const ref of expected) assert.deepEqual(stored[ref], all.data.index.documentStore.docs[ref]);
      for (const field of ['ref', 'fields', 'pipeline']) assert.deepEqual(data.index[field], all.data.index[field]);
      for (const field of ['doc_urls', 'search_options', 'results_options']) assert.deepEqual(data[field], all.data[field]);
      if (expected.length === 0) {
        assert.equal(index.search(locale === 'ru' ? 'наблюдения' : 'observations', data.search_options).length, 0);
      }
    }
  });

  test(`${locale}: a mixed-member result below the unfiltered limit remains searchable`, () => {
    const shared = graph.nodes.find(node => node.id === 'deos/typed-observations');
    const other = graph.nodes.find(node => node.id === 'deos/actor-system');
    const layers = [...graph.layers, { id: 'example', title: 'Example fixture' }];
    const mixed = { ...graph, layers, nodes: assignMemberships(graph.nodes, layers, { [shared.id]: ['deos', 'example'] }) };
    assert.equal(mixed.nodes.length, graph.nodes.length);
    assert.deepEqual(selectLayer(mixed, 'example').nodes.map(node => node.id), [shared.id]);
    const eligible = new Set(selectLayer(mixed, 'example').nodes.map(node => node.url[locale].slice(locale.length + 2)));
    const limit = all.data.results_options.limit_results;
    assert.ok(Number.isSafeInteger(limit) && limit > 0, 'native result-limit contract changed');
    const query = locale === 'ru' ? 'принадлежностьслою' : 'membershipneedle';
    const docs = {}, urls = [];
    for (let index = 0; index <= limit; index++) {
      docs[String(index)] = { title: query, body: query, breadcrumbs: query };
      urls.push(other.url[locale].slice(locale.length + 2));
    }
    const ref = String(urls.length);
    docs[ref] = { title: 'Shared fixture', body: query, breadcrumbs: '' };
    urls.push(shared.url[locale].slice(locale.length + 2));
    // Synthetic ranking witness only; never insert fixture claims or membership into the corpus.
    const input = { ...all.data, index: { fields: all.data.index.fields, ref: all.data.index.ref, pipeline: ['trimmer', 'stopWordFilter', 'stemmer'], documentStore: { docs } }, doc_urls: urls };
    const source = `window.search = ${JSON.stringify(input)};`;
    const unfiltered = loadNative(library, rebuildNativeSearch(library, source).script);
    const allResults = unfiltered.index.search(query, unfiltered.data.search_options);
    assert.equal(allResults.length, limit + 2);
    assert.ok(!Array.from(allResults.slice(0, limit), result => result.ref).includes(ref), 'fixture must falsify hiding after limiting');
    const rebuilt = rebuildNativeSearch(library, source, eligible);
    const filtered = loadNative(library, rebuilt.script);
    assert.equal(rebuilt.sections, 1);
    assert.deepEqual(Array.from(filtered.index.search(query, filtered.data.search_options).slice(0, limit), result => result.ref), [ref]);
    assert.equal(filtered.data.doc_urls[Number(ref)], shared.url[locale].slice(locale.length + 2));
    const empty = loadNative(library, rebuildNativeSearch(library, source, new Set()).script);
    assert.equal(Object.keys(empty.data.index.documentStore.docs).length, 0);
    assert.equal(empty.index.search(query, empty.data.search_options).length, 0);
    assert.deepEqual(mixed.edges, graph.edges);
    assert.deepEqual(mixed.linear_order, graph.linear_order);
  });
}
