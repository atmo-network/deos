/*
Domain: Crawler metadata regressions
Owns: Site URL admission, page descriptions, canonical/hreflang links, robots.txt and sitemap.xml shape.
Excludes: Search-engine behavior, hosting headers and rendered-page layout.
*/
import assert from 'node:assert/strict';
import test from 'node:test';
import { loadCorpus } from '../src/corpus.mjs';
import { projectBooks } from '../src/projection.mjs';
import { annotate, headLinks, movedStub, pageEntries, robots, siteUrlFor, sitemap } from '../scripts/prepare-meta.mjs';
import { movedRoutes, resolveMovedRoutes } from '../src/routes.mjs';

const graph = projectBooks(loadCorpus(), {}, '/deos/').graph;
const site = 'https://owner.github.io/deos/';

test('the site URL must be an absolute http(s) URL whose path is the base path', () => {
  assert.equal(siteUrlFor('', '/deos/'), '');
  assert.equal(siteUrlFor(site, '/deos/'), site);
  for (const value of ['ftp://owner.github.io/deos/', 'https://owner.github.io/', 'https://owner.github.io/deos/?x=1', 'https://user:pass@owner.github.io/deos/', 'deos/']) assert.throws(() => siteUrlFor(value, '/deos/'));
});

test('every published page has one entry with its language alternates and a description', () => {
  const entries = pageEntries(graph);
  assert.equal(entries.length, graph.locales.length * (graph.nodes.length + 1));
  assert.equal(new Set(entries.map(entry => entry.path)).size, entries.length);
  for (const entry of entries) {
    assert.ok(!entry.path.startsWith('/'), `${entry.path}: entries are site-relative`);
    assert.ok(entry.description.trim(), `${entry.path}: missing description`);
    assert.deepEqual(Object.keys(entry.alternates).sort(), [...graph.locales].sort());
    assert.equal(entry.alternates[entry.locale], entry.path);
  }
});

test('a moved article keeps answering its published path through one redirect stub per locale', () => {
  const routes = resolveMovedRoutes(graph);
  assert.equal(routes.length, movedRoutes.length * graph.locales.length);
  const published = new Set(pageEntries(graph).map(entry => entry.path));
  for (const route of routes) {
    assert.ok(!published.has(route.from), `${route.from}: a moved path is never also a rendered page`);
    assert.ok(published.has(route.to), `${route.from}: the stub forwards to a published page`);
    const html = movedStub(route, site);
    assert.match(html, /<meta http-equiv="refresh" content="0; url=[^"]+">/);
    assert.match(html, /<meta name="robots" content="noindex, follow">/);
    assert.match(html, new RegExp(`<link rel="canonical" href="${site}${route.to}">`));
    assert.match(html, /location\.hash/, 'a deep link survives the move');
  }
  const glossary = routes.find(route => route.from === 'en/deos/glossary/core-terms.html');
  assert.deepEqual(glossary, { locale: 'en', from: 'en/deos/glossary/core-terms.html', to: 'en/deos/core-terms.html', pageId: 'core-terms' });
  assert.match(movedStub(glossary, ''), /url=\.\.\/core-terms\.html/, 'a build without a site URL stays relative');
  assert.doesNotMatch(movedStub(glossary, ''), /canonical/, 'no absolute claim without a known site URL');
  assert.deepEqual(resolveMovedRoutes({ ...graph, nodes: [] }), [], 'a probe artifact with no pages resolves no stubs');
});

test('page metadata fills the native description slot and adds absolute links only with a site URL', () => {
  const entry = pageEntries(graph).find(item => item.path === 'ru/deos/overview/actor-system.html');
  const html = '<head>\n<meta name="description" content="">\n</head>';
  const relativeOnly = annotate(html, entry, '', 'en', '../../../sitemap.xml');
  assert.match(relativeOnly, /<meta name="description" content="[^"]+">/);
  assert.doesNotMatch(relativeOnly, /canonical|hreflang/);
  const absolute = annotate(html, entry, site, 'en', '../../../sitemap.xml');
  assert.match(absolute, new RegExp(`<link rel="canonical" href="${site}ru/deos/overview/actor-system.html">`));
  assert.match(absolute, new RegExp(`hreflang="en" href="${site}en/deos/overview/actor-system.html"`));
  assert.match(absolute, /hreflang="x-default"/);
  assert.match(absolute, /<link rel="sitemap" type="application\/xml" title="Sitemap" href="..\/..\/..\/sitemap.xml">/);
  assert.throws(() => annotate('<head></head>', entry, site, 'en', 'sitemap.xml'), /description slot/);
});

test('HTML and sitemap publish the same canonical and ordered alternate targets', () => {
  const en = 'en/page.html?a=1&b=2', ru = 'ru/page.html';
  const entry = { path: ru, alternates: { en, ru } };
  const readAlternates = markup => [...markup.matchAll(/hreflang="([^"]+)" href="([^"]+)"/g)].map(([, locale, href]) => [locale, href]);
  const enHref = `${site}en/page.html?a=1&amp;b=2`;
  const ruHref = `${site}${ru}`;
  for (const [defaultLocale, fallback] of [['en', [['x-default', enHref]]], ['absent', []]]) {
    const html = headLinks(entry, site, defaultLocale, 'sitemap.xml');
    const xml = sitemap([entry], site, defaultLocale);
    const expected = [['en', enHref], ['ru', ruHref], ...fallback];
    assert.ok(html.includes(`<link rel="canonical" href="${ruHref}">`));
    assert.ok(xml.includes(`<loc>${ruHref}</loc>`));
    assert.deepEqual(readAlternates(html), expected);
    assert.deepEqual(readAlternates(xml), expected);
  }
});

test('sitemap lists absolute alternates without invented freshness, and robots points to it', () => {
  const entries = pageEntries(graph);
  const xml = sitemap(entries, site, 'en');
  assert.equal([...xml.matchAll(/<url>/g)].length, entries.length);
  assert.doesNotMatch(xml, /lastmod|changefreq|priority/);
  assert.ok([...xml.matchAll(/<loc>([^<]+)<\/loc>/g)].every(match => match[1].startsWith(site)));
  assert.match(xml, /xmlns:xhtml="http:\/\/www\.w3\.org\/1999\/xhtml"/);
  assert.equal(robots(''), 'User-agent: *\nAllow: /\n');
  assert.equal(robots(site), `User-agent: *\nAllow: /\nSitemap: ${site}sitemap.xml\n`);
});
