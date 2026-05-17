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
const defaultWikiDir = join(repoRoot, 'wiki');
const args = process.argv.slice(2);
const helpRequested = args.some((arg) => arg === '--help' || arg === '-h');

if (helpRequested) {
  writeOut(`Usage: node scripts/validate-wiki-trust.mjs [validator args]

Runs the trusted wiki markdown validator used by the browser renderer.

Environment:
  WIKI_TRUST_VALIDATOR  Absolute path to validate-wiki-trust.sh

Defaults:
  validator=<repo>/.agents/skills/wiki-sync/scripts/validate-wiki-trust.sh
  wiki-dir=<repo>/wiki

Examples:
  npm run validate:wiki
  npm run validate:wiki -- --help
  WIKI_TRUST_VALIDATOR=/path/to/validate-wiki-trust.sh npm run validate:wiki`);
  process.exit(0);
}

const validator = process.env.WIKI_TRUST_VALIDATOR ?? defaultValidator;
const hasWikiDirArg = args.some(
  (arg) => arg === '--wiki-dir' || arg.startsWith('--wiki-dir='),
);
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

const result = spawnSync('bash', [validator, ...validatorArgs], {
  cwd: webClientRoot,
  env: process.env,
  stdio: 'inherit',
});

if (result.error) {
  writeErr(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
