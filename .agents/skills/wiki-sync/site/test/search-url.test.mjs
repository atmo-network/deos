/*
Domain: Reader URL regressions
Owns: Query-value isolation, Unicode/punctuation round trips and native history decisions.
Excludes: Browser navigation, search ranking, layer membership and production origins.
*/
import assert from 'node:assert/strict';
import test from 'node:test';
import { nextSearchLocation, readSearchLocation, rebindScriptAsset, rewriteNativeSearchUrls, withoutSearchHighlight, searchResultLocation } from '../src/search-url.mjs';

const base = 'https://wiki.invalid/ru/deos/overview/actor.ru.html?layer=deos&tag=a%3Db&tag=x%26y#heading';

test('queries are opaque values, never additional parameters, fragments or origins', () => {
  for (const query of ['observations', 'наблюдения', '% %25 + &=#?/', 'x&layer=example#bad', '%2F', '«Ёмкость»', 'a=b=c', 'https://evil.invalid/?x=y', "quotes'\"\\"]) {
    const next = nextSearchLocation(base, query, 'push_if_new_search_else_replace');
    const url = new URL(next.href);
    assert.equal(readSearchLocation(next.href).query, query);
    assert.equal(url.origin, new URL(base).origin);
    assert.equal(url.pathname, new URL(base).pathname);
    assert.deepEqual(url.searchParams.getAll('layer'), ['deos']);
    assert.deepEqual(url.searchParams.getAll('tag'), ['a=b', 'x&y']);
    assert.equal(url.hash, '');
    assert.equal(next.action, 'push');
  }
});

test('history uses one first-search push and later replacements; explicit close remains distinct', () => {
  const first = nextSearchLocation(base, 'one', 'push_if_new_search_else_replace');
  const second = nextSearchLocation(first.href, 'two', 'push_if_new_search_else_replace');
  assert.equal(second.action, 'replace');
  const empty = nextSearchLocation(second.href, '', 'push_if_new_search_else_replace');
  assert.equal(readSearchLocation(empty.href).query, '');
  assert.equal(empty.action, 'replace');
  const close = nextSearchLocation(second.href, '', 'push');
  assert.equal(readSearchLocation(close.href).query, null);
  assert.equal(close.action, 'push');
  assert.equal(nextSearchLocation(base, 'one', 'replace').action, 'replace');
  assert.throws(() => nextSearchLocation(base, 'one', 'unknown'), /Unsupported/);
});

test('URL reads decode exactly once and malformed escapes remain data rather than exceptions', () => {
  assert.deepEqual(readSearchLocation('https://wiki.invalid/?search=%252F%2B&highlight=%2525'), { query: '%2F+', highlight: '%25' });
  assert.equal(readSearchLocation('https://wiki.invalid/?search=a+b').query, 'a b');
  assert.equal(readSearchLocation('https://wiki.invalid/?search=%').query, '%');
  assert.doesNotThrow(() => readSearchLocation('https://wiki.invalid/?search=%E0%A4%A'));
});

test('dismissing highlights preserves active search, context and heading', () => {
  const href = nextSearchLocation(base, 'наблюдения %', 'replace').href + '&highlight=Actors#summary';
  const clean = new URL(withoutSearchHighlight(href));
  assert.equal(clean.searchParams.get('search'), 'наблюдения %');
  assert.equal(clean.searchParams.has('highlight'), false);
  assert.deepEqual(clean.searchParams.getAll('tag'), ['a=b', 'x&y']);
  assert.equal(clean.hash, '#summary');
});

test('result links preserve independent context and the native heading without retaining the old query', () => {
  const url = new URL(searchResultLocation(base + '&search=old', 'typed-observations.ru.html#heading', ['наблюдения', '%+&']));
  assert.equal(url.pathname, '/ru/deos/overview/typed-observations.ru.html');
  assert.equal(url.hash, '#heading');
  assert.equal(url.searchParams.has('search'), false);
  assert.equal(url.searchParams.get('layer'), 'deos');
  assert.deepEqual(url.searchParams.getAll('tag'), ['a=b', 'x&y']);
  assert.equal(url.searchParams.get('highlight'), 'наблюдения %+&');
  assert.throws(() => searchResultLocation(base, 'https://other.invalid/', ['query']), /left the site/);
});

test('native UI rebinding touches script resources only and unknown source fails closed', () => {
  const body = '<main><code>searcher-old.js</code></main>';
  const result = rebindScriptAsset('<script src="../../searcher-old.js"></script>' + body, 'searcher-old.js', 'searcher-new.js');
  assert.equal(result.bindings, 1);
  assert.equal(result.content, '<script src="../../searcher-new.js"></script>' + body);
  assert.throws(() => rewriteNativeSearchUrls('function differentRenderer() {}'), /Pinned native/);
});
