/*
Domain: Reader localization regressions
Owns: Russian plural forms, protected article bytes and guarded native label contracts.
Excludes: Browser behavior, prose fluency attestation and untrusted HTML sanitization.
*/
import assert from 'node:assert/strict';
import { createContext, runInContext } from 'node:vm';
import test from 'node:test';
import { formatRussianSearchMetric, localizeBookScript, localizeReaderChrome, localizeSearchScript } from '../src/locale.mjs';

const help = ['Keyboard shortcuts', 'Press <kbd>←</kbd> or <kbd>→</kbd> to navigate between chapters', 'Press <kbd>S</kbd> or <kbd>/</kbd> to search in the book', 'Press <kbd>?</kbd> to show this help', 'Press <kbd>Esc</kbd> to hide this help'];
const attributes = ['Table of contents', ...Array(2).fill('Toggle Table of Contents'), ...Array(2).fill('Change theme'), 'Themes', 'Search (`/`)', 'Toggle Searchbar', ...Array(2).fill('Print this book')];
const before = help.map(text => `<p>${text}</p>`).join('') + attributes.map(text => ` aria-label="${text}"`).join('') + ['Auto', 'Light', 'Coal', 'Navy'].map(text => `<button>${text}</button>`).join('') + '<input placeholder="Search this book ..." aria-describedby="searchresults-header"><iframe class="sidebar-iframe-outer"></iframe>';
const body = '<main><h1>Search this book ...</h1><p>Change theme; Clipboard error!; Keyboard shortcuts</p><pre>Copy to clipboard</pre></main>';
const after = '<nav aria-label="Page navigation"></nav><nav aria-label="Page navigation"></nav>' + Array(4).fill('Previous chapter Next chapter').join(' ');
const document = before + body + after;

test('Russian search metrics preserve opaque terms and use grammatical count families', () => {
  assert.equal(formatRussianSearchMetric(0, '% + &= «query»'), 'По запросу «% + &= «query»» ничего не найдено.');
  for (const [count, noun] of [[1, 'результат'], [2, 'результата'], [5, 'результатов'], [11, 'результатов'], [21, 'результат'], [22, 'результата'], [25, 'результатов'], [101, 'результат'], [111, 'результатов']]) assert.equal(formatRussianSearchMetric(count, 'наблюдения'), `${count} ${noun} по запросу «наблюдения»:`);
});

test('localization changes native chrome and label associations, never authored article bytes', () => {
  const translated = localizeReaderChrome(document, 'ru');
  assert.ok(translated.includes(body));
  assert.ok(translated.includes('Горячие клавиши'));
  assert.ok(translated.includes('aria-describedby="mdbook-searchresults-header"'));
  assert.ok(translated.includes('title="Оглавление"'));
  assert.ok(translated.includes('Следующая глава'));
  const english = localizeReaderChrome(document, 'en');
  assert.ok(english.includes(body));
  assert.ok(english.includes('Change theme'));
  assert.ok(english.includes('title="Table of contents"'));
  assert.ok(!english.includes('aria-describedby="searchresults-header"'));
});

test('unsupported native boundaries and label drift fail closed', () => {
  assert.throws(() => localizeReaderChrome(document.replace('Keyboard shortcuts', 'New help'), 'ru'), /Pinned reader localization changed/);
  assert.throws(() => localizeReaderChrome('<main>incomplete', 'ru'), /article boundary/);
  assert.equal(localizeReaderChrome('<nav>Standalone native TOC</nav>', 'ru'), '<nav>Standalone native TOC</nav>');
  assert.throws(() => localizeBookScript('different native book', 'ru'), /Pinned reader/);
  assert.throws(() => localizeSearchScript('different search', 'ru'), /metric formatter/);
});

test('search localization keeps the native formatter interface and produces plain text', () => {
  const original = `    function formatSearchMetric(count, searchterm) { return 'English'; }\n    function formatSearchResult(result, searchterms) { return 'aria-label="Search Result Teaser"'; }`;
  assert.equal(localizeSearchScript(original, 'en'), original);
  const context = createContext({});
  runInContext(localizeSearchScript(original, 'ru'), context);
  assert.equal(context.formatSearchMetric(2, '<b>текст</b>'), '2 результата по запросу «<b>текст</b>»:');
  assert.equal(context.formatSearchResult(), 'aria-label="Фрагмент совпадения"');
});
