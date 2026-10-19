#!/usr/bin/env node
/*
Domain: Wiki projection materialization
Owns: Validated Markdown/JSON output beneath the website's generated target directory.
Excludes: mdBook execution, HTML rendering, network access and process lifecycle.
*/
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCorpus } from '../src/corpus.mjs';
import { projectBooks } from '../src/projection.mjs';

const [mode, destination] = process.argv.slice(2);
if (mode === '--help' || mode === '-h') {
  console.log('Usage: compile.mjs --out TARGET_SUBDIRECTORY | --locales PROJECTION_JSON');
} else if (mode === '--locales') {
  const projection = JSON.parse(readFileSync(destination, 'utf8'));
  if (!Array.isArray(projection.locales) || projection.locales.length === 0 || projection.locales.some(locale => !/^[a-z][a-z0-9-]*$/.test(locale))) throw new Error('Invalid projected locale set');
  console.log(projection.locales.join('\n'));
} else if (mode === '--out' && destination) {
  const target = fileURLToPath(new URL('../target/', import.meta.url));
  const output = resolve(destination);
  if (!output.startsWith(target.endsWith(sep) ? target : target + sep)) throw new Error('Projection output must be beneath the Skill site/target');
  const assets = Object.fromEntries(['wiki.css', 'wiki.js', 'layers.js'].map(name => [name, readFileSync(new URL(`../theme/${name}`, import.meta.url), 'utf8')]));
  const projected = projectBooks(loadCorpus(), assets);
  for (const [path, content] of projected.files) {
    const file = join(output, path);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, content);
  }
  console.log(`Wiki projection: ${projected.graph.nodes.length} nodes, ${projected.graph.edges.length} typed edges, ${projected.locales.length} locales; mdBook owns HTML rendering.`);
} else throw new Error('Use --out TARGET_SUBDIRECTORY or --locales PROJECTION_JSON');
