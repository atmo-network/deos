#!/usr/bin/env node
/*
Domain: Web-client validation tooling
Owns: Package-script launcher for the Domain DAG validator.
Excludes: Domain DAG rule semantics and project source-boundary policy.
Zone: Client-local tooling bridge to repo/agent validation infrastructure.
*/
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';

const writeOut = (message) => process.stdout.write(`${message}\n`);
const writeErr = (message) => process.stderr.write(`${message}\n`);

const args = process.argv.slice(2);
const helpRequested = args.some((arg) => arg === '--help' || arg === '-h');

if (helpRequested) {
  writeOut(`Usage: node scripts/validate-domain-dag.mjs [validator args]

Runs the Domain DAG validator against this web-client workspace.

Environment:
  DOMAIN_DAG_VALIDATOR  Absolute path to validate-domain-dag.sh
  SKILL_DIR             Domain DAG skill directory

Defaults:
  SKILL_DIR=~/.pi/agent/skills/domain-dag

Examples:
  npm run validate:dag
  npm run validate:dag -- --help
  DOMAIN_DAG_VALIDATOR=/path/to/validate-domain-dag.sh npm run validate:dag`);
  process.exit(0);
}

const skillDir =
  process.env.SKILL_DIR ?? join(homedir(), '.pi/agent/skills/domain-dag');
const validator =
  process.env.DOMAIN_DAG_VALIDATOR ??
  join(skillDir, 'scripts/validate-domain-dag.sh');
const hasRootArg = args.some(
  (arg) => arg === '--root' || arg.startsWith('--root='),
);
const validatorArgs = hasRootArg ? args : ['--root', '.', ...args];

if (!existsSync(validator)) {
  writeErr(
    [
      'Domain DAG validator not found.',
      `Expected: ${validator}`,
      'Set DOMAIN_DAG_VALIDATOR to the validator script path,',
      'or set SKILL_DIR to the domain-dag skill directory.',
    ].join('\n'),
  );
  process.exit(127);
}

const result = spawnSync('bash', [validator, ...validatorArgs], {
  cwd: process.cwd(),
  env: {
    ...process.env,
    SKILL_DIR: skillDir,
  },
  stdio: 'inherit',
});

if (result.error) {
  writeErr(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
