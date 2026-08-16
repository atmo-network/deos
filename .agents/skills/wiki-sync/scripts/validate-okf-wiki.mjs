#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import { isAbsolute, join, relative, resolve } from 'node:path';
import { parseFrontmatter, readConcept, splitFrontmatter } from './okf-frontmatter.mjs';

function usage() {
  console.log(`Usage: validate-okf-wiki.mjs [--wiki-dir PATH]
  [--migration-baseline PATH | --migration-baseline-ref REF]

Validate /wiki as an explicitly declared strict OKF v0.2 bundle while preserving
DEOS locale, graph, and frontend-manifest contracts. A migration baseline checks
that every legacy page ID and typed edge remains present without constraining
later graph evolution.`);
}

let wikiDir = resolve(process.cwd(), 'wiki');
let migrationBaselinePath = null;
let migrationBaselineRef = null;
for (let index = 2; index < process.argv.length; index += 1) {
  const argument = process.argv[index];
  if (argument === '-h' || argument === '--help') {
    usage();
    process.exit(0);
  }
  const readValue = (name) => {
    const value = process.argv[++index];
    if (!value) throw new Error(`Missing value for ${name}`);
    return value;
  };
  if (argument === '--wiki-dir') wikiDir = resolve(readValue('--wiki-dir'));
  else if (argument.startsWith('--wiki-dir=')) wikiDir = resolve(argument.slice('--wiki-dir='.length));
  else if (argument === '--migration-baseline') migrationBaselinePath = resolve(readValue('--migration-baseline'));
  else if (argument.startsWith('--migration-baseline=')) migrationBaselinePath = resolve(argument.slice('--migration-baseline='.length));
  else if (argument === '--migration-baseline-ref') migrationBaselineRef = readValue('--migration-baseline-ref');
  else if (argument.startsWith('--migration-baseline-ref=')) migrationBaselineRef = argument.slice('--migration-baseline-ref='.length);
  else throw new Error(`Unknown argument: ${argument}`);
}
if (migrationBaselinePath && migrationBaselineRef) throw new Error('Choose only one migration baseline source');

const failures = [];
const conceptPaths = [];
function walk(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) walk(path);
    else if (entry.isFile() && entry.name.endsWith('.md')) {
      const rel = relative(wikiDir, path).replaceAll('\\', '/');
      if (entry.name === 'index.md' || entry.name === 'log.md') {
        if (rel !== 'index.md') failures.push(`${rel}: nested reserved files are not part of the DEOS bundle contract`);
      } else conceptPaths.push(path);
    }
  }
}
try {
  walk(wikiDir);
} catch (error) {
  failures.push(`wiki tree: ${error.message}`);
}

try {
  const rootText = readFileSync(join(wikiDir, 'index.md'), 'utf8');
  const { frontmatter, body } = splitFrontmatter(rootText, 'index.md');
  const meta = parseFrontmatter(frontmatter, 'index.md');
  if (Object.keys(meta).length !== 1 || meta.okf_version !== '0.2') {
    failures.push('index.md: root index must declare only okf_version: "0.2"');
  }
  if (!/^#\s+\S/m.test(body) || !/^\* \[[^\]]+\]\([^\)]+\) - \S/m.test(body)) {
    failures.push('index.md: reserved root index must contain grouped Markdown link entries with descriptions');
  }
} catch (error) {
  failures.push(error.message);
}

const pages = new Map();
for (const path of conceptPaths.sort()) {
  const rel = relative(wikiDir, path).replaceAll('\\', '/');
  try {
    const { meta } = readConcept(path);
    if (typeof meta.type !== 'string' || !meta.type.trim()) failures.push(`${rel}: missing non-empty type`);
    if (typeof meta.description !== 'string' || !meta.description.trim()) failures.push(`${rel}: missing non-empty description`);
    if ('page_type' in meta || 'summary' in meta) failures.push(`${rel}: noncanonical page_type/summary compatibility field remains`);
    for (const forbidden of ['generated', 'verified', 'stale_after', 'attester', 'executor', 'computation']) {
      if (forbidden in meta) failures.push(`${rel}: unsupported unevidenced ${forbidden} field`);
    }
    if (!['stable', 'draft', 'deprecated'].includes(meta.status ?? 'stable')) failures.push(`${rel}: invalid OKF status ${meta.status}`);
    if (!Array.isArray(meta.sources) || meta.sources.length === 0) failures.push(`${rel}: sources must be a non-empty structured list`);
    else {
      for (const [index, source] of meta.sources.entries()) {
        if (!source || typeof source !== 'object' || Array.isArray(source) || typeof source.resource !== 'string' || !source.resource.trim()) {
          failures.push(`${rel}: sources[${index}] must be a mapping with non-empty resource`);
          continue;
        }
        const resource = source.resource;
        if (!isAbsolute(resource) && !/^[a-z][a-z0-9+.-]*:/i.test(resource)) {
          try {
            readFileSync(resolve(path, '..', resource));
          } catch {
            failures.push(`${rel}: missing source resource ${resource}`);
          }
        }
      }
    }
    if (typeof meta.canonical_page_id !== 'string' || !meta.canonical_page_id) failures.push(`${rel}: missing canonical_page_id extension`);
    if (!['en', 'ru'].includes(meta.locale)) failures.push(`${rel}: invalid locale extension ${meta.locale}`);
    if (!rel.endsWith(`.${meta.locale}.md`)) failures.push(`${rel}: locale does not match filename suffix`);
    const key = `${meta.canonical_page_id}/${meta.locale}`;
    if (pages.has(key)) failures.push(`${rel}: duplicate page identity ${key}`);
    pages.set(key, { rel, meta });
  } catch (error) {
    failures.push(error.message);
  }
}

const ids = new Set([...pages.values()].map(({ meta }) => meta.canonical_page_id));
for (const id of ids) {
  for (const locale of ['en', 'ru']) if (!pages.has(`${id}/${locale}`)) failures.push(`${id}: missing ${locale} mirror`);
}

function loadMigrationBaseline() {
  if (migrationBaselinePath) return JSON.parse(readFileSync(migrationBaselinePath, 'utf8'));
  if (!migrationBaselineRef) return null;
  const projectRoot = execFileSync('git', ['-C', wikiDir, 'rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim();
  const wikiRelative = relative(projectRoot, wikiDir).replaceAll('\\', '/');
  const show = (path) => execFileSync('git', ['-C', projectRoot, 'show', `${migrationBaselineRef}:${wikiRelative}/${path}`], { encoding: 'utf8' });
  let legacyIndex;
  try {
    legacyIndex = show('index.en.md');
  } catch {
    throw new Error(`migration baseline ${migrationBaselineRef}: legacy index.en.md is unavailable`);
  }
  if (!/^page_type:/m.test(legacyIndex) || /^type:/m.test(legacyIndex)) return null;
  return JSON.parse(show('_meta/graph.json'));
}

function edgeIdentity(edge) {
  return JSON.stringify([edge.from, edge.to, edge.type]);
}

let graph = null;
try {
  const readManifest = (name) => JSON.parse(readFileSync(join(wikiDir, `_meta/${name}.json`), 'utf8'));
  graph = readManifest('graph');
  const state = readManifest('state');
  const navigation = readManifest('navigation');
  const locales = readManifest('locales');
  const aliases = readManifest('aliases');
  if (!graph || !Array.isArray(graph.nodes) || !Array.isArray(graph.edges)) throw new Error('_meta/graph.json: nodes and edges must be arrays');

  const graphIds = new Set();
  for (const [index, node] of graph.nodes.entries()) {
    if (!node || typeof node !== 'object' || Array.isArray(node) || typeof node.id !== 'string' || !node.id) {
      failures.push(`_meta/graph.json: nodes[${index}] must have a non-empty string id`);
      continue;
    }
    if (graphIds.has(node.id)) failures.push(`_meta/graph.json: duplicate node id ${node.id}`);
    graphIds.add(node.id);
  }
  const edgeIds = new Set();
  for (const [index, edge] of graph.edges.entries()) {
    if (!edge || typeof edge !== 'object' || Array.isArray(edge)) {
      failures.push(`_meta/graph.json: edges[${index}] must be a mapping`);
      continue;
    }
    for (const field of ['from', 'to', 'type']) {
      if (typeof edge[field] !== 'string' || !edge[field].trim()) failures.push(`_meta/graph.json: edges[${index}].${field} must be a non-empty string`);
    }
    if (typeof edge.from !== 'string' || typeof edge.to !== 'string' || typeof edge.type !== 'string') continue;
    if (!graphIds.has(edge.from)) failures.push(`_meta/graph.json: edges[${index}] has dangling from endpoint ${edge.from}`);
    if (!graphIds.has(edge.to)) failures.push(`_meta/graph.json: edges[${index}] has dangling to endpoint ${edge.to}`);
    const identity = edgeIdentity(edge);
    if (edgeIds.has(identity)) failures.push(`_meta/graph.json: duplicate typed edge ${identity}`);
    edgeIds.add(identity);
  }

  const stateIds = new Set(Object.keys(state.pages ?? {}));
  const localeIds = new Set(Object.keys(locales.pages ?? {}));
  for (const [name, manifestIds] of [['_meta/graph.json', graphIds], ['_meta/state.json', stateIds], ['_meta/locales.json', localeIds]]) {
    for (const id of ids) if (!manifestIds.has(id)) failures.push(`${name}: missing page ${id}`);
    for (const id of manifestIds) if (!ids.has(id)) failures.push(`${name}: stale page ${id}`);
  }

  const navigationItems = new Map();
  function collectItems(value) {
    if (Array.isArray(value)) for (const child of value) collectItems(child);
    else if (value && typeof value === 'object') {
      if (typeof value.id === 'string' && value.path) navigationItems.set(value.id, value);
      for (const child of Object.values(value)) collectItems(child);
    }
  }
  collectItems(navigation.sections);
  for (const [id, item] of navigationItems) if (!ids.has(id)) failures.push(`_meta/navigation.json: stale page ${id}`);
  for (const locale of ['en', 'ru']) {
    for (const [alias, target] of Object.entries(aliases.aliases?.[locale] ?? {})) {
      if (typeof target !== 'string' || !ids.has(target)) failures.push(`_meta/aliases.json: ${locale} alias ${alias} targets missing page ${target}`);
    }
  }
  for (const id of ids) {
    const node = graph.nodes.find((candidate) => candidate.id === id);
    const statePage = state.pages?.[id];
    const localePage = locales.pages?.[id];
    for (const locale of ['en', 'ru']) {
      const page = pages.get(`${id}/${locale}`);
      if (!page || !node || !statePage || !localePage) continue;
      const { rel } = page;
      if (node.path?.[locale] !== rel) failures.push(`_meta/graph.json: path drift for ${id}/${locale}`);
      if (statePage.path?.[locale] !== rel) failures.push(`_meta/state.json: path drift for ${id}/${locale}`);
      if (localePage[locale] !== rel) failures.push(`_meta/locales.json: path drift for ${id}/${locale}`);
      const item = navigationItems.get(id);
      if (item && item.path?.[locale] !== rel) failures.push(`_meta/navigation.json: path drift for ${id}/${locale}`);
    }
  }

  const baseline = loadMigrationBaseline();
  if (baseline) {
    if (!Array.isArray(baseline.nodes) || !Array.isArray(baseline.edges)) throw new Error('migration baseline: nodes and edges must be arrays');
    for (const node of baseline.nodes) if (!graphIds.has(node.id)) failures.push(`migration baseline: missing prior page ID ${node.id}`);
    for (const edge of baseline.edges) {
      const identity = edgeIdentity(edge);
      if (!edgeIds.has(identity)) failures.push(`migration baseline: missing prior typed edge ${identity}`);
    }
  }
} catch (error) {
  failures.push(`frontend manifests: ${error.message}`);
}

if (failures.length) {
  console.error('Strict OKF v0.2 wiki validation failed:');
  for (const failure of failures) console.error(`[FAIL] ${failure}`);
  process.exit(1);
}
console.log(`Strict OKF v0.2 wiki bundle passed: ${conceptPaths.length} concepts, ${ids.size} page IDs, ${graph.nodes.length} graph nodes, ${graph.edges.length} typed edges.`);
