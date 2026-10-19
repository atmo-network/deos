#!/usr/bin/env node
/*
Domain: Wiki trust command
Owns: Deterministic directory validation through the shared reviewed-content contract.
Excludes: HTML rendering, provenance inference and process orchestration.
*/
import { readFileSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { assertTrustedMarkdown } from '../src/trust.mjs';

function check(root) {
  let count = 0;
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isSymbolicLink()) throw new Error(`Symbolic content path is not supported: ${path}`);
    if (entry.isDirectory()) count += check(path);
    else if (entry.name.endsWith('.md')) { assertTrustedMarkdown(readFileSync(path, 'utf8'), path); count++; }
  }
  return count;
}
if (process.argv.includes('--help') || process.argv.includes('-h')) console.log('Usage: check-trust.mjs [--wiki-dir PATH]');
else {
  const args = process.argv.slice(2);
  if (args.length && (args.length !== 2 || args[0] !== '--wiki-dir')) throw new Error('Use --wiki-dir PATH');
  console.log(`Reviewed Wiki Markdown passed: ${check(resolve(args[1] ?? fileURLToPath(new URL('../../../../../wiki/deos/', import.meta.url))))} files.`);
}
