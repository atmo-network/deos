/*
Domain: Reader search state regressions
Owns: Fragment-value isolation, Unicode/punctuation round trips, overlay open/closed state and native history decisions.
Excludes: Browser navigation, search ranking, layer membership and production origins.
*/
import assert from 'node:assert/strict';
import test from 'node:test';
import { readHashState } from '../src/hash-state.mjs';
import { nextSearchLocation, readSearchLocation, rebindScriptAsset, rewriteNativeSearchUrls, withoutSearchHighlight, searchResultLocation } from '../src/search-url.mjs';

const base = 'https://wiki.invalid/ru/deos/overview/actor.html#!layer=deos&tag=a%3Db&tag=x%26y&at=heading';
const fragmentless = 'https://wiki.invalid/ru/deos/overview/actor.html#!layer=deos&tag=a%3Db&tag=x%26y';

test('queries are opaque fragment values, never additional keys, paths or origins', () => {
  for (const query of ['observations', 'наблюдения', '% %25 + &=#?/', 'x&layer=example#bad', '%2F', '«Ёмкость»', 'a=b=c', 'https://evil.invalid/?x=y', "quotes'\"\\"]) {
    const next = nextSearchLocation(base, query, 'push_if_new_search_else_replace');
    const url = new URL(next.href), state = readHashState(next.href);
    assert.equal(readSearchLocation(next.href).query, query);
    assert.equal(url.origin, new URL(base).origin);
    assert.equal(url.pathname, new URL(base).pathname);
    assert.equal(url.search, '', 'search state must never reach the server-facing query');
    assert.equal(state.layer, 'deos');
    assert.equal(state.at, 'heading', 'a search write must keep the article section');
    assert.deepEqual(new URLSearchParams((url.hash.slice(2))).getAll('tag'), ['a=b', 'x&y']);
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
  assert.equal(nextSearchLocation(base, 'one', 'replace').action, 'replace');
  assert.throws(() => nextSearchLocation(base, 'one', 'unknown'), /Unsupported/);
  assert.throws(() => nextSearchLocation(base, null, 'replace'), /opaque string/);
});

test('the search overlay open state is the fragment, including an empty query', () => {
  // Opening with no text records an open, empty sheet instead of a bare navigation.
  const opened = nextSearchLocation(base, '', 'push');
  assert.equal(readSearchLocation(opened.href).query, '');
  assert.equal(opened.action, 'push');
  assert.equal(readHashState(opened.href).at, 'heading', 'opening search must not drop the section');
  const typed = nextSearchLocation(opened.href, 'актор', 'push_if_new_search_else_replace');
  assert.equal(typed.action, 'replace', 'typing after opening must not add history');
  assert.equal(readSearchLocation(typed.href).query, 'актор');
  const cleared = nextSearchLocation(typed.href, '', 'push_if_new_search_else_replace');
  assert.equal(readSearchLocation(cleared.href).query, '', 'clearing the field closes nothing');
  // Closing removes the state; a real query stays recoverable through Back.
  const closedTyped = nextSearchLocation(typed.href, '', 'close');
  assert.equal(readSearchLocation(closedTyped.href).query, null);
  assert.equal(closedTyped.action, 'push');
  const closedEmpty = nextSearchLocation(cleared.href, '', 'close');
  assert.equal(readSearchLocation(closedEmpty.href).query, null);
  assert.equal(closedEmpty.action, 'replace', 'an empty open state must not add phantom history');
  assert.equal(readHashState(closedTyped.href).layer, 'deos', 'closing search dropped independent state');
  assert.equal(new URL(closedTyped.href).search, '');
  assert.equal(readHashState(closedTyped.href).at, 'heading', 'closing search dropped the article section');
});

test('fragment reads decode exactly once and malformed escapes remain data rather than exceptions', () => {
  assert.deepEqual(readSearchLocation('https://wiki.invalid/#!search=%252F%2B&highlight=%2525'), { query: '%2F+', highlight: '%25' });
  assert.equal(readSearchLocation('https://wiki.invalid/#!search=a+b').query, 'a b');
  assert.equal(readSearchLocation('https://wiki.invalid/#!search=%').query, '%');
  assert.doesNotThrow(() => readSearchLocation('https://wiki.invalid/#!search=%E0%A4%A'));
});

test('dismissing highlights preserves active search, context and section', () => {
  const href = nextSearchLocation(fragmentless, 'наблюдения %', 'replace').href;
  const clean = readHashState(withoutSearchHighlight(href));
  assert.equal(clean.search, 'наблюдения %');
  assert.equal(clean.highlight, null);
  assert.equal(clean.layer, 'deos');
  assert.deepEqual(new URLSearchParams(new URL(withoutSearchHighlight(href)).hash.slice(2)).getAll('tag'), ['a=b', 'x&y']);
});

test('result links preserve independent context and the target section without the old query', () => {
  const href = nextSearchLocation(base, 'old', 'replace').href;
  const url = new URL(searchResultLocation(href, 'typed-observations.html#section', ['наблюдения', '%+&']));
  const state = readHashState(url.href);
  assert.equal(url.pathname, '/ru/deos/overview/typed-observations.html');
  assert.equal(url.search, '', 'a result destination must stay free of server-facing query state');
  assert.equal(state.search, null);
  assert.equal(state.layer, 'deos');
  assert.equal(state.at, 'section');
  assert.equal(state.highlight, 'наблюдения %+&');
  assert.deepEqual(new URLSearchParams(url.hash.slice(2)).getAll('tag'), ['a=b', 'x&y'], 'a result must keep unrelated reader context');
  assert.throws(() => searchResultLocation(base, 'https://other.invalid/', ['query']), /left the site/);
});

test('native UI rebinding touches script resources only and unknown source fails closed', () => {
  const body = '<main><code>searcher-old.js</code></main>';
  const result = rebindScriptAsset('<script src="../../searcher-old.js"></script>' + body, 'searcher-old.js', 'searcher-new.js');
  assert.equal(result.bindings, 1);
  assert.equal(result.content, '<script src="../../searcher-new.js"></script>' + body);
  assert.throws(() => rewriteNativeSearchUrls('function differentRenderer() {}'), /Pinned native/);
});
