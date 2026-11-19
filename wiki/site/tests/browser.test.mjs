/*
Domain: Standalone mdBook browser regressions
Owns: Real static serving, EN/RU reading/search/sidebar journeys and owned fixture teardown.
Excludes: Iframe/client integration, reader-wide layer controls and production hosting acceptance.
*/
import assert from 'node:assert/strict';
import { mkdirSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { withChrome } from './chrome.mjs';
import { serveStatic as serve } from './serve.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../dist');
const screenshots = resolve(root, '../target/browser-smoke');
const graph = JSON.parse(readFileSync(join(root, 'graph.json'), 'utf8'));
const byId = new Map(graph.nodes.map(node => [node.id, node]));
const start = byId.get('deos/actor-system');
const edge = graph.edges.find(edge => edge.from === start.id && edge.to !== start.id);
const destination = byId.get(edge.to);

const serveStatic = requests => serve(root, requests);

for (const [locale, size, query] of [['en', [1280, 900], 'observations'], ['ru', [360, 800], 'наблюдения']]) {
  test(`${locale}: native reader, full-text search, sidebar and locale navigation`, { timeout: 60000 }, async t => {
    mkdirSync(screenshots, { recursive: true });
    const requests = [], server = serveStatic(requests);
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    const origin = `http://127.0.0.1:${server.address().port}`;
    try {
      await withChrome(size, t.signal, async browser => {
        await browser.navigate(origin + start.url[locale]);
        assert.equal(await browser.evaluate('document.documentElement.lang'), locale);
        assert.equal(await browser.evaluate('document.querySelector("main h1").textContent'), start.title[locale]);
        assert.ok(await browser.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'document overflowed its viewport');
        await browser.screenshot(join(screenshots, `${locale}.png`));

        await browser.evaluate('document.querySelector("#mdbook-search-toggle").click()');
        await browser.command('Input.insertText', { text: query });
        await browser.waitFor('!!document.querySelector("#mdbook-searchresults-header").textContent && !document.querySelector("#mdbook-searchbar-outer").classList.contains("searching")');
        const results = await browser.evaluate('Array.from(document.querySelectorAll("#mdbook-searchresults li > a"), a => ({path: new URL(a.href).pathname, text: a.textContent}))');
        assert.ok(results.length > 0, `${locale}: no native full-text results for ${query}`);
        assert.ok(results.some(result => result.path === byId.get('deos/typed-observations').url[locale]), `${locale}: the owning knowledge page was not found`);
        assert.equal(await browser.evaluate('new URLSearchParams(location.hash.slice(2)).get("search")'), query);
        assert.equal(await browser.evaluate('location.search'), '', 'search state must stay out of the server-facing query');
        await browser.screenshot(join(screenshots, `${locale}-search.png`));

        await browser.navigate(`${origin}${start.url[locale]}#!search=${encodeURIComponent(query)}`);
        await browser.waitFor('document.querySelectorAll("#mdbook-searchresults li > a").length > 0');
        const keyboardUrl = await browser.evaluate('document.querySelector("#mdbook-searchresults li > a").href');
        await browser.command('Input.dispatchKeyEvent', { type: 'keyDown', key: 'ArrowDown' });
        await browser.waitFor('!!document.querySelector("#mdbook-searchresults li.focus > a")');
        await browser.command('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter' });
        await browser.waitFor(`location.href === ${JSON.stringify(keyboardUrl)} && document.readyState === 'complete' && document.documentElement.dataset.wikiChrome === 'ready'`);

        await browser.navigate(`${origin}${start.url[locale]}#!search=deosmissingtokenxyz`);
        await browser.waitFor('document.querySelector("#mdbook-searchresults-header").textContent.includes("deosmissingtokenxyz") && !document.querySelector("#mdbook-searchbar-outer").classList.contains("searching")');
        assert.equal(await browser.evaluate('document.querySelectorAll("#mdbook-searchresults li").length'), 0, 'absent query fabricated a result');
        await browser.navigate(`${origin}${start.url[locale]}#!search=${encodeURIComponent(query)}`);
        await browser.waitFor('document.querySelectorAll("#mdbook-searchresults li > a").length > 0');

        await browser.evaluate(`Array.from(document.querySelectorAll('#mdbook-searchresults li > a')).find(a => new URL(a.href).pathname === ${JSON.stringify(byId.get('deos/typed-observations').url[locale])}).click()`);
        await browser.waitFor(`location.pathname === ${JSON.stringify(byId.get('deos/typed-observations').url[locale])} && document.readyState === 'complete' && document.querySelector('main h1')?.textContent === ${JSON.stringify(byId.get('deos/typed-observations').title[locale])}`);
        const deepUrl = await browser.evaluate('location.href');
        await browser.navigate(deepUrl, true);
        assert.equal(await browser.evaluate('document.querySelector("main h1").textContent'), byId.get('deos/typed-observations').title[locale]);

        // The knowledge graph stays behind the reader UI: no graph page, chapter or per-article relation block.
        assert.ok(await browser.evaluate(`!Array.from(document.querySelectorAll('a[href]')).some(a => new URL(a.href).pathname.endsWith('/graph.html'))`), 'a knowledge-graph link reached the reader');
        assert.ok(await browser.evaluate(`!Array.from(document.querySelectorAll('main h2')).some(h => ['Knowledge graph', 'Граф знаний'].includes(h.textContent.trim()))`), 'a relation block reached the article');
        await browser.evaluate(`Array.from(document.querySelectorAll('.sidebar-scrollbox a[href]')).find(a => new URL(a.href).pathname === ${JSON.stringify(destination.url[locale])}).click()`);
        await browser.waitFor(`location.pathname === ${JSON.stringify(destination.url[locale])} && document.querySelector('main h1')?.textContent === ${JSON.stringify(destination.title[locale])}`);
        await browser.evaluate('document.querySelector("main h2 a.header").click()');
        await browser.waitFor('!!location.hash');
        assert.ok(await browser.evaluate('!!document.getElementById(decodeURIComponent(location.hash.slice(1)))'), 'native heading fragment did not resolve');
        // A standalone page offers no print or open-in-new-tab chrome.
        assert.deepEqual(await browser.evaluate('[!!document.querySelector(".wiki-open-current"), !!document.querySelector("#mdbook-print-button, a[href$=\\"print.html\\"]")]'), [false, false]);

        const other = locale === 'ru' ? 'en' : 'ru';
        await browser.evaluate(`Array.from(document.querySelectorAll('.wiki-locale-link')).find(a => new URL(a.href).pathname === ${JSON.stringify(destination.url[other])}).click()`);
        await browser.waitFor(`document.documentElement.lang === '${other}' && document.querySelector('main h1')?.textContent === ${JSON.stringify(destination.title[other])}`);
        assert.equal(await browser.evaluate('location.pathname'), destination.url[other]);



        assert.deepEqual(graph.layers.map(layer => layer.id), ['deos']);
        // A single declared layer needs no filter bar; every article still shows its layer tag.
        assert.deepEqual(await browser.evaluate('Array.from(document.querySelectorAll(".wiki-layer-choice"), item => item.dataset.layer)'), []);
        assert.equal(await browser.evaluate('document.querySelector(".wiki-context")'), null);
        assert.deepEqual(await browser.evaluate('Array.from(document.querySelectorAll("main .wiki-tag"), tag => tag.textContent)'), ['#deos']);
        assert.deepEqual(browser.errors, [], 'browser reported script/resource errors');
      });
      assert.ok(requests.some(request => /searchindex-.*\.js$/.test(request.path)), 'native search index was never requested');
      assert.ok(!requests.some(request => request.status !== 200), JSON.stringify(requests.filter(request => request.status !== 200)));
    } finally {
      server.closeAllConnections();
      await new Promise(resolve => server.close(resolve));
    }
  });

  test(`${locale}: native search consumes prepared scope assets before rendering results`, { timeout: 60000 }, async t => {
    const manifest = JSON.parse(readFileSync(join(root, locale, 'search-layers.json'), 'utf8'));
    const requests = [], server = serveStatic(requests);
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    const origin = `http://127.0.0.1:${server.address().port}`;
    try {
      await withChrome(size, t.signal, async browser => {
        for (const layer of graph.layers.map(layer => layer.id)) {
          await browser.navigate(`${origin}/${locale}/index.html#!layer=${layer}`);
          assert.equal(await browser.evaluate('window.wikiContext.selection'), layer);
          await browser.evaluate('document.querySelector("#mdbook-search-toggle").click()');
          await browser.command('Input.insertText', { text: query });
          await browser.waitFor('!!document.querySelector("#mdbook-searchresults-header").textContent && !document.querySelector("#mdbook-searchbar-outer").classList.contains("searching")');
          const results = await browser.evaluate('Array.from(document.querySelectorAll("#mdbook-searchresults li > a"), a => new URL(a.href).pathname)');
          if (!graph.nodes.some(node => node.layers.includes(layer))) assert.deepEqual(results, [], 'an empty scope leaked unfiltered results');
          else {
            assert.ok(results.length > 0);
            assert.ok(results.every(path => graph.nodes.some(node => node.layers.includes(layer) && node.url[locale] === path)), 'scoped search returned synthetic navigation or an ineligible page');
            assert.ok(results.includes(byId.get('deos/typed-observations').url[locale]));
          }
        }
        assert.deepEqual(browser.errors, []);
      });
      for (const layer of graph.layers.map(layer => layer.id)) assert.ok(requests.some(request => request.path === `/${locale}/${manifest.indexes[layer].asset}`));
      assert.ok(!requests.some(request => request.path === `/${locale}/${manifest.indexes.all.asset}`), 'scope fixture accidentally loaded the full index');
      assert.ok(!requests.some(request => request.status !== 200));
    } finally {
      server.closeAllConnections();
      await new Promise(resolve => server.close(resolve));
    }
  });

  test(`${locale}: punctuation stays inside the search value and survives refresh`, { timeout: 60000 }, async t => {
    const requests = [], server = serveStatic(requests);
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    const base = `http://127.0.0.1:${server.address().port}${start.url[locale]}#!layer=deos&tag=a%3Db&tag=x%26y`;
    const punctuation = `${query} % %25 + &layer=example=bad# ? /`;
    try {
      await withChrome(size, t.signal, async browser => {
        await browser.navigate(base);
        await browser.evaluate('document.querySelector("#mdbook-search-toggle").click()');
        await browser.command('Input.insertText', { text: punctuation });
        await browser.waitFor('!!document.querySelector("#mdbook-searchresults-header").textContent && !document.querySelector("#mdbook-searchbar-outer").classList.contains("searching")');
        assert.equal(await browser.evaluate('new URLSearchParams(location.hash.slice(2)).get("search")'), punctuation, 'native search corrupted punctuation in its state');
        assert.deepEqual(await browser.evaluate('new URLSearchParams(location.hash.slice(2)).getAll("tag")'), ['a=b', 'x&y']);
        assert.deepEqual(await browser.evaluate('new URLSearchParams(location.hash.slice(2)).getAll("layer")'), ['deos'], 'search text changed independent layer state');
        assert.equal(await browser.evaluate('location.search'), '', 'search state must stay out of the server-facing query');
        assert.ok(await browser.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'punctuation query overflowed its viewport');
        await browser.screenshot(join(screenshots, `${locale}-search-url.png`));
        const shared = await browser.evaluate('location.href');
        await browser.navigate(shared, true);
        await browser.waitFor('!document.querySelector("#mdbook-searchbar-outer").classList.contains("searching")');
        assert.equal(await browser.evaluate('document.querySelector("#mdbook-searchbar").value'), punctuation, 'refresh double-decoded the search value');

        await browser.navigate(base);
        await browser.evaluate('delete window.path_to_searchindex_js');
        const initialHistory = await browser.evaluate('history.length');
        const documentEpoch = await browser.evaluate('performance.timeOrigin');
        await browser.evaluate('document.querySelector("#mdbook-search-toggle").click()');
        await browser.command('Input.insertText', { text: query });
        await browser.waitFor('document.querySelectorAll("#mdbook-searchresults li").length > 0');
        // Click Chrome's native clear affordance just inside the input's right padding.
        const clear = await browser.evaluate(`(() => {
          const input = document.getElementById('mdbook-searchbar');
          const box = input.getBoundingClientRect(), style = getComputedStyle(input);
          return { x: box.right - parseFloat(style.paddingRight) - parseFloat(style.borderRightWidth) - 6, y: box.top + box.height / 2 };
        })()`);
        for (const type of ['mousePressed', 'mouseReleased']) await browser.command('Input.dispatchMouseEvent', { type, ...clear, button: 'left', clickCount: 1 });
        assert.equal(await browser.evaluate('document.getElementById("mdbook-searchbar").value'), '', 'native clear affordance did not clear the input');
        assert.equal(await browser.evaluate('document.querySelectorAll("#mdbook-searchresults li").length'), 0, 'clearing the input left stale results');
        assert.equal(await browser.evaluate('document.getElementById("mdbook-searchresults-header").textContent'), '', 'clearing the input left a stale result count');
        assert.equal(await browser.evaluate('new URLSearchParams(location.hash.slice(2)).get("search")'), '', 'clearing must record an empty open search');
        assert.equal(await browser.evaluate('document.getElementById("mdbook-search-toggle").getAttribute("aria-expanded")'), 'true');
        await browser.command('Input.insertText', { text: query });
        await browser.waitFor('document.querySelectorAll("#mdbook-searchresults li").length > 0');
        const searched = await browser.evaluate('location.href');
        assert.equal(await browser.evaluate('history.length'), initialHistory + 1, 'editing a query grew history');
        await browser.evaluate('history.back()');
        await browser.waitFor(`location.href === ${JSON.stringify(base)} && document.querySelector('#mdbook-searchbar').value === '' && document.querySelector('#mdbook-search-wrapper').classList.contains('hidden')`);
        await browser.evaluate('history.forward()');
        await browser.waitFor(`location.href === ${JSON.stringify(searched)} && document.querySelector('#mdbook-searchbar').value === ${JSON.stringify(query)} && document.querySelectorAll('#mdbook-searchresults li').length > 0`);
        assert.equal(await browser.evaluate('history.length'), initialHistory + 1, 'restoration wrote history');
        assert.equal(await browser.evaluate('performance.timeOrigin'), documentEpoch, 'query history reloaded the document');

        const heading = await browser.evaluate('document.querySelector("main h2").id');
        const marked = `${base}&search=${encodeURIComponent(query)}&highlight=Actors&at=${encodeURIComponent(heading)}`;
        await browser.navigate(marked);
        await browser.waitFor('Array.from(document.querySelectorAll("main mark")).some(mark => !mark.closest("a"))');
        await browser.evaluate('Array.from(document.querySelectorAll("main mark")).find(mark => !mark.closest("a")).click()');
        await browser.waitFor('!new URLSearchParams(location.hash.slice(2)).has("highlight")');
        assert.equal(await browser.evaluate('new URLSearchParams(location.hash.slice(2)).get("search")'), null, 'clicking the article outside the panel must dismiss search');
        assert.equal(await browser.evaluate('new URLSearchParams(location.hash.slice(2)).get("at")'), heading);

        for (const encoded of ['%', '%E0%A4%A', '%252F%2B']) {
          const malformed = `${base}&search=${encoded}`;
          await browser.navigate(malformed);
          assert.equal(await browser.evaluate('document.querySelector("#mdbook-searchbar").value'), new URLSearchParams(new URL(malformed).hash.slice(2)).get('search'));
        }
        assert.deepEqual(browser.errors, []);
      });
      assert.ok(!requests.some(request => request.status !== 200));
    } finally {
      server.closeAllConnections();
      await new Promise(resolve => server.close(resolve));
    }
  });
}
