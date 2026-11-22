/*
Domain: Reader context regressions
Owns: Exact layer URL admission, filtered order and early native context bindings.
Excludes: Browser behavior, iframe acceptance and untrusted-script execution.
*/
import assert from 'node:assert/strict';
import { createContext, runInContext } from 'node:vm';
import test from 'node:test';
import { loadCorpus } from '../src/corpus.mjs';
import { readHashState } from '../src/hash-state.mjs';
import { unifiedGraph } from '../src/projection.mjs';
import { bindNativeSearchContext, chapterOrder, contextScript, readLayerSelection, withLayer } from '../src/context.mjs';

const corpus = loadCorpus();
const graph = unifiedGraph({ ...corpus, planes: [...corpus.planes, { id: 'example', title: 'Example fixture', pages: new Map(), graph: { edges: [] }, locales: [] }] });

test('layer admission is exact in the fragment, rejects duplicates and ignores server-facing queries', () => {
  for (const [fragment, selected] of [['', 'all'], ['#!layer=all', 'all'], ['#!layer=deos', 'deos'], ['#!layer=example', 'example'], ['#!layer=', null], ['#!layer=unknown', null], ['#!layer=deos&layer=deos', null], ['#!layer=example&layer=deos', null], ['?layer=deos', 'all'], ['#summary', 'all']]) assert.equal(readLayerSelection('https://wiki.invalid/en/' + fragment, graph.layers), selected);
  const source = 'https://wiki.invalid/en/?tag=a%3Db&tag=x%26y#!search=%25%2B&at=section';
  const next = new URL(withLayer(source, 'example'));
  assert.deepEqual(next.searchParams.getAll('tag'), ['a=b', 'x&y'], 'a layer change must not touch the query string');
  assert.equal(readHashState(next.href).search, '%+');
  assert.equal(readHashState(next.href).at, 'section');
  assert.equal(readHashState(next.href).layer, 'example');
  assert.ok(!readHashState(withLayer(next.href, 'all')).stateful || readHashState(withLayer(next.href, 'all')).layer === 'all');
});

test('chapter order is the native metadata subsequence with honest empty-plane navigation', () => {
  const all = chapterOrder(graph, 'en', 'all');
  assert.deepEqual(all, ['/en/index.html', ...graph.linear_order.map(id => graph.nodes.find(node => node.id === id).url.en), '/en/example/index.html']);
  assert.deepEqual(chapterOrder(graph, 'ru', 'example'), ['/ru/index.html', '/ru/example/index.html']);
  const ids = graph.linear_order.filter((_, index) => index === 3 || index === 10);
  const mixed = { ...graph, nodes: graph.nodes.map(node => ({ ...node, layers: ids.includes(node.id) ? ['deos', 'example'] : node.layers })) };
  assert.deepEqual(chapterOrder(mixed, 'en', 'example'), ['/en/index.html', ...ids.map(id => graph.nodes.find(node => node.id === id).url.en), '/en/example/index.html']);
});

test('chapter order and serialized reader context carry the deployment base path', () => {
  const prefixed = unifiedGraph({ ...corpus, planes: [...corpus.planes, { id: 'example', title: 'Example fixture', pages: new Map(), graph: { edges: [] }, locales: [] }] }, '/deos/');
  const order = chapterOrder(prefixed, 'ru', 'all');
  assert.equal(order[0], '/deos/ru/index.html');
  assert.equal(order.at(-1), '/deos/ru/example/index.html');
  assert.ok(!order.some(path => path.endsWith('/graph.html')), 'the knowledge graph stays behind the reader UI');
  assert.ok(order.includes('/deos/ru/example/index.html'));
  assert.ok(order.every(path => path.startsWith('/deos/ru/')));
  assert.match(contextScript(prefixed, 'en', { all: { asset: 'a.js', sections: 0 } }), /\\"base_path\\":\\"\/deos\/\\"/);
});

test('context data selects the native index before reader initialization without provenance duplication', () => {
  for (const selection of ['all', 'deos', 'example', 'unknown']) {
    const href = `https://wiki.invalid/en/deos/overview/actor-system.html${selection === 'all' ? '' : `#!layer=${selection}`}`;
    const context = createContext({ URL, URLSearchParams, document: { baseURI: href }, location: { href }, path_to_root: '../../', window: { wikiSource: 'deos/overview/actor-system.md' } });
    const indexes = Object.fromEntries(['all', 'deos', 'example'].map(id => [id, { asset: `searchindex-${id}.js` }]));
    runInContext(contextScript(graph, 'en', indexes), context);
    assert.equal(context.window.wikiContext.selection, selection === 'unknown' ? null : selection);
    assert.ok(!('provenance' in context.window.wikiContext.graph.nodes[0]));
    assert.equal(context.window.wikiContext.siteRoot, 'https://wiki.invalid/');
    assert.equal(context.window.wikiContext.page, 'en/deos/overview/actor-system.html');
    if (selection !== 'unknown') assert.equal(context.window.path_to_searchindex_js, `https://wiki.invalid/en/searchindex-${selection}.js`);
    else assert.equal(context.window.path_to_searchindex_js, undefined);
  }
});

const assets = { index: 'searchindex-old.js', nextIndex: 'searchindex-new.js', context: 'layer-context-12345678.js', ui: 'searcher-old.js', book: 'book-old.js' };
const assignment = (directory, index = assets.index) => `<script>const path_to_root = "${directory}";\nwindow.path_to_searchindex_js = "${directory}${index}";\n</script>`;
const resource = (directory, asset) => `<script src="${directory}${asset}"></script>`;
const article = '<main><code>searchindex-old.js</code></main>';
const page = (head, body) => `<html><head>${head}</head><body>${body}</body></html>`;
const renderedPage = directory => page(assignment(directory), article + resource(directory, assets.ui) + resource(directory, assets.book));

test('native head binding pairs the index and early context without rewriting article text', () => {
  for (const directory of ['', '../../']) {
    const result = bindNativeSearchContext(renderedPage(directory), assets);
    const head = assignment(directory, assets.nextIndex).replace('</script>', `window.wikiRequiresContext = true;\n document.documentElement.dataset.wikiLayer = "pending";\n</script>`) + resource(directory, assets.context);
    assert.deepEqual(result, { content: page(head, article + resource(directory, assets.ui) + resource(directory, assets.book)), bindings: 1 });
  }
  const unbound = page('', '<nav>Native TOC</nav>');
  assert.deepEqual(bindNativeSearchContext(unbound, assets), { content: unbound, bindings: 0 });
});

test('native head contract rejects duplicate, misplaced and unexpected assignments', () => {
  const valid = renderedPage('../../');
  const index = assignment('../../');
  assert.throws(() => bindNativeSearchContext(valid.replace(index, index + index), assets), /exactly one native search index assignment/);
  assert.throws(() => bindNativeSearchContext(valid.replace('</script>', index.split('\n')[1] + '\n</script>'), assets), /exactly one native search index assignment/);
  assert.throws(() => bindNativeSearchContext(valid.replace(index, '').replace('<body>', '<body>' + index), assets), /inline script in the head/);
  assert.throws(() => bindNativeSearchContext(valid.replace(assets.index, 'different.js'), assets), /Unexpected native search asset/);
  assert.throws(() => bindNativeSearchContext(valid.replace(index, index + resource('../../', assets.context)), assets), /pre-existing native search context/);
});

test('every search-bearing page needs its own ordered companions, regardless of totals', () => {
  const valid = renderedPage('../../');
  const search = resource('../../', assets.ui), book = resource('../../', assets.book);
  for (const companion of [search, book]) {
    const pages = [valid.replace(companion, companion + companion), valid.replace(companion, ''), renderedPage('')];
    assert.throws(() => pages.map(html => bindNativeSearchContext(html, assets)), /Duplicate native .* asset binding/);
    pages[0] = valid;
    assert.throws(() => pages.map(html => bindNativeSearchContext(html, assets)), /Missing or unexpected native search UI\/book reader companion/);
  }
  assert.throws(() => bindNativeSearchContext(valid.replace(search + book, book + search), assets), /Expected native search context before search UI and book reader/);
  assert.throws(() => bindNativeSearchContext(valid.replace(assignment('../../'), search + assignment('../../')).replace(search + book, book), assets), /Expected native search context before search UI and book reader/);
});
