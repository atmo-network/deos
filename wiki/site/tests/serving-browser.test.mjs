/*
Domain: Static serving regressions
Owns: Location independence, trailing-slash-free directory indexes and the language-remembering site root.
Excludes: Base-path-specific 404 routing, iframe embedding and production host headers.
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
const landing = graph.nodes.find(node => node.id === 'deos/index');
const article = graph.nodes.find(node => node.id === 'deos/actor-system');
// Site-relative path of a composed route, so the same artifact can be served beneath any prefix.
const path = route => route.slice(base.length);

async function served(prefix, callback) {
  const requests = [], server = serveStatic(root, requests, new Map(), prefix);
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try { await callback(`http://127.0.0.1:${server.address().port}`, requests); }
  finally { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
}

const ready = (browser, pathname) => browser.waitFor(`location.pathname === ${JSON.stringify(pathname)} && document.readyState === 'complete' && document.documentElement.dataset.wikiLayer === 'ready'`);

test('a directory index served without its trailing slash keeps layers, search, assets and the current chapter', { timeout: 60000 }, async t => {
  await served(base, async (origin, requests) => {
    // 900px is below mdBook's own 1080px sidebar breakpoint; the reader still opens the sidebar.
    await withChrome([900, 800], t.signal, async browser => {
      // Clean-URL servers deliver `/ru` instead of `/ru/`; every relative URL would otherwise resolve one level too high.
      await browser.command('Page.navigate', { url: `${origin}${base}ru` });
      await ready(browser, `${base}ru`);
      assert.equal(await browser.evaluate('window.wikiContext.page'), 'ru/index.html');
      assert.equal(await browser.evaluate('document.querySelector("main h1").textContent'), 'Содержание');
      assert.ok(await browser.evaluate('!!window.path_to_searchindex_js && !!window.elasticlunr'), 'search assets did not load');
      assert.ok(await browser.evaluate('document.documentElement.classList.contains("sidebar-visible")'), 'sidebar closed by default on a desktop width');
      assert.deepEqual(await browser.evaluate('Array.from(document.querySelectorAll(".sidebar-scrollbox a.active"), a => a.textContent)'), ['Содержание']);
      const destination = await browser.evaluate(`Array.from(document.querySelectorAll('main li a'), a => new URL(a.href).pathname).find(pathname => pathname.endsWith(${JSON.stringify(path(article.url.ru))}))`);
      assert.equal(destination, `${base}${path(article.url.ru)}`);
      assert.deepEqual(browser.errors, []);
    });
    // Chrome's preload scanner may speculatively fetch scripts against the slash-less URL before the
    // restored <base> applies; each such miss must be followed by the real load from the locale book.
    const loaded = new Set(requests.filter(request => request.status === 200).map(request => request.path));
    const unrecovered = requests.filter(request => request.status !== 200 && !loaded.has(`${base}ru/${request.path.slice(base.length)}`));
    assert.deepEqual(unrecovered, [], 'a resource missed the artifact');
  });
});

test('the same artifact serves from any URL prefix without leaving it', { timeout: 60000 }, async t => {
  const prefix = base === '/' ? '/any/where/' : base;
  await served(prefix, async (origin, requests) => {
    await withChrome([1280, 900], t.signal, async browser => {
      await browser.command('Page.navigate', { url: `${origin}${prefix}` });
      await ready(browser, `${prefix}${path(landing.url.en)}`);
      await browser.evaluate(`Array.from(document.querySelectorAll('main a')).find(a => new URL(a.href).pathname === ${JSON.stringify(`${prefix}${path(article.url.en)}`)}).click()`);
      await ready(browser, `${prefix}${path(article.url.en)}`);
      assert.deepEqual(await browser.evaluate('Array.from(document.querySelectorAll("main .wiki-tag"), tag => tag.textContent)'), ['#deos']);
      // Sidebar groups start folded except the current page's group, and the whole row toggles a group.
      const groups = `Array.from(document.querySelectorAll('#mdbook-sidebar .wiki-group'), row => ({ open: row.parentElement.classList.contains('expanded'), aria: row.firstElementChild.getAttribute('aria-expanded'), current: !!row.parentElement.querySelector('a.active') }))`;
      const initial = await browser.evaluate(groups);
      assert.ok(initial.length > 1 && initial.every(group => group.open === group.current && group.aria === String(group.open)), 'only the current group may start open');
      const closed = initial.findIndex(group => !group.open);
      await browser.evaluate(`document.querySelectorAll('#mdbook-sidebar .wiki-group > span')[${closed}].click()`);
      assert.deepEqual((await browser.evaluate(groups))[closed], { open: true, aria: 'true', current: false });
      await browser.evaluate(`document.querySelectorAll('#mdbook-sidebar .wiki-group')[${closed}].click()`);
      assert.deepEqual((await browser.evaluate(groups))[closed], { open: false, aria: 'false', current: false });
      await browser.evaluate(`document.querySelector('.wiki-locale-link[lang="ru"]').click()`);
      await ready(browser, `${prefix}${path(article.url.ru)}`);
      await browser.evaluate(`document.querySelector('.nav-chapters.next').click()`);
      await browser.waitFor(`location.pathname.startsWith(${JSON.stringify(`${prefix}ru/`)}) && location.pathname !== ${JSON.stringify(`${prefix}${path(article.url.ru)}`)} && document.documentElement.dataset.wikiLayer === 'ready'`);
      assert.deepEqual(browser.errors, []);
    });
    const stray = requests.filter(request => request.status !== 200 || !request.path.startsWith(prefix));
    assert.deepEqual(stray, [], 'a request left the prefix or missed the artifact');
  });
});

test('the site root opens the landing in the last used language instead of a chooser', { timeout: 60000 }, async t => {
  await served(base, async origin => {
    await withChrome([1280, 900], t.signal, async browser => {
      await browser.command('Page.navigate', { url: `${origin}${base}` });
      await ready(browser, `${base}${path(landing.url[graph.default_locale])}`);
      assert.equal(await browser.evaluate('document.querySelectorAll(".wiki-locale-link").length'), graph.locales.length);
      await browser.evaluate(`document.querySelector('.wiki-locale-link[lang="ru"]').click()`);
      await ready(browser, `${base}${path(landing.url.ru)}`);
      await browser.command('Page.navigate', { url: `${origin}${base}` });
      await ready(browser, `${base}${path(landing.url.ru)}`);
      assert.equal(await browser.evaluate('document.documentElement.lang'), 'ru');
      assert.deepEqual(browser.errors, []);
    });
  });
});
