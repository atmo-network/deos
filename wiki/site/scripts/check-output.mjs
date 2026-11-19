#!/usr/bin/env node
/*
Domain: Static mdBook output validation
Owns: Local document/asset targets, native heading fragments and published graph destinations.
Excludes: Markdown rendering, network link checking and generic untrusted-HTML sanitization.
*/
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { extname, join, relative, resolve, sep } from 'node:path';
import { pathToFileURL } from 'node:url';

export function checkOutput(directory) {
  const root = resolve(directory), pages = [];
  function walk(path) {
    for (const entry of readdirSync(path, { withFileTypes: true })) {
      const file = join(path, entry.name);
      if (entry.isSymbolicLink()) throw new Error(`Unexpected output symlink: ${file}`);
      if (entry.isDirectory()) walk(file);
      else if (extname(file) === '.html') pages.push(file);
    }
  }
  walk(root);
  if (!pages.length) throw new Error('No rendered HTML pages');
  const source = new Map(pages.map(path => [path, readFileSync(path, 'utf8')]));
  const ids = new Map(pages.map(path => [path, new Set([...source.get(path).matchAll(/\bid="([^"]+)"/g)].map(match => match[1]))]));
  const graph = JSON.parse(readFileSync(join(root, 'graph.json'), 'utf8'));
  // Every same-origin destination must stay beneath the deployment prefix the site was built for.
  const basePath = graph.base_path ?? '/';
  if (typeof basePath !== 'string' || !/^\/(?:[^/]+\/)*$/.test(basePath)) throw new Error('Invalid composed graph base path');
  const site = new URL(basePath, 'https://wiki-output.invalid/');
  let targets = 0;
  function targetFor(url, owner) {
    if (!url.pathname.startsWith(basePath)) throw new Error(`${owner}: destination ${url.pathname} is outside the site base path ${basePath}`);
    let target = resolve(root, './' + decodeURIComponent(url.pathname.slice(basePath.length)));
    if (target !== root && !target.startsWith(root + sep)) throw new Error(`${owner}: target escapes static output`);
    try {
      if (statSync(target).isDirectory()) target = join(target, 'index.html');
      if (!statSync(target).isFile()) throw new Error('not a file');
    } catch { throw new Error(`${owner}: missing output target ${url.pathname}`); }
    if (url.search && [...new URLSearchParams(url.search).keys()].some(key => ['layer', 'search', 'highlight'].includes(key))) throw new Error(`${owner}: reader state belongs to the fragment, not the query ${url.pathname}${url.search}`);
    if (url.hash && extname(target) === '.html') {
      // A stateful fragment carries its section; a plain one is the native anchor id.
      const fragment = url.hash.slice(1);
      const at = fragment.startsWith('!') ? new URLSearchParams(fragment.slice(1)).get('at') : decodeURIComponent(fragment);
      if (at && !ids.get(target)?.has(at)) throw new Error(`${owner}: missing heading target ${url.pathname}${url.hash}`);
    }
    targets++;
  }
  for (const path of pages) {
    const owner = relative(root, path).replaceAll('\\', '/');
    const base = new URL(owner, site);
    for (const tag of source.get(path).matchAll(/<(?:a|script|link|img|iframe)\b[^>]*>/gi)) {
      for (const attribute of tag[0].matchAll(/\b(?:href|src)="([^"]*)"/g)) {
        const url = new URL(attribute[1].replaceAll('&amp;', '&'), base);
        if (!['https:', 'http:', 'mailto:'].includes(url.protocol)) throw new Error(`${owner}: unsupported output URL scheme ${url.protocol}`);
        if (url.origin === base.origin) targetFor(url, owner);
      }
    }
    for (const script of source.get(path).matchAll(/<script>([\s\S]*?)<\/script>/g)) {
      const binding = script[1].match(/window\.path_to_searchindex_js = "([^"]+)";/);
      if (!binding) continue;
      const url = new URL(binding[1], base);
      if (url.origin !== base.origin) throw new Error(`${owner}: native search asset left the static origin`);
      targetFor(url, owner);
    }
  }
  if (!Array.isArray(graph.nodes) || !Array.isArray(graph.edges) || !Array.isArray(graph.linear_order)) throw new Error('Missing composed graph contract');
  for (const node of graph.nodes) for (const url of Object.values(node.url)) targetFor(new URL(url, site), node.id);
  if (graph.layers !== undefined) {
    if (!Array.isArray(graph.layers) || !Array.isArray(graph.locales)) throw new Error('Missing layer search contract');
    const expected = ['all', ...graph.layers.map(layer => layer.id)].sort();
    for (const locale of graph.locales) {
      const owner = `${locale}/search-layers.json`;
      targetFor(new URL(owner, site), owner);
      const manifest = JSON.parse(readFileSync(join(root, owner), 'utf8'));
      if (manifest.schema_version !== 1 || manifest.locale !== locale || !manifest.indexes || Object.keys(manifest.indexes).sort().join(',') !== expected.join(',')) throw new Error(`${owner}: invalid native layer search manifest`);
      if (typeof manifest.context_asset !== 'string' || !/^layer-context-[a-f0-9]{8}\.js$/.test(manifest.context_asset)) throw new Error(`${owner}: invalid reader context asset`);
      targetFor(new URL(`${locale}/${manifest.context_asset}`, site), owner);
      for (const [layer, entry] of Object.entries(manifest.indexes)) {
        const prefix = layer === 'all' ? 'searchindex-unicode-' : `searchindex-layer-${layer}-`;
        if (typeof entry.asset !== 'string' || !entry.asset.startsWith(prefix) || !/^[a-f0-9]{8}\.js$/.test(entry.asset.slice(prefix.length)) || !Number.isSafeInteger(entry.sections) || entry.sections < 0) throw new Error(`${owner}: invalid native layer search asset`);
        targetFor(new URL(`${locale}/${entry.asset}`, site), owner);
      }
    }
  }
  // A sitemap, when present, may list only real pages beneath one absolute site URL ending in the base path.
  let sitemapUrls = 0;
  try {
    const xml = readFileSync(join(root, 'sitemap.xml'), 'utf8');
    const urls = [...xml.matchAll(/<loc>([^<]+)<\/loc>|href="([^"]+)"/g)].map(match => (match[1] ?? match[2]).replaceAll('&amp;', '&'));
    if (!urls.length) throw new Error('sitemap.xml lists no pages');
    const origin = new URL(urls[0]);
    const prefix = new URL(basePath, origin).href;
    for (const url of urls) {
      if (!url.startsWith(prefix)) throw new Error(`sitemap.xml: ${url} is outside ${prefix}`);
      targetFor(new URL(url.slice(prefix.length), site), 'sitemap.xml');
      sitemapUrls++;
    }
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  return { basePath, sitemapUrls, htmlPages: pages.length, localTargets: targets, nodes: graph.nodes.length, edges: graph.edges.length };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  if (process.argv.includes('--help') || process.argv.includes('-h')) console.log('Usage: check-output.mjs STATIC_DIRECTORY');
  else {
    if (process.argv.length !== 3) throw new Error('Provide exactly one static output directory');
    console.log(`Static mdBook output passed: ${JSON.stringify(checkOutput(process.argv[2]))}`);
  }
}
