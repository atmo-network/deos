/*
Domain: Native reader-control acceptance
Owns: Real localized shell, theme/help/clipboard interaction and enlarged-text reflow evidence.
Excludes: Article translation attestation, iframe permissions and production hosting.
*/
import assert from 'node:assert/strict';
import { mkdirSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { withChrome } from './chrome.mjs';
import { serveStatic } from './serve.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../dist');
const screenshots = resolve(root, '../target/browser-smoke');
const graph = JSON.parse(readFileSync(join(root, 'graph.json'), 'utf8'));
const start = graph.nodes.find(node => node.id === 'deos/actor-system');
const codePage = graph.nodes.find(node => readFileSync(join(root, node.url.en), 'utf8').includes('<pre'));
assert.ok(codePage, 'native clipboard acceptance requires a real published code example');

async function fixture(callback) {
  const requests = [], server = serveStatic(root, requests);
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try { await callback(`http://127.0.0.1:${server.address().port}`, requests); }
  finally { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
}

for (const [locale, size, query] of [['en', [1280, 900], 'observations'], ['ru', [360, 800], 'наблюдения']]) {
  const ru = locale === 'ru';
  test(`${locale}: native localized themes, help, search and clipboard retain their mechanics`, { timeout: 60000 }, async t => {
    mkdirSync(screenshots, { recursive: true });
    await fixture(async (origin, requests) => {
      await withChrome(size, t.signal, async browser => {
        await browser.navigate(`${origin}${start.url[locale]}?layer=deos`);
        assert.equal(await browser.evaluate('document.querySelector("#mdbook-sidebar").getAttribute("aria-label")'), ru ? 'Оглавление' : 'Table of contents');
        const association = await browser.evaluate('document.querySelector("#mdbook-searchbar").getAttribute("aria-describedby")');
        assert.equal(association, 'mdbook-searchresults-header');
        assert.ok(await browser.evaluate(`!!document.getElementById(${JSON.stringify(association)})`));
        assert.equal(await browser.evaluate('document.querySelector("#mdbook-searchbar").placeholder'), ru ? 'Поиск по материалам…' : 'Search this book ...');
        await browser.evaluate('document.querySelector("#mdbook-theme-toggle").click()');
        assert.equal(await browser.evaluate('document.querySelector("#mdbook-theme-toggle").getAttribute("aria-expanded")'), 'true');
        assert.deepEqual(await browser.evaluate('Array.from(document.querySelectorAll("#mdbook-theme-list button"), button => button.textContent)'), ru ? ['Системная', 'Светлая', 'Rust', 'Угольная', 'Тёмно-синяя', 'Ayu'] : ['Auto', 'Light', 'Rust', 'Coal', 'Navy', 'Ayu']);
        await browser.evaluate('document.querySelector("#mdbook-theme-navy").click()');
        assert.ok(await browser.evaluate('document.documentElement.classList.contains("navy")'));
        assert.equal(await browser.evaluate('localStorage.getItem("mdbook-theme")'), 'navy');
        const themed = await browser.evaluate('location.href');
        await browser.navigate(themed, true);
        assert.ok(await browser.evaluate('document.documentElement.classList.contains("navy")'));
        await browser.evaluate('document.querySelector("#mdbook-theme-toggle").click(); document.querySelector("#mdbook-theme-light").click()');
        await browser.command('Input.dispatchKeyEvent', { type: 'keyDown', key: '?', shiftKey: true });
        await browser.waitFor('getComputedStyle(document.querySelector("#mdbook-help-container")).display !== "none"');
        assert.equal(await browser.evaluate('document.querySelector(".mdbook-help-title").textContent'), ru ? 'Горячие клавиши' : 'Keyboard shortcuts');
        assert.match(await browser.evaluate('document.querySelector("#mdbook-help-popup").textContent'), ru ? /для перехода между главами/ : /navigate between chapters/);
        assert.ok(await browser.evaluate('document.documentElement.scrollWidth <= innerWidth'));
        await browser.screenshot(join(screenshots, `${locale}-reader-help.png`));
        await browser.command('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape' });
        await browser.waitFor('getComputedStyle(document.querySelector("#mdbook-help-container")).display === "none"');
        await browser.evaluate('document.querySelector("#mdbook-sidebar-toggle").click()');
        const expanded = await browser.evaluate('document.querySelector("#mdbook-sidebar-toggle").getAttribute("aria-expanded")');
        await browser.evaluate('document.querySelector("#mdbook-sidebar-toggle").click()');
        assert.notEqual(await browser.evaluate('document.querySelector("#mdbook-sidebar-toggle").getAttribute("aria-expanded")'), expanded);
        await browser.evaluate('document.querySelector("#mdbook-search-toggle").click()');
        await browser.command('Input.insertText', { text: query });
        await browser.command('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Unidentified' });
        await browser.waitFor('document.querySelectorAll("#mdbook-searchresults li").length > 0');
        const metric = await browser.evaluate('document.querySelector("#mdbook-searchresults-header").textContent');
        assert.match(metric, ru ? /результат[а-я]* по запросу/ : /search result/);
        assert.ok(metric.includes(query));
        assert.equal(await browser.evaluate('document.querySelector(".teaser").getAttribute("aria-label")'), ru ? 'Фрагмент совпадения' : 'Search Result Teaser');
        await browser.screenshot(join(screenshots, `${locale}-reader-search.png`));
        await browser.navigate(`${origin}${codePage.url[locale]}?layer=deos`);
        await browser.waitFor('!!document.querySelector(".clip-button")');
        assert.equal(await browser.evaluate('document.querySelector(".clip-button").getAttribute("aria-label")'), ru ? 'Копировать в буфер обмена' : 'Copy to clipboard');
        await browser.evaluate('document.querySelector(".clip-button").click()');
        await browser.waitFor(`document.querySelector('.clip-button .tooltiptext').textContent === ${JSON.stringify(ru ? 'Скопировано!' : 'Copied!')}`);
        // Controlled local clipboard refusal; keep the actual native handler and library.
        await browser.evaluate('document.execCommand = () => false; document.querySelector(".clip-button").click()');
        await browser.waitFor(`document.querySelector('.clip-button .tooltiptext').textContent === ${JSON.stringify(ru ? 'Не удалось скопировать' : 'Clipboard error!')}`);
        assert.deepEqual(browser.errors, []);
      });
      assert.ok(!requests.some(request => request.status !== 200));
    });
  });

  test(`${locale}: native controls and reading reflow at 200% default text size`, { timeout: 60000 }, async t => {
    await fixture(async (origin, requests) => {
      await withChrome([360, 800], t.signal, async browser => {
        await browser.navigate(`${origin}${codePage.url[locale]}?layer=deos`);
        const initial = await browser.evaluate('parseFloat(getComputedStyle(document.documentElement).fontSize)');
        await browser.command('Page.setFontSizes', { fontSizes: { standard: 32 } });
        await browser.waitFor(`parseFloat(getComputedStyle(document.documentElement).fontSize) >= ${initial * 1.9}`);
        assert.ok(await browser.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'enlarged text overflowed the narrow document');
        await browser.evaluate('document.querySelector("#mdbook-theme-toggle").click()');
        assert.ok(await browser.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'theme popup overflowed enlarged text');
        assert.ok(await browser.evaluate('Array.from(document.querySelectorAll("#mdbook-theme-list button")).every(button => button.getBoundingClientRect().width <= innerWidth)'));
        await browser.screenshot(join(screenshots, `${locale}-reader-large-text.png`));
        await browser.command('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape' });
        assert.equal(await browser.evaluate('new URL(location.href).searchParams.get("layer")'), 'deos');
        assert.deepEqual(browser.errors, []);
      });
      assert.ok(!requests.some(request => request.status !== 200));
    });
  });
}
