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
  test(`${locale}: light/dark switch, help, search and clipboard retain their native mechanics`, { timeout: 60000 }, async t => {
    mkdirSync(screenshots, { recursive: true });
    await fixture(async (origin, requests) => {
      await withChrome(size, t.signal, async browser => {
        await browser.navigate(`${origin}${start.url[locale]}#!layer=deos`);
        assert.equal(await browser.evaluate('document.querySelector("#mdbook-sidebar").getAttribute("aria-label")'), ru ? 'Оглавление' : 'Table of contents');
        assert.ok(await browser.evaluate('document.querySelector("#mdbook-sidebar .chapter a.active")?.pathname === location.pathname'), 'the active page must stay in the sidebar');
        assert.equal(await browser.evaluate('document.querySelector("#mdbook-sidebar .on-this-page")'), null, 'the sidebar must not duplicate headings from the current article');
        assert.ok(await browser.evaluate(`!!document.querySelector('main h2[id] > a.header[href^="#"]')`), 'heading permalinks must remain in the article');
        const association = await browser.evaluate('document.querySelector("#mdbook-searchbar").getAttribute("aria-describedby")');
        assert.equal(association, 'mdbook-searchresults-header');
        assert.ok(await browser.evaluate(`!!document.getElementById(${JSON.stringify(association)})`));
        assert.equal(await browser.evaluate('document.querySelector("#mdbook-searchbar").placeholder'), ru ? 'Поиск по материалам…' : 'Search this book ...');
        // One visible light/dark switch drives mdBook's native theme mechanics; the palette stays hidden.
        assert.equal(await browser.evaluate('getComputedStyle(document.querySelector("#mdbook-theme-toggle")).display'), 'none');
        const toggle = '.wiki-theme-toggle';
        assert.equal(await browser.evaluate(`document.querySelector('${toggle}').getAttribute('aria-pressed')`), 'false');
        assert.equal(await browser.evaluate(`document.querySelector('${toggle}').getAttribute('aria-label')`), ru ? 'Тёмная тема' : 'Dark theme');
        await browser.evaluate(`document.querySelector('${toggle}').click()`);
        assert.ok(await browser.evaluate('document.documentElement.classList.contains("navy")'));
        assert.equal(await browser.evaluate('localStorage.getItem("mdbook-theme")'), 'navy');
        assert.equal(await browser.evaluate(`document.querySelector('${toggle}').getAttribute('aria-pressed')`), 'true');
        await browser.screenshot(join(screenshots, `${locale}-reader-dark.png`));
        const themed = await browser.evaluate('location.href');
        await browser.navigate(themed, true);
        assert.ok(await browser.evaluate('document.documentElement.classList.contains("navy")'));
        assert.equal(await browser.evaluate(`document.querySelector('${toggle}').getAttribute('aria-label')`), ru ? 'Светлая тема' : 'Light theme');
        await browser.evaluate(`document.querySelector('${toggle}').click()`);
        assert.ok(await browser.evaluate('document.documentElement.classList.contains("light")'));
        assert.equal(await browser.evaluate('localStorage.getItem("mdbook-theme")'), 'light');
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
        await browser.waitFor('document.querySelectorAll("#mdbook-searchresults li").length > 0');
        assert.ok(await browser.evaluate(`(() => {
          const input = document.getElementById('mdbook-searchbar');
          const style = getComputedStyle(input);
          const color = document.createElement('canvas').getContext('2d');
          color.fillStyle = style.borderTopColor;
          const border = color.fillStyle;
          color.fillStyle = style.getPropertyValue('--sidebar-header-border-color').trim();
          return document.activeElement === input && border === color.fillStyle;
        })()`), 'the focused search border must retain the theme accent');
        const metric = await browser.evaluate('document.querySelector("#mdbook-searchresults-header").textContent');
        assert.match(metric, ru ? /результат[а-я]* по запросу/ : /search result/);
        assert.ok(metric.includes(query));
        assert.ok(await browser.evaluate(`(() => {
          const { left, right } = document.getElementById('mdbook-search-wrapper').getBoundingClientRect();
          return left > 0 && Math.abs(left + right - innerWidth) <= 2;
        })()`), 'search is not centred in the usable viewport');
        assert.equal(await browser.evaluate('document.querySelector(".teaser").getAttribute("aria-label")'), ru ? 'Фрагмент совпадения' : 'Search Result Teaser');
        await browser.screenshot(join(screenshots, `${locale}-reader-search.png`));
        await browser.navigate(`${origin}${codePage.url[locale]}#!layer=deos`);
        await browser.waitFor('!!document.querySelector(".clip-button")');
        assert.equal(await browser.evaluate('document.querySelector(".clip-button").getAttribute("aria-label")'), ru ? 'Копировать в буфер обмена' : 'Copy to clipboard');
        await browser.evaluate('document.querySelector(".clip-button").click()');
        await browser.waitFor(`document.querySelector('.clip-button .tooltiptext').textContent === ${JSON.stringify(ru ? 'Скопировано!' : 'Copied!')}`);
        // The feedback tooltip stays inside its code block and never makes the block scroll.
        assert.deepEqual(await browser.evaluate(`(() => { const button = document.querySelector('.clip-button'), pre = button.closest('pre'), tip = button.querySelector('.tooltiptext').getBoundingClientRect(), box = pre.getBoundingClientRect(); return [pre.scrollWidth <= pre.clientWidth, pre.scrollHeight <= pre.clientHeight, tip.left >= box.left && tip.right <= box.right]; })()`), [true, true, true]);
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
        await browser.navigate(`${origin}${codePage.url[locale]}#!layer=deos`);
        const initial = await browser.evaluate('parseFloat(getComputedStyle(document.documentElement).fontSize)');
        await browser.command('Page.setFontSizes', { fontSizes: { standard: 32 } });
        await browser.waitFor(`parseFloat(getComputedStyle(document.documentElement).fontSize) >= ${initial * 1.9}`);
        assert.ok(await browser.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'enlarged text overflowed the narrow document');
        const controls = await browser.evaluate('Array.from(document.querySelectorAll("#mdbook-menu-bar .icon-button, .wiki-locale-choices"), item => item.getBoundingClientRect()).filter(box => box.width > 0).map(box => box.left >= 0 && box.right <= innerWidth + 0.5)');
        assert.ok(controls.length >= 4 && controls.every(Boolean), 'menu-bar controls left the enlarged narrow viewport');
        await browser.screenshot(join(screenshots, `${locale}-reader-large-text.png`));
        await browser.command('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape' });
        assert.equal(await browser.evaluate('new URLSearchParams(location.hash.slice(2)).get("layer")'), 'deos');
        assert.deepEqual(browser.errors, []);
      });
      assert.ok(!requests.some(request => request.status !== 200));
    });
  });
}

test('search opens as a sheet and only its results scroll while it is open', { timeout: 60000 }, async t => {
  mkdirSync(screenshots, { recursive: true });
  await fixture(async (origin, requests) => {
    await withChrome([1600, 900], t.signal, async browser => {
      await browser.navigate(`${origin}${start.url.en}#!layer=deos`);
      const place = () => browser.evaluate(`(() => {
        const sheet = document.querySelector('#mdbook-search-wrapper');
        const style = getComputedStyle(sheet);
        const box = sheet.getBoundingClientRect();
        const article = document.querySelector('.content main h1').getBoundingClientRect();
        const column = document.querySelector('.content main').getBoundingClientRect();
        const results = document.querySelector('.searchresults-outer');
        const channels = value => {
          const srgb = value.match(/color\\(srgb ([^)]+)\\)/);
          return srgb ? srgb[1].trim().split(/\\s+/).map(Number).map(part => part * 255) : value.match(/\\d+/g).slice(0, 3).map(Number);
        };
        const luminance = value => channels(value).map(number => { const channel = number / 255; return channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4; }).reduce((sum, channel, index) => sum + channel * [0.2126, 0.7152, 0.0722][index], 0);
        const probe = (x, y) => document.elementFromPoint(x, y);
        const probeY = Math.min(innerHeight - 20, 300);
        return {
          scrollY,
          sidebarScroll: document.querySelector('#mdbook-sidebar .sidebar-scrollbox').scrollTop,
          hidden: sheet.classList.contains('hidden'),
          backdrop: getComputedStyle(document.body, '::before').content !== 'none',
          position: style.position,
          sheetTop: Math.round(box.top),
          sheetLeft: Math.round(box.left),
          sheetWidth: Math.round(box.width),
          sheetBottom: Math.round(box.bottom),
          sheetCenter: Math.round(box.left + box.width / 2),
          viewportCenter: Math.round(innerWidth / 2),
          sheetLuminance: Number(luminance(style.backgroundColor).toFixed(4)),
          pageLuminance: Number(luminance(getComputedStyle(document.querySelector('.page-wrapper')).backgroundColor).toFixed(4)),
          backdropAtEdge: probe(innerWidth - 2, probeY) === document.body,
          articleTop: Math.round(article.top + scrollY),
          columnLeft: Math.round(column.left),
          columnWidth: Math.round(column.width),
          resultsLeft: results ? Math.round(results.getBoundingClientRect().left) : null,
          resultsRight: results ? Math.round(results.getBoundingClientRect().right) : null,
          panelCoversArticle: probe(box.left + box.width / 2, probeY)?.closest('#mdbook-search-wrapper') === sheet,
          articleHit: !!probe(column.right - 20, probeY)?.closest('.content main'),
          overflows: document.documentElement.scrollWidth > innerWidth,
        };
      })()`);
      const closed = await place();
      assert.equal(closed.hidden, true);
      assert.equal(closed.backdrop, false);
      assert.equal(closed.position, 'static');
      const setSidebarScroll = top => browser.evaluate(`(() => {
        for (const group of document.querySelectorAll('.chapter-item:not(.expanded) > .wiki-group')) group.click();
        document.querySelector('#mdbook-sidebar .sidebar-scrollbox').scrollTop = ${top};
      })()`);
      await setSidebarScroll(60);
      await browser.evaluate('window.scrollTo(0, 480)');
      await browser.evaluate('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
      const scrolled = await place();
      assert.equal(scrolled.scrollY, 480);
      assert.equal(scrolled.sidebarScroll, 60, 'the fixture must have a scrollable sidebar');
      assert.ok(scrolled.articleHit, 'the article must be the reading surface before search opens');
      const nextChevron = await browser.evaluate(`(() => {
        const { left, right, top, bottom } = document.querySelector('.nav-chapters.next').getBoundingClientRect();
        return { x: (left + right) / 2, y: (top + bottom) / 2 };
      })()`);
      const hoverChevron = () => browser.command('Input.dispatchMouseEvent', { type: 'mouseMoved', ...nextChevron });
      const chevronHovered = 'document.querySelector(".nav-chapters.next").matches(":hover")';
      await hoverChevron();
      await browser.waitFor(chevronHovered);
      const historyBeforeOpen = await browser.evaluate('history.length');
      await browser.evaluate('document.querySelector("#mdbook-search-toggle").click()');
      await browser.evaluate('new Promise(resolve => setTimeout(resolve, 250))');
      const opened = await place();
      assert.equal(opened.hidden, false);
      assert.equal(opened.backdrop, true, 'opening search must dim the page behind it');
      assert.ok(opened.backdropAtEdge, 'the backdrop must cover the viewport scrollbar edge');
      assert.equal(opened.position, 'fixed', 'search still takes part in the article flow');
      assert.equal(opened.scrollY, scrolled.scrollY, 'opening search moved the reading position');
      assert.equal(opened.sidebarScroll, scrolled.sidebarScroll, 'opening search moved the sidebar');
      assert.deepEqual([opened.articleTop, opened.columnLeft, opened.columnWidth], [scrolled.articleTop, scrolled.columnLeft, scrolled.columnWidth], 'opening search reflowed the article');
      // Opening is a URL state, not a bare navigation: a shared or restored URL reproduces the sheet.
      assert.equal(await browser.evaluate('new URLSearchParams(location.hash.slice(2)).has("search")'), true, 'opening search left no fragment state');
      assert.equal(await browser.evaluate('new URLSearchParams(location.hash.slice(2)).get("search")'), '');
      assert.equal(await browser.evaluate('location.search'), '', 'search state must never reach the server-facing query');
      assert.equal(await browser.evaluate('history.length'), historyBeforeOpen + 1, 'opening search must push exactly one entry');
      const openHref = await browser.evaluate('location.href');
      assert.equal(opened.sheetTop, 50, 'the panel must start beneath the menu bar');
      assert.equal(opened.sheetTop + opened.sheetBottom, 900, 'the panel must have equal vertical insets');
      assert.ok(opened.sheetWidth < opened.columnWidth, 'the panel must stay narrower than the page');
      assert.ok(Math.abs(opened.sheetCenter - opened.viewportCenter) <= 1, 'the panel must be centred in the viewport');
      assert.ok(opened.sheetLuminance < opened.pageLuminance, 'the panel surface must be darker than the page');
      assert.ok(opened.panelCoversArticle, 'the panel must cover the article it replaces');
      assert.equal(opened.articleHit, false, 'the article must not receive pointer hits through the backdrop');
      await hoverChevron();
      assert.equal(await browser.evaluate(chevronHovered), false, 'the backdrop must block hover on chapter navigation');
      assert.equal(opened.overflows, false);
      const click = async (x, y) => {
        for (const type of ['mousePressed', 'mouseReleased']) await browser.command('Input.dispatchMouseEvent', { type, x, y, button: 'left', clickCount: 1 });
      };
      await click(opened.sheetCenter, opened.sheetTop + 30);
      assert.equal(await browser.evaluate('location.href'), openHref, 'a click inside search dismissed it');
      await browser.command('Input.insertText', { text: 'observations' });
      await browser.waitFor('!!document.querySelector("#mdbook-searchresults-header").textContent && !document.querySelector("#mdbook-searchbar-outer").classList.contains("searching")');
      const withResults = await place();
      assert.ok(await browser.evaluate('document.querySelectorAll("#mdbook-searchresults li").length > 0'));
      assert.equal(withResults.scrollY, scrolled.scrollY, 'rendering results moved the reading position');
      assert.ok(withResults.resultsLeft >= withResults.sheetLeft && withResults.resultsRight <= withResults.sheetLeft + withResults.sheetWidth, 'results escaped the panel');
      assert.equal(withResults.overflows, false);
      assert.ok(await browser.evaluate('document.querySelector("#mdbook-search-wrapper").scrollHeight > document.querySelector("#mdbook-search-wrapper").clientHeight'), 'the sheet must scroll its own results');
      assert.equal(await browser.evaluate('new URLSearchParams(location.hash.slice(2)).get("search")'), 'observations');
      const wheel = x => browser.command('Input.dispatchMouseEvent', { type: 'mouseWheel', x, y: 400, deltaX: 0, deltaY: 180 });
      const outsideX = opened.sheetLeft + opened.sheetWidth + 40;
      await wheel(outsideX);
      await wheel(50);
      await wheel(opened.sheetCenter);
      await browser.waitFor('document.getElementById("mdbook-search-wrapper").scrollTop > 0');
      const afterWheel = await place();
      assert.equal(afterWheel.scrollY, scrolled.scrollY, 'wheel input scrolled the article behind search');
      assert.equal(afterWheel.sidebarScroll, scrolled.sidebarScroll, 'wheel input scrolled the sidebar behind search');
      await browser.evaluate('document.getElementById("mdbook-search-wrapper").scrollTop = 0');
      await browser.screenshot(join(screenshots, 'en-search-sheet.png'));
      // Reload reproduces the open sheet from the URL alone.
      await browser.navigate(await browser.evaluate('location.href'), true);
      await browser.waitFor('!!document.querySelector("#mdbook-searchresults-header").textContent && !document.querySelector("#mdbook-searchbar-outer").classList.contains("searching")');
      assert.equal(await browser.evaluate('document.querySelector("#mdbook-search-wrapper").classList.contains("hidden")'), false, 'a search URL must restore the open sheet');
      assert.equal(await browser.evaluate('document.querySelector("#mdbook-searchbar").value'), 'observations');
      // Clicking over a chevron dismisses search without activating chapter navigation.
      await click(nextChevron.x, nextChevron.y);
      await browser.waitFor('document.getElementById("mdbook-search-wrapper").classList.contains("hidden")');
      const dismissed = await place();
      assert.equal(dismissed.hidden, true);
      assert.equal(dismissed.backdrop, false, 'closing search left the backdrop visible');
      assert.equal(dismissed.backdropAtEdge, false, 'closing search left the backdrop at the viewport edge');
      assert.equal(dismissed.scrollY, scrolled.scrollY, 'closing search moved the reading position');
      assert.equal(dismissed.articleTop, scrolled.articleTop);
      assert.equal(dismissed.panelCoversArticle, false);
      assert.ok(dismissed.articleHit, 'the article must be readable again after search closes');
      assert.equal(await browser.evaluate('location.pathname + location.hash'), `${start.url.en}#!layer=deos`, 'backdrop click must close search without navigating');
      await hoverChevron();
      await browser.waitFor(chevronHovered);
      // History restores both directions: Back reopens the sheet, Forward closes it again.
      await browser.evaluate('history.back()');
      await browser.waitFor('new URLSearchParams(location.hash.slice(2)).has("search") && !document.querySelector("#mdbook-search-wrapper").classList.contains("hidden")');
      await browser.evaluate('history.forward()');
      await browser.waitFor('!new URLSearchParams(location.hash.slice(2)).has("search") && document.querySelector("#mdbook-search-wrapper").classList.contains("hidden")');
      const resumedAt = await browser.evaluate('scrollY');
      await wheel(outsideX);
      await browser.waitFor(`scrollY > ${resumedAt}`);
      await setSidebarScroll(0);
      await wheel(50);
      await browser.waitFor('document.querySelector("#mdbook-sidebar .sidebar-scrollbox").scrollTop > 0');
      assert.ok(/#!.*search=/.test(new URL(openHref).hash), 'the open state must be the recorded history entry');
      assert.equal(new URLSearchParams(new URL(openHref).hash.slice(2)).get('layer'), 'deos', 'opening search dropped the layer state');
      assert.deepEqual(browser.errors, []);
    });
    assert.ok(!requests.some(request => request.status !== 200));
  });
});
