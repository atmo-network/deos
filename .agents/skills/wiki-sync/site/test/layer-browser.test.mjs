/*
Domain: Layered reader browser acceptance
Owns: URL-selected native views, empty/error recovery and isolated mixed-membership journeys.
Excludes: Published downstream knowledge, replacement documents, iframe compatibility and live hosting.
*/
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { assignMemberships, selectLayer } from '../src/layers.mjs';
import { contextScript } from '../src/context.mjs';
import { rebuildNativeSearch } from '../src/search.mjs';
import { withChrome } from './chrome.mjs';
import { serveStatic } from './serve.mjs';
import { loadNative, nativeLibrary } from './native.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../dist');
const screenshots = resolve(root, '../target/browser-smoke');
const graph = JSON.parse(readFileSync(join(root, 'graph.json'), 'utf8'));
const start = graph.nodes.find(node => node.id === 'deos/actor-system');
const shared = graph.nodes.find(node => node.id === 'deos/typed-observations');
const position = id => graph.linear_order.indexOf(id);
const relation = graph.edges.find(edge => (edge.from === shared.id || edge.to === shared.id) && Math.abs(position(edge.from) - position(edge.to)) > 1);
assert.ok(relation, 'mixed fixture needs an authored relation across a linear gap');
const memberIds = [relation.from, relation.to];
const fixtureLayers = [...graph.layers, { id: 'example', title: 'Example fixture' }];
const classify = ids => ({ ...graph, layers: fixtureLayers, nodes: assignMemberships(graph.nodes, fixtureLayers, Object.fromEntries(graph.nodes.map(node => [node.id, ids.includes(node.id) ? ['deos', 'example'] : ['example']]))) });
const mixed = classify(memberIds);
const empty = classify([]);
const members = mixed.nodes.filter(node => memberIds.includes(node.id));
const selected = selectLayer(mixed, 'deos');

function projectionData(model) {
  const overlays = new Map();
  for (const locale of graph.locales) {
    const directory = join(root, locale);
    const manifest = JSON.parse(readFileSync(join(directory, 'search-layers.json'), 'utf8'));
    const library = nativeLibrary(directory);
    const original = loadNative(library, readFileSync(join(directory, manifest.indexes.all.asset), 'utf8')).data;
    original.index.pipeline = original.index.pipeline.map(step => step === 'wikiUnicodeTrim' ? 'trimmer' : step);
    const documents = new Set(selectLayer(model, 'deos').nodes.map(node => node.url[locale].slice(locale.length + 2)));
    const rebuilt = rebuildNativeSearch(library, `window.search = ${JSON.stringify(original)};`, documents);
    const asset = `searchindex-layer-deos-${createHash('sha256').update(rebuilt.script).digest('hex').slice(0, 8)}.js`;
    const indexes = { ...manifest.indexes, example: manifest.indexes.deos, deos: { asset, sections: rebuilt.sections } };
    // Only classification/search data is overlaid; HTML and controller code are the actual built site.
    overlays.set(`/${locale}/${manifest.context_asset}`, contextScript(model, locale, indexes));
    overlays.set(`/${locale}/${asset}`, rebuilt.script);
  }
  return overlays;
}

async function fixture(overlays, callback) {
  const requests = [], server = serveStatic(root, requests, overlays);
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try { await callback(`http://127.0.0.1:${server.address().port}`, requests); }
  finally { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
}

async function ready(browser, path, state = 'ready') {
  await browser.waitFor(`location.pathname === ${JSON.stringify(path)} && document.readyState === 'complete' && document.documentElement.dataset.wikiLayer === ${JSON.stringify(state)}`);
}

for (const [locale, size, query] of [['en', [1280, 900], 'observations'], ['ru', [360, 800], 'наблюдения']]) {
  test(`${locale}: layer controls, empty search/graph, refresh and language entry preserve URL context`, { timeout: 60000 }, async t => {
    mkdirSync(screenshots, { recursive: true });
    await fixture(projectionData(empty), async (origin, requests) => {
      await withChrome(size, t.signal, async browser => {
        await browser.navigate(`${origin}${start.url[locale]}?layer=all&tag=a%3Db&tag=x%26y`);
        await ready(browser, start.url[locale]);
        assert.equal(await browser.evaluate('document.querySelector(".wiki-layer-choice[aria-current]").dataset.layer'), 'all');

        await browser.evaluate('document.querySelector(".wiki-layer-choice[data-layer=deos]").click()');
        await ready(browser, `/${locale}/index.html`);
        assert.equal(await browser.evaluate('new URL(location.href).searchParams.get("layer")'), 'deos');
        assert.deepEqual(await browser.evaluate('new URL(location.href).searchParams.getAll("tag")'), ['a=b', 'x&y']);
        assert.equal(await browser.evaluate('window.wikiContext.view.nodes.length'), 0);
        assert.equal(await browser.evaluate('Array.from(document.querySelectorAll("main li")).filter(item => !item.hidden).length'), 0);
        assert.equal(await browser.evaluate('Array.from(document.querySelectorAll("#mdbook-menu-bar a")).some(a => new URL(a.href).pathname.endsWith("/print.html"))'), false);
        await browser.evaluate('document.querySelector("#mdbook-search-toggle").click()');
        await browser.command('Input.insertText', { text: query });
        await browser.command('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Unidentified' });
        await browser.waitFor('!!document.querySelector("#mdbook-searchresults-header").textContent && !document.querySelector("#mdbook-searchbar-outer").classList.contains("searching")');
        assert.equal(await browser.evaluate('document.querySelectorAll("#mdbook-searchresults li").length'), 0);
        const saved = await browser.evaluate('location.href');
        await browser.navigate(saved, true);
        await ready(browser, `/${locale}/index.html`);
        await browser.waitFor('!!document.querySelector("#mdbook-searchresults-header").textContent && !document.querySelector("#mdbook-searchbar-outer").classList.contains("searching")');
        assert.equal(await browser.evaluate('document.querySelectorAll("#mdbook-searchresults li").length'), 0);
        assert.ok(await browser.evaluate('document.documentElement.scrollWidth <= innerWidth'));
        await browser.screenshot(join(screenshots, `${locale}-layer-empty.png`));
        await browser.evaluate(`Array.from(document.querySelectorAll('main a')).find(a => new URL(a.href).pathname === '/${locale}/graph.html').click()`);
        await ready(browser, `/${locale}/graph.html`);
        assert.equal(await browser.evaluate('new URL(location.href).searchParams.get("layer")'), 'deos');
        assert.equal(await browser.evaluate('Array.from(document.querySelectorAll("main tbody tr")).filter(row => !row.hidden).length'), 0);
        assert.ok(await browser.evaluate('!!document.querySelector(".wiki-graph-empty")'));
        await browser.navigate(`${origin}/?layer=deos`);
        await ready(browser, '/');
        await browser.evaluate(`Array.from(document.querySelectorAll('.wiki-locale-link')).find(a => new URL(a.href).pathname === '/${locale}/index.html').click()`);
        await ready(browser, `/${locale}/index.html`);
        assert.equal(await browser.evaluate('window.wikiContext.selection'), 'deos');
        assert.deepEqual(browser.errors, []);
      });
      assert.ok(!requests.some(request => request.status !== 200));
    });
  });

  test(`${locale}: invalid/outside URLs and missing metadata fail explicitly without rewriting history`, { timeout: 60000 }, async t => {
    const manifest = JSON.parse(readFileSync(join(root, locale, 'search-layers.json'), 'utf8'));
    const unavailable = `/${locale}/${manifest.context_asset}`;
    await fixture(projectionData(empty), async origin => {
      await withChrome(size, t.signal, async browser => {
        await browser.command('Page.addScriptToEvaluateOnNewDocument', { source: 'window.__historyAtLoad = history.length;' });
        for (const query of ['layer=unknown', 'layer=deos&layer=example']) {
          const href = `${origin}${start.url[locale]}?${query}&tag=keep`;
          await browser.navigate(href);
          await ready(browser, start.url[locale], 'error');
          assert.equal(await browser.evaluate('location.href'), href);
          assert.equal(await browser.evaluate('history.length'), await browser.evaluate('window.__historyAtLoad'), 'restoration changed history');
          assert.ok(await browser.evaluate('getComputedStyle(document.querySelector(".content")).display === "none"'));
          assert.ok(await browser.evaluate('document.querySelector("#mdbook-search-toggle").disabled'));
          assert.equal(await browser.evaluate('document.querySelectorAll(".nav-chapters, .mobile-nav-chapters").length'), 0);
          await browser.command('Input.dispatchKeyEvent', { type: 'keyDown', key: 'ArrowRight' });
          assert.equal(await browser.evaluate('location.href'), href);
          const recovery = await browser.evaluate('document.querySelector(".wiki-layer-recovery").href');
          assert.equal(new URL(recovery).searchParams.has('layer'), false);
          assert.equal(new URL(recovery).searchParams.get('tag'), 'keep');
        }
        const outside = `${origin}${start.url[locale]}?layer=deos`;
        await browser.navigate(outside);
        await ready(browser, start.url[locale], 'outside');
        assert.equal(await browser.evaluate('location.href'), outside);
        assert.ok(await browser.evaluate('getComputedStyle(document.querySelector(".content")).display === "none"'));
        await browser.evaluate('document.querySelector(".wiki-layer-recovery").click()');
        await ready(browser, start.url[locale]);
        assert.equal(await browser.evaluate('window.wikiContext.selection'), 'all');
        assert.deepEqual(browser.errors, []);
      });
    });
    await fixture(new Map([[unavailable, null]]), async (origin, requests) => {
      await withChrome(size, t.signal, async browser => {
        await browser.navigate(`${origin}${start.url[locale]}?layer=deos`);
        await ready(browser, start.url[locale], 'error');
        assert.ok(await browser.evaluate('getComputedStyle(document.querySelector(".content")).display === "none"'));
        assert.ok(await browser.evaluate('document.querySelector("#mdbook-search-toggle").disabled'));
        assert.ok(!browser.errors.some(error => error.method === 'Runtime.exceptionThrown'), JSON.stringify(browser.errors));
      });
      assert.deepEqual(requests.filter(request => request.status !== 200).map(request => request.path), [unavailable]);
    });
  });

  test(`${locale}: isolated shared membership drives one reader, graph, keyboard order and explicit exits`, { timeout: 60000 }, async t => {
    await fixture(projectionData(mixed), async (origin, requests) => {
      await withChrome(size, t.signal, async browser => {
        const first = members[0], second = members[1];
        await browser.navigate(`${origin}${first.url[locale]}?layer=deos`);
        await ready(browser, first.url[locale]);
        assert.equal(await browser.evaluate('window.wikiContext.view.nodes.length'), 2);
        const visible = await browser.evaluate('Array.from(document.querySelectorAll(".chapter > li > .chapter-link-wrapper > a")).filter(a => !a.closest("li").hidden).map(a => new URL(a.href).pathname)');
        assert.deepEqual(visible, [`/${locale}/index.html`, ...members.map(node => node.url[locale]), `/${locale}/graph.html`]);
        const nextLinks = await browser.evaluate('Array.from(document.querySelectorAll(".nav-chapters.next, .mobile-nav-chapters.next"), link => ({path: new URL(link.href).pathname, layer: new URL(link.href).searchParams.get("layer")}))');
        assert.equal(nextLinks.length, 2);
        assert.ok(nextLinks.every(link => link.path === second.url[locale] && link.layer === 'deos'));
        await browser.command('Input.dispatchKeyEvent', { type: 'keyDown', key: 'ArrowRight' });
        await ready(browser, second.url[locale]);
        assert.equal(await browser.evaluate('window.wikiContext.selection'), 'deos');
        const other = locale === 'ru' ? 'en' : 'ru';
        await browser.evaluate(`Array.from(document.querySelectorAll('.wiki-locale-link')).find(a => new URL(a.href).pathname === ${JSON.stringify(second.url[other])}).click()`);
        await ready(browser, second.url[other]);
        assert.equal(await browser.evaluate('window.wikiContext.selection'), 'deos');
        await browser.navigate(`${origin}/${locale}/graph.html?layer=deos`);
        await ready(browser, `/${locale}/graph.html`);
        assert.equal(await browser.evaluate('Array.from(document.querySelectorAll("main tbody tr")).filter(row => !row.hidden).length'), selected.edges.length);
        const endpoints = await browser.evaluate('Array.from(document.querySelectorAll("main tbody tr")).filter(row => !row.hidden).map(row => Array.from(row.querySelectorAll("a"), a => ({path: new URL(a.href).pathname, layer: new URL(a.href).searchParams.get("layer")})))');
        assert.ok(endpoints.every(row => row.every(endpoint => members.some(node => node.url[locale] === endpoint.path) && endpoint.layer === 'deos')));
        await browser.navigate(`${origin}${shared.url[locale]}?layer=deos&search=${encodeURIComponent(query)}&tag=one&tag=two`);
        await ready(browser, shared.url[locale]);
        await browser.waitFor('document.querySelectorAll("#mdbook-searchresults li > a").length > 0');
        const results = await browser.evaluate('Array.from(document.querySelectorAll("#mdbook-searchresults li > a"), a => ({path: new URL(a.href).pathname, layer: new URL(a.href).searchParams.get("layer"), tags: new URL(a.href).searchParams.getAll("tag")}))');
        assert.ok(results.every(result => members.some(node => node.url[locale] === result.path) && result.layer === 'deos'));
        assert.ok(results.every(result => result.tags.join(',') === 'one,two'));
        await browser.evaluate('document.querySelector("main h2 a.header").click()');
        assert.equal(await browser.evaluate('new URL(location.href).searchParams.get("layer")'), 'deos');
        assert.ok(await browser.evaluate('!!location.hash && !!document.getElementById(decodeURIComponent(location.hash.slice(1)))'));
        const deep = await browser.evaluate('location.href');
        await browser.navigate(deep, true);
        await ready(browser, shared.url[locale]);
        assert.equal(await browser.evaluate('window.wikiContext.selection'), 'deos');
        assert.ok(await browser.evaluate('!!document.getElementById(decodeURIComponent(location.hash.slice(1)))'));
        for (const width of [360, 768, 1280]) {
          await browser.command('Emulation.setDeviceMetricsOverride', { width, height: 800, deviceScaleFactor: 1, mobile: false });
          assert.ok(await browser.evaluate('document.documentElement.scrollWidth <= innerWidth'), `${width}px overflow`);
        }
        await browser.command('Emulation.setDeviceMetricsOverride', { width: size[0], height: size[1], deviceScaleFactor: 1, mobile: false });
        await browser.evaluate('scrollTo(0, 0)');
        await browser.screenshot(join(screenshots, `${locale}-layer-shared.png`));
        const exit = await browser.evaluate('Array.from(document.querySelectorAll("main a[data-wiki-layer-exit]")).find(a => a.getClientRects().length > 0)?.href');
        assert.ok(exit, 'shared page must expose a visible, labeled outside-layer link');
        assert.equal(new URL(exit).searchParams.has('layer'), false);
        await browser.evaluate(`Array.from(document.querySelectorAll('main a[data-wiki-layer-exit]')).find(a => a.href === ${JSON.stringify(exit)}).click()`);
        await ready(browser, new URL(exit).pathname);
        assert.equal(await browser.evaluate('window.wikiContext.selection'), 'all');
        assert.deepEqual(browser.errors, []);
      });
      assert.ok(!requests.some(request => request.status !== 200));
    });
  });

  test(`${locale}: no-script baseline explicitly remains unfiltered`, { timeout: 60000 }, async t => {
    await fixture(new Map(), async (origin, requests) => {
      await withChrome(size, t.signal, async browser => {
        await browser.command('Emulation.setScriptExecutionDisabled', { value: true });
        await browser.command('Page.navigate', { url: `${origin}${start.url[locale]}?layer=deos` });
        await browser.waitFor(`document.readyState === 'complete' && document.querySelector('main h1')?.textContent === ${JSON.stringify(start.title[locale])}`);
        assert.match(await browser.evaluate('document.querySelector(".wiki-basic-notice").textContent'), locale === 'ru' ? /Без него доступны все материалы/ : /reading is unfiltered/);
        assert.equal(await browser.evaluate('document.querySelector(".wiki-layer-choice")'), null);
        assert.equal(await browser.evaluate('document.querySelector(".sidebar-iframe-outer").title'), locale === 'ru' ? 'Оглавление' : 'Table of contents');
        assert.ok(await browser.evaluate('getComputedStyle(document.querySelector("main")).visibility !== "hidden"'));
        assert.deepEqual(browser.errors, []);
      });
      assert.ok(!requests.some(request => request.status !== 200));
    });
  });
}
