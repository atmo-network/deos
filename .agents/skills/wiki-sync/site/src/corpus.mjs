/*
Domain: Wiki corpus and routes
Owns: Declared plane loading, canonical page identity, localized manifests and build-time provenance.
Excludes: HTML presentation, hosting, browser state and Skill runtime dependencies.
*/
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseFrontmatter, readConcept, splitFrontmatter } from './frontmatter.mjs';
import { assertTrustedMarkdown } from './trust.mjs';

const atom = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
export const defaultWikiRoot = fileURLToPath(new URL('../../../../../wiki/', import.meta.url));

function requireAtom(value) {
  if (typeof value !== 'string' || !atom.test(value)) throw new Error(`Invalid route atom: ${value}`);
  return value;
}

export function pageRoute(locale, plane, sourcePath) {
  if (typeof sourcePath !== 'string' || !/^(?:[a-z0-9-]+\/)*[a-z0-9.-]+\.md$/.test(sourcePath) || sourcePath.split('/').includes('..')) throw new Error(`Invalid source route: ${sourcePath}`);
  return `/${requireAtom(locale)}/${requireAtom(plane)}/${sourcePath.slice(0, -3)}.html`;
}

function json(path) {
  return JSON.parse(readFileSync(path, 'utf8'));
}

function concepts(root) {
  const paths = [];
  for (const entry of readdirSync(root, { withFileTypes: true }).sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0)) {
    const path = join(root, entry.name);
    if (entry.isSymbolicLink()) throw new Error(`Symbolic content path is not supported: ${path}`);
    if (entry.isDirectory()) paths.push(...concepts(path));
    else if (entry.name.endsWith('.md') && !['index.md', 'log.md'].includes(entry.name)) paths.push(path);
  }
  return paths;
}

function loadPlane(wikiRoot, declaration) {
  const id = requireAtom(declaration.id);
  if (typeof declaration.title !== 'string' || !declaration.title || typeof declaration.required !== 'boolean') throw new Error(`Invalid plane declaration: ${id}`);
  const root = join(wikiRoot, id);
  const plane = { id, title: declaration.title, pages: new Map(), byPath: new Map(), navigation: { sections: [] }, graph: { nodes: [], edges: [] }, locales: [] };
  if (!existsSync(root)) {
    if (declaration.required) throw new Error(`Required plane is missing: ${id}`);
    return plane;
  }
  const { frontmatter } = splitFrontmatter(readFileSync(join(root, 'index.md'), 'utf8'));
  const declarationMeta = parseFrontmatter(frontmatter);
  if (declarationMeta.okf_version !== '0.2' || Object.keys(declarationMeta).length !== 1) throw new Error(`${id}: undeclared strict OKF v0.2 bundle`);
  const locales = json(join(root, '_meta/locales.json'));
  if (!Array.isArray(locales.supported_locales) || locales.supported_locales.length === 0 || !locales.supported_locales.includes(locales.default_locale)) throw new Error(`${id}: invalid locale manifest`);
  plane.locales = locales.supported_locales.map(requireAtom);
  plane.defaultLocale = locales.default_locale;
  const identities = new Set();
  for (const path of concepts(root)) {
    const { meta, body, text } = readConcept(path);
    assertTrustedMarkdown(text, path);
    const pageId = requireAtom(meta.canonical_page_id);
    const locale = requireAtom(meta.locale);
    const sourcePath = relative(root, path).replaceAll('\\', '/');
    if (!plane.locales.includes(locale) || locales.pages?.[pageId]?.[locale] !== sourcePath) throw new Error(`${id}/${sourcePath}: locale/path drift`);
    for (const field of ['type', 'title', 'description']) if (typeof meta[field] !== 'string' || !meta[field].trim()) throw new Error(`${id}/${sourcePath}: missing ${field}`);
    if (!['stable', 'draft', 'deprecated'].includes(meta.status ?? 'stable')) throw new Error(`${id}/${sourcePath}: invalid status`);
    if (!Array.isArray(meta.sources) || meta.sources.length === 0) throw new Error(`${id}/${sourcePath}: missing provenance`);
    for (const source of meta.sources) {
      const resource = source?.resource;
      if (typeof resource !== 'string' || !resource.trim()) throw new Error(`${id}/${sourcePath}: invalid provenance`);
      if (!/^[a-z][a-z0-9+.-]*:/i.test(resource)) {
        const target = resource.startsWith('/') ? resolve(root, resource.slice(1)) : resolve(dirname(path), resource);
        readFileSync(target);
      }
    }
    const key = `${locale}/${pageId}`;
    if (plane.pages.has(key)) throw new Error(`${id}: duplicate page identity ${key}`);
    const page = { id: pageId, locale, sourcePath, meta, body };
    plane.pages.set(key, page);
    plane.byPath.set(sourcePath, page);
    identities.add(pageId);
  }
  for (const [pageId, mirrors] of Object.entries(locales.pages ?? {})) {
    for (const locale of plane.locales) {
      const page = plane.pages.get(`${locale}/${pageId}`);
      if (!page || mirrors[locale] !== page.sourcePath) throw new Error(`${id}: missing mirror ${locale}/${pageId}`);
    }
  }
  for (const locale of plane.locales) if (!plane.pages.has(`${locale}/index`)) throw new Error(`${id}: missing ${locale} landing`);
  plane.navigation = json(join(root, '_meta/navigation.json'));
  plane.graph = json(join(root, '_meta/graph.json'));
  const graphIds = new Set(plane.graph.nodes.map(node => node.id));
  if (graphIds.size !== identities.size || plane.graph.nodes.length !== graphIds.size || [...graphIds].some(key => !identities.has(key))) throw new Error(`${id}: graph/page identity drift`);
  for (const edge of plane.graph.edges) if (!graphIds.has(edge.from) || !graphIds.has(edge.to) || typeof edge.type !== 'string' || !edge.type) throw new Error(`${id}: invalid graph edge`);
  for (const section of plane.navigation.sections) {
    for (const item of section.items) {
      for (const locale of plane.locales) {
        const page = plane.pages.get(`${locale}/${item.id}`);
        if (!page || item.path?.[locale] !== page.sourcePath) throw new Error(`${id}: navigation drift ${item.id}/${locale}`);
      }
    }
  }
  return plane;
}

export function loadCorpus(wikiRoot = defaultWikiRoot) {
  const manifest = json(join(wikiRoot, '_meta/planes.json'));
  if (!Array.isArray(manifest.planes) || manifest.planes.length === 0) throw new Error('No declared Wiki planes');
  const planes = manifest.planes.map(declaration => loadPlane(wikiRoot, declaration));
  if (new Set(planes.map(plane => plane.id)).size !== planes.length) throw new Error('Duplicate plane declaration');
  const base = planes.find(plane => plane.id === manifest.default_plane);
  if (!base || base.pages.size === 0) throw new Error('Default plane is missing or empty');
  const locales = [...new Set(planes.flatMap(plane => plane.locales))];
  if (locales.some(locale => !['en', 'ru'].includes(locale))) throw new Error('Website UI translation unavailable for a declared locale');
  return { planes, locales, defaultPlane: base.id, defaultLocale: base.defaultLocale, memberships: Object.hasOwn(manifest, 'memberships') ? manifest.memberships : {} };
}
