#!/usr/bin/env node
/*
Domain: Web-client validation tooling
Owns: Package-script launcher for the trusted wiki markdown validator.
Excludes: Wiki trust rule semantics and generated wiki content.
Zone: Client-local tooling bridge to repo/agent validation infrastructure.
*/
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const writeOut = (message) => process.stdout.write(`${message}\n`);
const writeErr = (message) => process.stderr.write(`${message}\n`);

const scriptDir = dirname(fileURLToPath(import.meta.url));
const webClientRoot = resolve(scriptDir, '..');
const repoRoot = resolve(webClientRoot, '..');
const defaultValidator = join(
  repoRoot,
  '.agents/skills/wiki-sync/scripts/validate-wiki-trust.sh',
);
const defaultConsolidationAuditor = join(
  repoRoot,
  '.agents/skills/wiki-sync/scripts/audit-wiki-consolidation.sh',
);
const defaultSearchIndexer = join(
  repoRoot,
  '.agents/skills/wiki-sync/scripts/build-search-index.mjs',
);
const defaultWikiDir = join(repoRoot, 'wiki');
const args = process.argv.slice(2);
const helpRequested = args.some((arg) => arg === '--help' || arg === '-h');

if (helpRequested) {
  writeOut(`Usage: node scripts/validate-wiki-trust.mjs [validator args]

Runs the trusted wiki markdown validator used by the browser renderer, then runs the wiki consolidation guard that prevents low-signal leaflet drift.

Environment:
  WIKI_TRUST_VALIDATOR  Absolute path to validate-wiki-trust.sh
  WIKI_CONSOLIDATION_AUDITOR  Absolute path to audit-wiki-consolidation.sh
  WIKI_SEARCH_INDEXER  Absolute path to build-search-index.mjs

Defaults:
  validator=<repo>/.agents/skills/wiki-sync/scripts/validate-wiki-trust.sh
  consolidation=<repo>/.agents/skills/wiki-sync/scripts/audit-wiki-consolidation.sh
  wiki-dir=<repo>/wiki

Examples:
  npm run validate:wiki
  npm run validate:wiki -- --help
  WIKI_TRUST_VALIDATOR=/path/to/validate-wiki-trust.sh npm run validate:wiki`);
  process.exit(0);
}

const validator = process.env.WIKI_TRUST_VALIDATOR ?? defaultValidator;
const consolidationAuditor =
  process.env.WIKI_CONSOLIDATION_AUDITOR ?? defaultConsolidationAuditor;
const searchIndexer = process.env.WIKI_SEARCH_INDEXER ?? defaultSearchIndexer;
const hasWikiDirArg = args.some(
  (arg) => arg === '--wiki-dir' || arg.startsWith('--wiki-dir='),
);
const wikiDirValueIndex = args.indexOf('--wiki-dir');
const wikiDirEqualsArg = args.find((arg) => arg.startsWith('--wiki-dir='));
const selectedWikiDir =
  (wikiDirValueIndex >= 0 ? args[wikiDirValueIndex + 1] : undefined) ??
  wikiDirEqualsArg?.slice('--wiki-dir='.length) ??
  defaultWikiDir;
const validatorArgs = hasWikiDirArg
  ? args
  : ['--wiki-dir', defaultWikiDir, ...args];

if (!existsSync(validator)) {
  writeErr(
    [
      'Wiki trust validator not found.',
      `Expected: ${validator}`,
      'Set WIKI_TRUST_VALIDATOR to the validator script path.',
    ].join('\n'),
  );
  process.exit(127);
}

if (!existsSync(consolidationAuditor)) {
  writeErr(
    [
      'Wiki consolidation auditor not found.',
      `Expected: ${consolidationAuditor}`,
      'Set WIKI_CONSOLIDATION_AUDITOR to the auditor script path.',
    ].join('\n'),
  );
  process.exit(127);
}

if (!existsSync(searchIndexer)) {
  writeErr(
    [
      'Wiki search indexer not found.',
      `Expected: ${searchIndexer}`,
      'Set WIKI_SEARCH_INDEXER to the indexer script path.',
    ].join('\n'),
  );
  process.exit(127);
}

const result = spawnSync('bash', [validator, ...validatorArgs], {
  cwd: webClientRoot,
  env: process.env,
  stdio: 'inherit',
});

if (result.error) {
  writeErr(result.error.message);
  process.exit(1);
}

if ((result.status ?? 1) !== 0) {
  process.exit(result.status ?? 1);
}

const searchResult = spawnSync(
  process.execPath,
  [searchIndexer, '--wiki-dir', selectedWikiDir, '--check'],
  {
    cwd: webClientRoot,
    env: process.env,
    stdio: 'inherit',
  },
);

if (searchResult.error) {
  writeErr(searchResult.error.message);
  process.exit(1);
}

if ((searchResult.status ?? 1) !== 0) {
  process.exit(searchResult.status ?? 1);
}

const consolidationResult = spawnSync(
  'bash',
  [consolidationAuditor, ...validatorArgs],
  {
    cwd: webClientRoot,
    env: process.env,
    stdio: 'inherit',
  },
);

if (consolidationResult.error) {
  writeErr(consolidationResult.error.message);
  process.exit(1);
}

process.exit(consolidationResult.status ?? 1);
