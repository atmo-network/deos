/*
Domain: Wiki projection regressions
Owns: Real-corpus identity/order, source preservation, trust and mdBook-only build contracts.
Excludes: Browser/iframe acceptance, production headers and semantic source review.
*/
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { loadCorpus, pageRoute } from '../src/corpus.mjs';
import { parseFrontmatter, splitFrontmatter } from '../src/frontmatter.mjs';
import { linearOrder, projectBooks, unifiedGraph } from '../src/projection.mjs';
import { selectLayer } from '../src/layers.mjs';
import { assertTrustedMarkdown } from '../src/trust.mjs';
import { artifactFor, mdbook } from '../scripts/toolchain.mjs';
import { checkOutput } from '../scripts/check-output.mjs';
import { trimSearchToken, rebindSearchAsset, rebuildNativeSearch, nativeSections } from '../src/search.mjs';

const corpus = loadCorpus();
const extended = { ...corpus, planes: [...corpus.planes, { id: 'example', title: 'Example fixture', pages: new Map(), graph: { edges: [] }, locales: [] }] };
const assets = { 'wiki.css': '', 'wiki.js': '' };
const projected = projectBooks(corpus, assets);

test('one composed graph retains every authored typed edge and localized identity', () => {
  const expectedEdges = corpus.planes.flatMap(plane => plane.graph.edges.map(edge => ({ from: `${plane.id}/${edge.from}`, to: `${plane.id}/${edge.to}`, type: edge.type })));
  assert.deepEqual(projected.graph.edges, expectedEdges);
  assert.equal(projected.graph.nodes.length, corpus.planes.reduce((count, plane) => count + plane.graph.nodes.length, 0));
  assert.equal(new Set(projected.graph.linear_order).size, projected.graph.nodes.length);
  for (const node of projected.graph.nodes) for (const locale of corpus.locales) {
    assert.ok(node.title[locale]);
    assert.match(node.url[locale], new RegExp(`^/${locale}/${node.plane}/.+\\.html$`));
    assert.ok(node.provenance[locale].sources.length > 0);
  }
});

test('the default bilingual MVP declares only DEOS and membership follows ownership', () => {
  assert.deepEqual(projected.graph.layers, [{ id: 'deos', title: 'DEOS' }]);
  assert.deepEqual(corpus.locales, ['en', 'ru']);
  assert.match(projected.files.get('books/root/book.toml'), /DEOS Wiki/);
  assert.match(projected.files.get('books/ru/book.toml'), /Вики DEOS/);
  for (const node of projected.graph.nodes) assert.deepEqual(node.layers, [node.plane]);
});

test('explicit mixed membership shares one identity across locales without changing the graph', () => {
  const mixed = structuredClone(extended);
  mixed.memberships = { 'deos/typed-observations': ['example', 'deos'] };
  const graph = unifiedGraph(mixed);
  assert.deepEqual(graph.nodes.find(node => node.id === 'deos/typed-observations').layers, ['deos', 'example']);
  assert.equal(graph.nodes.length, projected.graph.nodes.length);
  assert.deepEqual(graph.edges, projected.graph.edges);
  assert.deepEqual(graph.linear_order, projected.graph.linear_order);
  for (const node of graph.nodes) {
    const original = projected.graph.nodes.find(candidate => candidate.id === node.id);
    assert.deepEqual(node.url, original.url);
    assert.deepEqual(node.provenance, original.provenance);
  }
});

test('invalid classification fails rather than silently broadening membership', () => {
  for (const memberships of [null, [], { 'deos/missing-node': ['deos'] }, { 'deos/typed-observations': [] }, { 'deos/typed-observations': ['unknown'] }, { 'deos/typed-observations': ['deos', 'deos'] }, { 'deos/typed-observations': 'example' }]) {
    assert.throws(() => unifiedGraph({ ...corpus, memberships }), /membership/i);
  }
});

test('layer views retain induced typed cycles and metadata order, never duplicate nodes', () => {
  assert.equal(selectLayer(projected.graph), projected.graph);
  assert.equal(selectLayer(projected.graph, 'deos').nodes.length, projected.graph.nodes.length);
  assert.deepEqual(selectLayer(unifiedGraph(extended), 'example').nodes, []);
  assert.throws(() => selectLayer(projected.graph, 'unknown'), /Unknown knowledge layer/);
  const mixed = structuredClone(extended);
  const plane = mixed.planes.find(candidate => candidate.graph.edges.some(edge => edge.from !== edge.to));
  const edge = plane.graph.edges.find(edge => edge.from !== edge.to);
  plane.graph.edges.push({ from: edge.to, to: edge.from, type: 'references' });
  const from = `${plane.id}/${edge.from}`, to = `${plane.id}/${edge.to}`;
  mixed.memberships = { [from]: ['example'], [to]: ['deos', 'example'] };
  const graph = unifiedGraph(mixed), selected = selectLayer(graph, 'example');
  assert.equal(selected.nodes.length, 2);
  assert.deepEqual(selected.linear_order, graph.linear_order.filter(id => id === from || id === to));
  assert.deepEqual(selected.edges, graph.edges.filter(edge => [from, to].includes(edge.from) && [from, to].includes(edge.to)));
  assert.ok(selected.edges.some(edge => edge.from === from && edge.to === to));
  assert.ok(selected.edges.some(edge => edge.from === to && edge.to === from));
  assert.ok(!selectLayer(graph, 'deos').nodes.some(node => node.id === from), 'explicit membership must replace the ownership default');
  for (const node of selected.nodes) assert.equal(node, graph.nodes.find(candidate => candidate.id === node.id));
  assert.throws(() => unifiedGraph({ ...corpus, planes: [...corpus.planes, { id: 'all', title: 'Reserved', pages: new Map(), graph: { edges: [] } }] }), /virtual layer all is reserved/);
});

test('navigation metadata, not semantic edge order or graph degree, owns the linear projection', () => {
  const shuffled = structuredClone(corpus);
  for (const plane of shuffled.planes) { plane.graph.nodes.reverse(); plane.graph.edges.reverse(); }
  assert.deepEqual(unifiedGraph(shuffled).linear_order, projected.graph.linear_order);
  const changed = structuredClone(corpus);
  const plane = changed.planes.find(item => item.pages.size > 0);
  plane.navigation.sections[0].items.reverse();
  assert.notDeepEqual(unifiedGraph(changed).linear_order, projected.graph.linear_order);
  assert.equal(unifiedGraph(changed).edges.length, projected.graph.edges.length);
});

test('unplaced graph pages fail rather than acquiring an invented priority', () => {
  const plane = structuredClone(corpus.planes.find(item => item.pages.size > 0));
  const removed = plane.navigation.sections[0].items[0].id;
  for (const section of plane.navigation.sections) section.items = section.items.filter(item => item.id !== removed);
  assert.throws(() => linearOrder(plane, plane.defaultLocale), /explicit navigation position/);
});

test('every locale is a projection of the same branch with source Markdown unchanged', () => {
  for (const plane of corpus.planes) for (const locale of plane.locales) {
    const summary = projected.files.get(`books/${locale}/src/SUMMARY.md`);
    let position = -1;
    for (const id of linearOrder(plane, plane.defaultLocale)) {
      const page = plane.pages.get(`${locale}/${id}`);
      const path = `${plane.id}/${page.sourcePath}`;
      const next = summary.indexOf(`](${path})`);
      assert.ok(next > position, `${locale}: ${id} changed metadata order`);
      position = next;
      const markdown = projected.files.get(`books/${locale}/src/${path}`);
      assert.ok(markdown.includes(page.body.trimEnd()), `${path}: authored body was rewritten`);
      assert.ok(!markdown.startsWith('---\n'), `${path}: frontmatter leaked into the document`);
      assert.ok(!markdown.includes('resource: ../../'), `${path}: provenance source panel leaked`);
    }
  }
});

test('empty optional plane has an honest landing but creates no authored graph node', () => {
  const empty = extended.planes.find(plane => plane.pages.size === 0);
  const emptyProjection = projectBooks(extended, assets);
  assert.ok(empty, 'fixture must retain the declared empty Example plane');
  assert.ok(!projected.graph.nodes.some(node => node.plane === empty.id));
  for (const locale of corpus.locales) assert.ok(emptyProjection.files.has(`books/${locale}/src/${empty.id}/index.md`));
  assert.match(emptyProjection.files.get('books/en/src/example/index.md'), /No pages are published/);
  assert.match(emptyProjection.files.get('books/ru/src/example/index.md'), /нет опубликованных страниц/);
});

test('mdBook alone owns HTML and optional executable preprocessors are disabled', () => {
  for (const [path, content] of projected.files) {
    assert.ok(!path.endsWith('.html'), `${path}: a second HTML renderer appeared`);
    if (path.endsWith('book.toml')) {
      assert.match(content, /use-default-preprocessors = false/);
      assert.match(content, /create-missing = false/);
      assert.match(content, /runnable = false/);
    }
  }
  for (const locale of corpus.locales) assert.match(projected.files.get(`books/${locale}/book.toml`), /enable = true/);
});

test('no-script chrome uses the native header partial, never indexed article Markdown', () => {
  for (const locale of corpus.locales) {
    assert.match(projected.files.get(`books/${locale}/theme/header.hbs`), /<noscript>/);
    for (const [path, content] of projected.files) if (path.startsWith(`books/${locale}/src/`)) assert.ok(!content.includes('Layer filters require JavaScript') && !content.includes('Без него доступны все материалы'));
  }
});

test('source-route atoms cannot escape the declared locale/plane or become executable URLs', () => {
  assert.equal(pageRoute('ru', 'deos', 'overview/actor-system.ru.md'), '/ru/deos/overview/actor-system.ru.html');
  for (const value of ['../secret.md', '/root.md', 'folder/../../secret.md', 'a%2fb.md', 'a\\b.md', 'javascript:test.md']) assert.throws(() => pageRoute('en', 'deos', value));
  assert.throws(() => pageRoute('../ru', 'deos', 'index.md'));
});

test('shared parser retains strict YAML and delimiter behavior', () => {
  assert.throws(() => splitFrontmatter('# no YAML'), /opening frontmatter/);
  assert.throws(() => parseFrontmatter('type: one\ntype: two'), /unique|duplicate/i);
  assert.throws(() => parseFrontmatter('description: Topic: details'), /invalid YAML/);
  assert.equal(parseFrontmatter('type: FutureKind').type, 'FutureKind');
});

test('reviewed Markdown guard still rejects HTML, embedded media and executable URLs', () => {
  for (const source of ['<div>raw</div>', 'text <iframe src="evil">', '[bad](javascript:alert)', '[bad](data:text/html,evil)', '<a onclick=evil>']) assert.throws(() => assertTrustedMarkdown(source));
  assert.doesNotThrow(() => assertTrustedMarkdown('```rust\nfn main() {}\n```\n\n[Page](page.en.md)'));
});

test('native output validation rejects missing files, headings and executable destinations', () => {
  const root = mkdtempSync(join(tmpdir(), 'deos-mdbook-targets-'));
  try {
    const valid = '<script src="app.js"></script><a href="next.html#section">Next</a>';
    writeFileSync(join(root, 'index.html'), valid);
    writeFileSync(join(root, 'next.html'), '<h1 id="section">Section</h1>');
    writeFileSync(join(root, 'app.js'), '');
    writeFileSync(join(root, 'graph.json'), JSON.stringify({ nodes: [{ id: 'page', url: { en: '/next.html' } }], edges: [], linear_order: ['page'] }));
    assert.equal(checkOutput(root).htmlPages, 2);
    for (const [href, expected] of [['missing.html', /missing output target/], ['next.html#missing', /missing heading target/], ['javascript:alert(1)', /unsupported output URL scheme/]]) {
      writeFileSync(join(root, 'index.html'), `<a href="${href}">Target</a>`);
      assert.throws(() => checkOutput(root), expected);
    }
    writeFileSync(join(root, 'index.html'), '<script>window.path_to_searchindex_js = "missing.js";</script>');
    assert.throws(() => checkOutput(root), /missing output target/);
    writeFileSync(join(root, 'index.html'), valid);
    assert.equal(checkOutput(root).nodes, 1);
    writeFileSync(join(root, 'graph.json'), JSON.stringify({ nodes: [], edges: [], linear_order: [], layers: [{ id: 'deos', title: 'DEOS' }], locales: ['en'] }));
    assert.throws(() => checkOutput(root), /missing output target.*search-layers/);
    mkdirSync(join(root, 'en'));
    const manifest = { schema_version: 1, locale: 'en', context_asset: 'layer-context-12345678.js', indexes: { all: { asset: 'searchindex-unicode-12345678.js', sections: 0 }, deos: { asset: 'searchindex-layer-deos-12345678.js', sections: 0 } } };
    for (const entry of Object.values(manifest.indexes)) writeFileSync(join(root, 'en', entry.asset), '');
    writeFileSync(join(root, 'en', manifest.context_asset), '');
    writeFileSync(join(root, 'en/search-layers.json'), JSON.stringify(manifest));
    assert.equal(checkOutput(root).htmlPages, 2);
    for (const asset of ['https://off-origin.invalid/index.js', 'searchindex-layer-deos-87654321.js']) {
      writeFileSync(join(root, 'en/search-layers.json'), JSON.stringify({ ...manifest, indexes: { ...manifest.indexes, deos: { asset, sections: 0 } } }));
      assert.throws(() => checkOutput(root), /invalid native layer search asset|missing output target/);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('Unicode search preserves ASCII boundaries and adds Cyrillic without transliteration', () => {
  const ascii = token => token.replace(/^\W+/, '').replace(/\W+$/, '');
  for (const token of ['observations', '(ActorType)', '$BLDR', '[get_balance]', "it's", '123', '!!!', '']) assert.equal(trimSearchToken(token), ascii(token));
  for (const [token, expected] of [['«наблюдения»', 'наблюдения'], ['Ёмкость.', 'Ёмкость'], ['(café)', 'café'], ['«e\u0301»', 'e\u0301']]) assert.equal(trimSearchToken(token), expected);
  assert.throws(() => rebuildNativeSearch('', 'window.search = {};'), /Unsupported native/);
});

test('native section eligibility precedes any result cap and preserves sparse reference IDs', () => {
  const docs = Object.fromEntries(Array.from({ length: 33 }, (_, index) => [String(index), { body: 'membershipneedle' }]));
  const data = { index: { documentStore: { docs } }, doc_urls: Array.from({ length: 33 }, (_, index) => index === 32 ? 'shared.html#last' : `other.html#section-${index}`) };
  assert.equal(nativeSections(data).length, 33);
  assert.deepEqual(nativeSections(data, new Set(['shared.html'])), [['32', docs['32']]]);
  assert.deepEqual(nativeSections(data, new Set()), []);
  assert.throws(() => nativeSections(data, ['shared.html']), /must be a Set/);
  assert.throws(() => nativeSections({ ...data, doc_urls: [] }), /disagree/);
  assert.throws(() => nativeSections({ index: { documentStore: { docs: { x: {} } } }, doc_urls: ['shared.html'] }), /section reference/);
});

test('search rebinding changes only the native head assignment and preserves document text', () => {
  const head = '<script>const path_to_root = "../../";\nwindow.path_to_searchindex_js = "../../searchindex-old.js";</script>';
  const body = '<main><code>searchindex-old.js</code></main><script>const sample = "searchindex-old.js";</script>';
  const rebound = rebindSearchAsset(head + body, 'searchindex-old.js', 'searchindex-new.js');
  assert.equal(rebound.bindings, 1);
  assert.ok(rebound.content.endsWith(body));
  assert.ok(rebound.content.includes('../../searchindex-new.js'));
  assert.throws(() => rebindSearchAsset(head, 'different.js', 'new.js'), /Unexpected native/);
});

test('mdBook release and platform assets remain exact and unsupported targets fail closed', () => {
  assert.equal(mdbook.version, '0.5.4');
  for (const platform of Object.keys(mdbook.artifacts)) {
    const artifact = artifactFor(platform);
    assert.match(artifact.sha256, /^[a-f0-9]{64}$/);
    assert.ok(artifact.url.startsWith(`https://github.com/rust-lang/mdBook/releases/download/v${mdbook.version}/`));
  }
  assert.throws(() => artifactFor('unsupported'));
});
