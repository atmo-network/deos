/*
Domain: Deployment base-path browser regressions
Owns: Real reading, locale, layer, search and contents journeys under a served URL prefix.
Excludes: Root-served journeys, iframe embedding and production host header acceptance.
*/
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { withChrome } from './chrome.mjs';
import { serveStatic } from './serve.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../dist');
const graph = JSON.parse(readFileSync(join(root, 'graph.json'), 'utf8'));
const base = graph.base_path ?? '/';
const byId = new Map(graph.nodes.map(node => [node.id, node]));
const start = byId.get('deos/actor-system');
const found = byId.get('deos/typed-observations');

// Only meaningful for an artifact built with WIKI_BASE_PATH; root-served journeys own the default.
const run = base === '/' ? test.skip : test;

for (const [locale, other, size, query] of [['en', 'ru', [1280, 900], 'observations'], ['ru', 'en', [360, 800], 'наблюдения']]) {
  run(`${locale}: site served beneath ${base} keeps every journey inside its prefix`, { timeout: 60000 }, async t => {
    const requests = [], server = serveStatic(root, requests, new Map(), base);
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    const origin = `http://127.0.0.1:${server.address().port}`;
    try {
      await withChrome(size, t.signal, async browser => {
        // The site root redirects straight to the default landing inside the prefix.
        const landing = graph.nodes.find(node => node.id === 'deos/index');
        await browser.command('Page.navigate', { url: origin + base });
        await browser.waitFor(`location.pathname === ${JSON.stringify(landing.url[graph.default_locale])} && document.documentElement.dataset.wikiLayer === 'ready'`);

        await browser.navigate(origin + start.url[locale]);
        await browser.waitFor(`document.documentElement.dataset.wikiLayer === 'ready'`);
        assert.equal(await browser.evaluate('document.querySelector("main h1").textContent'), start.title[locale]);
        const switches = await browser.evaluate(`Array.from(document.querySelectorAll('.wiki-locale-link'), a => new URL(a.href).pathname)`);
        assert.ok(switches.includes(start.url[other]), 'locale switch lost the base path');
        const tag = await browser.evaluate(`(() => { const url = new URL(document.querySelector('main .wiki-tag').href); return [url.pathname, new URLSearchParams(url.hash.slice(2)).get('layer'), url.search]; })()`);
        assert.deepEqual(tag, [`${base}${locale}/index.html`, 'deos', ''], 'layer tag left the base path or wrote query state');
        const chapters = await browser.evaluate(`Array.from(document.querySelectorAll('.nav-chapters, .mobile-nav-chapters'), a => new URL(a.href).pathname)`);
        assert.ok(chapters.length > 0 && chapters.every(path => path.startsWith(`${base}${locale}/`)), 'chapter navigation left the base path');

        await browser.navigate(`${origin}${start.url[locale]}#!layer=deos`);
        await browser.waitFor(`document.documentElement.dataset.wikiLayer === 'ready'`);
        assert.equal(await browser.evaluate(`document.querySelector('.wiki-layer-choice[aria-current="true"]').dataset.layer`), 'deos');

        await browser.navigate(`${origin}${start.url[locale]}#!search=${encodeURIComponent(query)}`);
        await browser.waitFor('document.querySelectorAll("#mdbook-searchresults li > a").length > 0');
        const results = await browser.evaluate('Array.from(document.querySelectorAll("#mdbook-searchresults li > a"), a => new URL(a.href).pathname)');
        assert.ok(results.includes(found.url[locale]), 'search did not reach the owning page beneath the base path');
        await browser.evaluate(`Array.from(document.querySelectorAll('#mdbook-searchresults li > a')).find(a => new URL(a.href).pathname === ${JSON.stringify(found.url[locale])}).click()`);
        await browser.waitFor(`location.pathname === ${JSON.stringify(found.url[locale])} && document.readyState === 'complete'`);

        await browser.evaluate(`document.querySelector('main .wiki-tag').click()`);
        await browser.waitFor(`location.pathname === ${JSON.stringify(`${base}${locale}/index.html`)} && document.readyState === 'complete' && document.documentElement.dataset.wikiLayer === 'ready'`);
        const listed = await browser.evaluate('Array.from(document.querySelectorAll("main li > a")).filter(a => !a.closest("li").hidden).map(a => new URL(a.href).pathname)');
        assert.ok(listed.length > 0 && listed.every(path => path.startsWith(`${base}${locale}/`)), 'layer contents left the base path');

        assert.deepEqual(browser.errors, [], 'browser reported script/resource errors');
      });
      const stray = requests.filter(request => request.status !== 200 || !request.path.startsWith(base));
      assert.deepEqual(stray, [], 'a request left the base path or missed the artifact');
    } finally {
      await new Promise(resolve => server.close(resolve));
    }
  });
}
