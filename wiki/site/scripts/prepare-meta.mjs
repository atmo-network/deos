#!/usr/bin/env node
/*
Domain: Crawler-facing site metadata
Owns: Page descriptions, canonical/hreflang links, moved-path redirect stubs, robots.txt and sitemap.xml for the rendered artifact.
Excludes: Markdown/HTML rendering, freshness claims, network access and hosting configuration.
*/
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { pathToFileURL } from 'node:url';
import { movedRoutes, resolveMovedRoutes } from '../src/routes.mjs';

const contentsDescription = {
  en: 'Every published page of the wiki, grouped by section.',
  ru: 'Все опубликованные страницы вики, сгруппированные по разделам.',
};

const escapeAttribute = value => String(value).replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
const escapeXml = value => escapeAttribute(value).replaceAll("'", '&apos;');

// `siteUrl` is the absolute public URL of the site root (ending in its base path), or empty when
// unknown. Absolute canonical, hreflang and sitemap URLs exist only when it is known.
export function siteUrlFor(value, basePath) {
  if (!value) return '';
  const url = new URL(value);
  if (!['https:', 'http:'].includes(url.protocol) || url.search || url.hash || url.username || url.password) throw new Error(`Invalid site URL: ${value}`);
  if (url.pathname !== basePath) throw new Error(`Site URL path ${url.pathname} does not match base path ${basePath}`);
  return url.href;
}

// Site-relative page entries: every article in every locale plus each locale's contents page.
export function pageEntries(graph) {
  const base = graph.base_path ?? '/';
  const entries = [];
  for (const locale of graph.locales) {
    const alternates = Object.fromEntries(graph.locales.map(other => [other, `${other}/index.html`]));
    entries.push({ locale, path: `${locale}/index.html`, description: contentsDescription[locale] ?? '', alternates });
  }
  for (const node of graph.nodes) for (const [locale, route] of Object.entries(node.url)) {
    const alternates = Object.fromEntries(Object.entries(node.url).map(([other, url]) => [other, url.slice(base.length)]));
    entries.push({ locale, path: route.slice(base.length), description: node.description?.[locale] ?? '', alternates });
  }
  return entries;
}

export function headLinks(entry, siteUrl, defaultLocale, sitemapHref) {
  const links = [];
  if (siteUrl) {
    links.push(`<link rel="canonical" href="${escapeAttribute(new URL(entry.path, siteUrl).href)}">`);
    for (const [locale, path] of Object.entries(entry.alternates)) links.push(`<link rel="alternate" hreflang="${locale}" href="${escapeAttribute(new URL(path, siteUrl).href)}">`);
    if (entry.alternates[defaultLocale]) links.push(`<link rel="alternate" hreflang="x-default" href="${escapeAttribute(new URL(entry.alternates[defaultLocale], siteUrl).href)}">`);
    links.push(`<link rel="sitemap" type="application/xml" title="Sitemap" href="${escapeAttribute(sitemapHref)}">`);
  }
  return links.join('\n        ');
}

export function annotate(html, entry, siteUrl, defaultLocale, sitemapHref) {
  const empty = '<meta name="description" content="">';
  if (!html.includes(empty)) throw new Error(`${entry.path}: pinned mdBook description slot changed`);
  let next = html.replace(empty, `<meta name="description" content="${escapeAttribute(entry.description)}">`);
  const links = headLinks(entry, siteUrl, defaultLocale, sitemapHref);
  if (links) next = next.replace('</head>', `        ${links}\n    </head>`);
  return next;
}

export function sitemap(entries, siteUrl, defaultLocale) {
  const urls = entries.map(entry => {
    const alternates = Object.entries(entry.alternates).map(([locale, path]) => `    <xhtml:link rel="alternate" hreflang="${locale}" href="${escapeXml(new URL(path, siteUrl).href)}"/>`);
    if (entry.alternates[defaultLocale]) alternates.push(`    <xhtml:link rel="alternate" hreflang="x-default" href="${escapeXml(new URL(entry.alternates[defaultLocale], siteUrl).href)}"/>`);
    // No <lastmod>: the wiki records no page freshness, so none is invented here.
    return `  <url>\n    <loc>${escapeXml(new URL(entry.path, siteUrl).href)}</loc>\n${alternates.join('\n')}\n  </url>`;
  });
  return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${urls.join('\n')}\n</urlset>\n`;
}

// A moved article keeps answering its published URL. The stub is deliberately not a rendered page:
// it carries no body content, tells crawlers the real owner through canonical, and forwards the
// fragment so a deep link survives the move.
export function movedStub(route, siteUrl) {
  const target = relative(dirname(route.from), route.to).split('\\').join('/');
  const absolute = siteUrl ? new URL(route.to, siteUrl).href : '';
  const canonical = absolute ? `<link rel="canonical" href="${escapeAttribute(absolute)}">\n` : '';
  return `<!DOCTYPE html>
<html lang="${route.locale}">
<head>
<meta charset="utf-8">
<title>Moved — DEOS Wiki</title>
<meta name="robots" content="noindex, follow">
${canonical}<meta http-equiv="refresh" content="0; url=${escapeAttribute(target)}">
<script>location.replace("${target}" + location.hash);</script>
</head>
<body>
<p><a href="${escapeAttribute(target)}">${escapeAttribute(route.to)}</a></p>
</body>
</html>
`;
}

export function writeMovedStubs(root, graph, entries, siteUrl) {
  const written = [];
  for (const route of resolveMovedRoutes(graph)) {
    if (entries.has(route.from)) throw new Error(`Moved path ${route.from} is also a published page`);
    if (!entries.has(route.to)) throw new Error(`Moved path ${route.from} points at unknown page ${route.to}`);
    const file = join(root, route.from);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, movedStub(route, siteUrl));
    written.push(route.from);
  }
  const expected = movedRoutes.length * (graph.locales?.length ?? 0);
  if (written.length !== expected) throw new Error(`Wrote ${written.length} of ${expected} moved-path stubs`);
  return written;
}

export function robots(siteUrl) {
  return `User-agent: *\nAllow: /\n${siteUrl ? `Sitemap: ${new URL('sitemap.xml', siteUrl).href}\n` : ''}`;
}

export function prepareMeta(root, graph, siteUrl) {
  const defaultLocale = graph.default_locale;
  const entries = new Map(pageEntries(graph).map(entry => [entry.path, entry]));
  let annotated = 0;
  function walk(directory) {
    for (const item of readdirSync(directory, { withFileTypes: true })) {
      const file = join(directory, item.name);
      if (item.isDirectory()) { walk(file); continue; }
      if (!item.name.endsWith('.html')) continue;
      const path = relative(root, file).replaceAll('\\', '/');
      const html = readFileSync(file, 'utf8');
      const depth = path.split('/').length - 1;
      const sitemapHref = '../'.repeat(depth) + 'sitemap.xml';
      if (entries.has(path)) {
        writeFileSync(file, annotate(html, entries.get(path), siteUrl, defaultLocale, sitemapHref));
        annotated++;
      } else if (/(^|\/)(404|toc)\.html$/.test(path)) {
        // Navigation fragments and error pages are reachable but not search results.
        writeFileSync(file, html.replace('</head>', '        <meta name="robots" content="noindex, follow">\n    </head>'));
      }
    }
  }
  walk(root);
  if (annotated !== entries.size) throw new Error(`Annotated ${annotated} of ${entries.size} published pages`);
  // The site-root redirect points crawlers at the default landing and its language alternates.
  const index = join(root, 'index.html');
  const landing = graph.nodes.find(node => node.id.endsWith('/index'));
  if (siteUrl && landing) {
    const entry = pageEntries(graph).find(item => item.path === landing.url[defaultLocale].slice((graph.base_path ?? '/').length));
    writeFileSync(index, readFileSync(index, 'utf8').replace('</head>', `${headLinks(entry, siteUrl, defaultLocale, 'sitemap.xml')}\n</head>`));
  }
  const moved = writeMovedStubs(root, graph, entries, siteUrl);
  writeFileSync(join(root, 'robots.txt'), robots(siteUrl));
  if (siteUrl) writeFileSync(join(root, 'sitemap.xml'), sitemap([...entries.values()], siteUrl, defaultLocale));
  return { annotated, sitemap: siteUrl ? entries.size : 0, moved: moved.length };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  if (process.argv.includes('--help') || process.argv.includes('-h')) console.log('Usage: prepare-meta.mjs STATIC_DIRECTORY PROJECTION_JSON  (WIKI_SITE_URL enables absolute links and sitemap.xml)');
  else {
    const [root, projection] = process.argv.slice(2);
    if (!root || !projection) throw new Error('Provide the static directory and projection JSON');
    const graph = JSON.parse(readFileSync(projection, 'utf8'));
    const siteUrl = siteUrlFor(process.env.WIKI_SITE_URL, graph.base_path ?? '/');
    console.log(`Crawler metadata: ${JSON.stringify(prepareMeta(root, graph, siteUrl))}${siteUrl ? ` at ${siteUrl}` : ' (no WIKI_SITE_URL: relative-only, no sitemap)'}`);
  }
}
