/*
Domain: Reader context regressions
Owns: Exact layer URL admission, filtered order and early native context bindings.
Excludes: Browser behavior, iframe acceptance and untrusted-script execution.
*/
import assert from 'node:assert/strict';
import { createContext, runInContext } from 'node:vm';
import test from 'node:test';
import { loadCorpus } from '../src/corpus.mjs';
import { unifiedGraph } from '../src/projection.mjs';
import { bindLayerContext, chapterOrder, contextScript, readLayerSelection, withLayer } from '../src/context.mjs';

const corpus = loadCorpus();
const graph = unifiedGraph({ ...corpus, planes: [...corpus.planes, { id: 'example', title: 'Example fixture', pages: new Map(), graph: { edges: [] }, locales: [] }] });

test('layer URL admission is exact, rejects duplicates and preserves unrelated query values', () => {
  for (const [query, selected] of [['', 'all'], ['?layer=all', 'all'], ['?layer=deos', 'deos'], ['?layer=example', 'example'], ['?layer=', null], ['?layer=unknown', null], ['?layer=deos&layer=deos', null], ['?layer=example&layer=deos', null]]) assert.equal(readLayerSelection('https://wiki.invalid/en/' + query, graph.layers), selected);
  const source = 'https://wiki.invalid/en/?tag=a%3Db&tag=x%26y&layer=deos&search=%25%2B#section';
  const next = new URL(withLayer(source, 'example'));
  assert.deepEqual(next.searchParams.getAll('tag'), ['a=b', 'x&y']);
  assert.equal(next.searchParams.get('search'), '%+');
  assert.equal(next.hash, '#section');
  assert.equal(next.searchParams.get('layer'), 'example');
  assert.ok(!new URL(withLayer(next.href, 'all')).searchParams.has('layer'));
});

test('chapter order is the native metadata subsequence with honest empty-plane navigation', () => {
  const all = chapterOrder(graph, 'en', 'all');
  assert.deepEqual(all, ['/en/index.html', ...graph.linear_order.map(id => graph.nodes.find(node => node.id === id).url.en), '/en/example/index.html', '/en/graph.html']);
  assert.deepEqual(chapterOrder(graph, 'ru', 'example'), ['/ru/index.html', '/ru/example/index.html', '/ru/graph.html']);
  const ids = graph.linear_order.filter((_, index) => index === 3 || index === 10);
  const mixed = { ...graph, nodes: graph.nodes.map(node => ({ ...node, layers: ids.includes(node.id) ? ['deos', 'example'] : node.layers })) };
  assert.deepEqual(chapterOrder(mixed, 'en', 'example'), ['/en/index.html', ...ids.map(id => graph.nodes.find(node => node.id === id).url.en), '/en/example/index.html', '/en/graph.html']);
});

test('context data selects the native index before reader initialization without provenance duplication', () => {
  for (const selection of ['all', 'deos', 'example', 'unknown']) {
    const context = createContext({ URL, window: {}, location: { href: `https://wiki.invalid/en/deos/overview/actor-system.en.html?layer=${selection}` }, path_to_root: '../../' });
    const indexes = Object.fromEntries(['all', 'deos', 'example'].map(id => [id, { asset: `searchindex-${id}.js` }]));
    runInContext(contextScript(graph, 'en', indexes), context);
    assert.equal(context.window.wikiContext.selection, selection === 'unknown' ? null : selection);
    assert.ok(!('provenance' in context.window.wikiContext.graph.nodes[0]));
    if (selection !== 'unknown') assert.equal(context.window.path_to_searchindex_js, `https://wiki.invalid/en/searchindex-${selection}.js`);
    else assert.equal(context.window.path_to_searchindex_js, undefined);
  }
});

test('native context binding gates the reader in the head and never rewrites article text', () => {
  const head = '<script>const path_to_root = "../../"; window.path_to_searchindex_js = "../../searchindex-old.js";</script>';
  const body = '<main><code>window.path_to_searchindex_js = "example.js";</code></main>';
  const result = bindLayerContext(head + body, 'layer-context-12345678.js');
  assert.equal(result.bindings, 1);
  assert.ok(result.content.includes('wikiRequiresContext = true'));
  assert.ok(result.content.includes('<script src="../../layer-context-12345678.js"></script>'));
  assert.ok(result.content.endsWith(body));
});
