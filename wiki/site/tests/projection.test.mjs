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
import { assertChromeIcons, assertPageIcons, basePath, loadCorpus, localizeLinks, pageRoute } from '../src/corpus.mjs';
import { contextScript } from '../src/context.mjs';
import { parseFrontmatter, splitFrontmatter } from '../src/frontmatter.mjs';
import { linearOrder, projectBooks, unifiedGraph } from '../src/projection.mjs';
import { selectLayer } from '../src/layers.mjs';
import { assertTrustedMarkdown, assertTrustedSvg } from '../src/trust.mjs';
import { artifactFor, mdbook } from '../scripts/toolchain.mjs';
import { checkOutput } from '../scripts/check-output.mjs';
import { renderMath } from '../src/math.mjs';
import { renderDiagrams } from '../src/mermaid.mjs';
import { trimSearchToken, rebindSearchAsset, rebuildNativeSearch, nativeSections } from '../src/search.mjs';

const corpus = loadCorpus();
const extended = { ...corpus, planes: [...corpus.planes, { id: 'example', title: 'Example fixture', pages: new Map(), graph: { edges: [] }, locales: [] }] };
const assets = { 'wiki.css': '', 'wiki.js': '' };
const projected = projectBooks(corpus, assets);

test('every page identity owns one unique glyph that the reader carries into the sidebar', () => {
  const graph = unifiedGraph(corpus, '/deos/');
  const glyphs = graph.nodes.map(node => node.icon);
  assert.equal(glyphs.length, graph.nodes.length);
  for (const [index, glyph] of glyphs.entries()) {
    assert.equal(typeof glyph, 'string', `${graph.nodes[index].canonical_page_id}: missing icon`);
    assert.ok(glyph.trim().length > 0 && glyph.trim().length <= 8, `${graph.nodes[index].canonical_page_id}: unusable icon`);
  }
  assert.equal(new Set(glyphs).size, glyphs.length, 'two pages never share one glyph');
  // The reader embeds a reduced graph; the glyph must survive that reduction or the sidebar is silent.
  const script = contextScript(graph, 'en', {});
  for (const glyph of glyphs) assert.ok(script.includes(glyph), `${glyph}: dropped from the reader context`);
});

test('the contents page and its groups own navigation glyphs that no article reuses', () => {
  const graph = unifiedGraph(corpus, '/deos/');
  assert.deepEqual(Object.keys(graph.chrome ?? {}).sort(), ['contents', 'group']);
  assert.notEqual(graph.chrome.contents, graph.chrome.group);
  const pageGlyphs = new Set(graph.nodes.map(node => node.icon));
  for (const glyph of Object.values(graph.chrome)) assert.ok(!pageGlyphs.has(glyph), `${glyph}: chrome glyph must not mark an article`);
  // The reader needs both: the contents entry is not a graph node, so the glyph travels separately.
  const script = contextScript(graph, 'en', {});
  for (const glyph of Object.values(graph.chrome)) assert.ok(script.includes(glyph), `${glyph}: dropped from the reader context`);
});

test('incomplete, colliding or ambiguous navigation glyphs are refused', () => {
  const pages = new Map([['alpha', '🏠'], ['beta', '🚪']]);
  assert.throws(() => assertChromeIcons('deos', { contents: '📚' }, pages), /must declare exactly contents\/group/);
  assert.throws(() => assertChromeIcons('deos', { contents: '📚', group: '🗂️', extra: '📦' }, pages), /must declare exactly contents\/group/);
  assert.throws(() => assertChromeIcons('deos', { contents: '📚', group: '📚' }, pages), /distinct icons/);
  assert.throws(() => assertChromeIcons('deos', { contents: '🏠', group: '🗂️' }, pages), /collides with a page icon/);
  assert.throws(() => assertChromeIcons('deos', { contents: '', group: '🗂️' }, pages), /invalid contents chrome icon/);
  assert.deepEqual(assertChromeIcons('deos', { contents: '📚', group: '🗂️' }, pages), { contents: '📚', group: '🗂️' });
});

test('a missing, duplicated or invented page glyph is refused', () => {
  const identities = new Set(['alpha', 'beta']);
  assert.throws(() => assertPageIcons('deos', identities, new Map([['alpha', '🏠']])), /icon\/page identity drift/);
  assert.throws(() => assertPageIcons('deos', identities, new Map([['alpha', '🏠'], ['beta', '🏠']])), /duplicate page icon/);
  assert.throws(() => assertPageIcons('deos', identities, new Map([['alpha', '🏠'], ['beta', ''], ['gamma', '🚪']])), /icon\/page identity drift/);
  assert.throws(() => assertPageIcons('deos', identities, new Map([['alpha', '🏠'], ['beta', '  ']])), /invalid icon for beta/);
  assert.deepEqual([...assertPageIcons('deos', identities, new Map([['alpha', '🏠'], ['beta', '🚪']])).keys()], ['alpha', 'beta']);
});

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

test('a titled navigation group needs three pages; an untitled run lists them at plane level', () => {
  // The composition rule rejects a titled group of fewer than three pages and the projection lists
  // an untitled run flat, so this fixture demotes the last two pages of one real group instead of
  // depending on the live corpus happening to declare an untitled section.
  const plane = structuredClone(corpus.planes.find(item => item.pages.size > 0));
  const group = plane.navigation.sections.find(section => section.title && section.items.length >= 5);
  assert.ok(group, 'fixture needs one titled group of at least five pages');
  const moved = group.items.splice(group.items.length - 2, 2);
  plane.navigation.sections.push({ id: 'plane-flat-run', items: moved });
  const book = projectBooks({ ...corpus, planes: corpus.planes.map(item => item.id === plane.id ? plane : item) }, assets).files.get('books/en/src/SUMMARY.md');
  assert.ok(!book.includes('plane-flat-run'), 'an untitled run must not own a foldable node');
  for (const item of moved) {
    assert.ok(new RegExp(`\\n- \\[${item.title.en}\\]\\(`).test(book), `untitled page ${item.id} must sit at plane level`);
  }
});

test('unplaced graph pages fail rather than acquiring an invented priority', () => {
  const plane = structuredClone(corpus.planes.find(item => item.pages.size > 0));
  const removed = plane.navigation.sections[0].items[0].id;
  for (const section of plane.navigation.sections) section.items = section.items.filter(item => item.id !== removed);
  assert.throws(() => linearOrder(plane, plane.defaultLocale), /explicit navigation position/);
});

test('every locale is a projection of the same branch with source Markdown unchanged', () => {
  // Two rewrites are contractual: link destinations are localized and notation becomes typeset
  // markup. Anything else must leave the authored bytes alone.
  for (const plane of corpus.planes) for (const locale of plane.locales) {
    const summary = projected.files.get(`books/${locale}/src/SUMMARY.md`);
    let position = -1;
    for (const id of linearOrder(plane, plane.defaultLocale)) {
      const page = plane.pages.get(`${locale}/${id}`);
      const path = `${plane.id}/${page.sourcePath.replace(`.${locale}.md`, '.md')}`;
      const next = summary.indexOf(`](${path})`);
      assert.ok(next > position, `${locale}: ${id} changed metadata order`);
      position = next;
      const markdown = projected.files.get(`books/${locale}/src/${path}`);
      const expected = renderDiagrams(renderMath(localizeLinks(page.body, locale)).markdown, path).markdown;
      assert.ok(markdown.includes(expected.trimEnd()), `${path}: authored body was rewritten beyond link destinations, notation and diagram placeholders`);
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
    // The only compiler HTML is the content-free site-root redirect; every document is rendered by mdBook.
    if (path === 'site-index.html') {
      assert.doesNotMatch(content, /<main|<h1|<article|<table/);
      assert.match(content, /location\.replace/);
      continue;
    }
    assert.ok(!path.endsWith('.html'), `${path}: a second HTML renderer appeared`);
    if (path.endsWith('book.toml')) {
      assert.match(content, /use-default-preprocessors = false/);
      assert.match(content, /create-missing = false/);
      assert.match(content, /runnable = false/);
      assert.match(content, /sidebar-header-nav = false/);
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
  assert.equal(pageRoute('ru', 'deos', 'overview/actor-system.ru.md'), '/ru/deos/overview/actor-system.html');
  for (const value of ['../secret.md', '/root.md', 'folder/../../secret.md', 'a%2fb.md', 'a\\b.md', 'javascript:test.md']) assert.throws(() => pageRoute('en', 'deos', value));
  assert.throws(() => pageRoute('../ru', 'deos', 'index.md'));
});

test('a deployment base path prefixes every projected route without changing storage identity', () => {
  assert.equal(basePath(), '/');
  assert.equal(pageRoute('en', 'deos', 'overview/actor-system.en.md', '/deos/'), '/deos/en/deos/overview/actor-system.html');
  for (const value of ['', 'deos', 'deos/', '/deos', '//deos/', '/../', '/a/./', '/a b/', '/a/../b/', 'https://x/']) assert.throws(() => basePath(value), /base path/);
  const root = projectBooks(corpus, {});
  const prefixed = projectBooks(corpus, {}, '/deos/');
  assert.equal(root.graph.base_path, '/');
  assert.equal(prefixed.graph.base_path, '/deos/');
  assert.deepEqual(prefixed.graph.nodes.map(node => node.id), root.graph.nodes.map(node => node.id));
  for (const node of prefixed.graph.nodes) for (const [locale, url] of Object.entries(node.url)) assert.equal(url, '/deos' + root.graph.nodes.find(other => other.id === node.id).url[locale]);
  for (const locale of prefixed.locales) {
    // Only the native 404 page needs the prefix; every other link is relative and location-independent.
    assert.match(prefixed.files.get(`books/${locale}/book.toml`), new RegExp(`site-url = "/deos/${locale}/"`));
    assert.ok(!prefixed.files.has(`books/${locale}/src/graph.md`), 'the knowledge graph stays behind the reader UI');
    assert.ok(prefixed.files.get(`books/${locale}/src/index.md`).includes('(deos/index.'));
    assert.ok(prefixed.files.get(`books/${locale}/theme/header.hbs`).includes('href="{{ path_to_root }}../'));
    for (const [path, content] of prefixed.files) if (path.startsWith(`books/${locale}/src/`)) assert.doesNotMatch(content, /\]\(\//, `${path}: root-absolute link`);
  }
  assert.equal(prefixed.files.get('site-index.html'), root.files.get('site-index.html'));
  assert.match(prefixed.files.get('books/root/book.toml'), /site-url = "\/deos\/"/);
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

// The corpus expresses ordinary diagrams as Mermaid, but a custom infographic stays an authored SVG,
// so the SVG path keeps its own witness instead of depending on a page that happens to use it.
const trustedSvg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">\n  <path d="M0 0L10 10" fill="none" stroke="#000000" stroke-width="1"/>\n</svg>';

test('Mermaid blocks become inert placeholders and only a vendored syntax is admitted', () => {
  const result = renderDiagrams("```mermaid\nflowchart LR\n  A[\"One\"] --> B[\"Two\"]\n```\n", 'fixture.md');
  assert.equal(result.hasDiagram, true);
  assert.equal(result.count, 1);
  assert.match(result.markdown, /<div class="wiki-mermaid" data-diagram="1">flowchart LR/);
  assert.match(result.markdown, /^<figure class="wiki-diagram">$/m);
  // The source is escaped, so an authored angle bracket cannot become markup.
  assert.match(renderDiagrams("```mermaid\nflowchart LR\n A[\"<b>x</b>\"] --> B\n```\n", 'x.md').markdown, /&lt;b&gt;x&lt;\/b&gt;/);
  assert.match(renderDiagrams("```mermaid\ngraph TD\n A-->B\n```\n", 'x.md').markdown, /wiki-mermaid/);
  assert.throws(() => renderDiagrams("```mermaid\nsequenceDiagram\n A->>B: hi\n```\n", 'x.md'), /not vendored/);
  assert.throws(() => renderDiagrams("```mermaid\nflowchart LR\n```\n", 'x.md'), /needs a syntax and at least one statement/);
  assert.equal(renderDiagrams('plain text\n', 'x.md').hasDiagram, false);
});

test('only a book with a diagram loads the Mermaid renderer', () => {
  assert.match(projected.files.get('books/en/book.toml'), /theme\/wiki-mermaid\.js/, 'the corpus ships a diagram, so the loader is listed');
  assert.doesNotMatch(projected.files.get('books/root/book.toml'), /wiki-mermaid\.js/);
  assert.ok(projected.files.has('books/en/theme/wiki-mermaid.js'));
});

test('only a book with pseudocode loads its language definition', () => {
  assert.match(projected.files.get('books/en/book.toml'), /theme\/wiki-code\.js/);
  assert.doesNotMatch(projected.files.get('books/root/book.toml'), /wiki-code\.js/);
  assert.ok(projected.files.has('books/en/theme/wiki-code.js'));
  assert.match(projected.files.get('books/en/src/deos/overview/router.md'), /^\x60{3}pseudocode$/m);
});

test('the Actor diagram is inert and projected next to both book chapters', () => {
  const relative = 'deos/concepts/actor-wakeup.svg';
  const svg = projected.files.get(`books/en/src/${relative}`) ?? trustedSvg;
  assert.doesNotThrow(() => assertTrustedSvg(svg));
  assert.doesNotThrow(() => assertTrustedSvg(trustedSvg));
  assert.match(trustedSvg, /<svg xmlns/);
  for (const source of [
    svg.replace('<svg ', '<svg onload="run()" '),
    svg.replace('</svg>', '  <script/>\n</svg>'),
    svg.replace('</svg>', '  <image href="https://example.invalid/resource.svg"/>\n</svg>'),
    '<!DOCTYPE svg>\n' + svg,
  ]) assert.throws(() => assertTrustedSvg(source), /expected an inert local SVG diagram/);
});

test('native output validation refuses a missing localized SVG at the Pages prefix', () => {
  const root = mkdtempSync(join(tmpdir(), 'deos-mdbook-diagram-'));
  try {
    writeFileSync(join(root, 'graph.json'), JSON.stringify({
      base_path: '/deos/', nodes: [], edges: [], linear_order: [],
    }));
    for (const locale of ['en', 'ru']) {
      const chapter = join(root, locale, 'deos', 'concepts');
      mkdirSync(chapter, { recursive: true });
      writeFileSync(join(chapter, 'flow.html'), '<img src="actor-wakeup.svg" alt="Diagram">');
    }
    const svg = trustedSvg;
    writeFileSync(join(root, 'en/deos/concepts/actor-wakeup.svg'), svg);
    assert.throws(() => checkOutput(root), /ru\/deos\/concepts\/flow\.html: missing output target \/deos\/ru\/deos\/concepts\/actor-wakeup\.svg/);
    writeFileSync(join(root, 'ru/deos/concepts/actor-wakeup.svg'), svg);
    assert.equal(checkOutput(root).localTargets, 2);
  } finally { rmSync(root, { recursive: true, force: true }); }
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
