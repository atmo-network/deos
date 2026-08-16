#!/usr/bin/env node
/*
Domain: DEOS Actors golden-equivalence validation
Owns: Pinned 0.7.17 oracle identity, immutable corpus freshness, explicit anchor mappings, and proof coverage.
Excludes: Cargo execution, temporary worktree lifecycle, release publication, and runtime artifact generation.
Zone: Shared human/CI validator implementation; invoked through actors-golden-equivalence.sh.
*/
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(new URL('..', import.meta.url)));
const fixturePath = resolve(
  root,
  'template/pallets/actors/tests/fixtures/golden-equivalence.v1.json',
);
const fixture = JSON.parse(readFileSync(fixturePath, 'utf8'));
const authority = JSON.parse(readFileSync(resolve(root, 'scripts/validation-authority.v1.json'), 'utf8'));
const args = process.argv.slice(2);
const emitAnchors = args.includes('--anchors');
if (args.some((arg) => arg !== '--anchors')) {
  throw new Error('usage: validate-actors-golden-equivalence.mjs [--anchors]');
}

function git(...args) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
}
function sha256(value) {
  return createHash('sha256').update(value).digest('hex');
}
function requireValue(condition, message) {
  if (!condition) throw new Error(message);
}
function hasTest(source, symbol) {
  const escaped = symbol.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return new RegExp(`\\bfn\\s+${escaped}\\s*\\(`).test(source);
}

requireValue(
  fixture.schemaVersion === 'deos.actors-golden-equivalence/1',
  'unsupported golden-equivalence schemaVersion',
);
const { baseline } = fixture;
requireValue(
  /^[0-9a-f]{40}$/.test(baseline?.commit ?? ''),
  'baseline commit must be a full Git object id',
);
const baselineAuthority = authority.immutableRefInputs?.find(
  (input) => input.name === 'deos-actors-0.7.17-baseline',
);
requireValue(
  baselineAuthority?.commit === baseline.commit,
  'golden fixture and immutable validation authority disagree on baseline commit',
);
requireValue(
  git('rev-parse', `${baseline.commit}^{commit}`) === baseline.commit,
  'pinned baseline commit is unavailable',
);

for (const corpus of fixture.immutableCorpora ?? []) {
  requireValue(
    typeof corpus.path === 'string' && /^[0-9a-f]{64}$/.test(corpus.sha256),
    'every immutable corpus must have a path and SHA-256',
  );
  const current = readFileSync(resolve(root, corpus.path));
  const historical = execFileSync(
    'git',
    ['show', `${baseline.commit}:${corpus.path}`],
    { cwd: root },
  );
  requireValue(
    sha256(current) === corpus.sha256,
    `${corpus.kind} current corpus differs from the pinned oracle`,
  );
  requireValue(
    sha256(historical) === corpus.sha256,
    `${corpus.kind} baseline corpus differs from the pinned oracle`,
  );
}

const baselineTests = git(
  'show',
  `${baseline.commit}:template/pallets/actors/src/tests.rs`,
);
const currentTests = readFileSync(
  resolve(root, 'template/pallets/actors/src/tests.rs'),
  'utf8',
);
const mappings = new Map();
for (const mapping of fixture.intentionalMappings ?? []) {
  requireValue(
    mapping.kind === 'TestAnchorRename' &&
      typeof mapping.baseline === 'string' &&
      typeof mapping.current === 'string' &&
      typeof mapping.reason === 'string' &&
      mapping.reason.length > 0,
    'intentional mappings must name both anchors and a reason',
  );
  requireValue(!mappings.has(mapping.baseline), 'duplicate baseline mapping');
  mappings.set(mapping.baseline, mapping.current);
}

const covered = new Set();
const ids = new Set();
for (const anchor of fixture.semanticBehaviorAnchors ?? []) {
  requireValue(typeof anchor.id === 'string' && !ids.has(anchor.id), 'duplicate or missing semantic anchor id');
  ids.add(anchor.id);
  requireValue(hasTest(baselineTests, anchor.baseline), `missing baseline test: ${anchor.baseline}`);
  requireValue(hasTest(currentTests, anchor.current), `missing current test: ${anchor.current}`);
  if (anchor.baseline !== anchor.current) {
    requireValue(
      mappings.get(anchor.baseline) === anchor.current,
      `unreviewed cross-version anchor replacement: ${anchor.baseline}`,
    );
  }
  for (const proof of anchor.proves ?? []) covered.add(proof);
  if (emitAnchors) console.log(`${anchor.baseline}\t${anchor.current}\t${anchor.id}`);
}
for (const proof of fixture.requiredProofs ?? []) {
  requireValue(covered.has(proof), `golden equivalence lacks required proof: ${proof}`);
}
for (const [before, after] of mappings) {
  requireValue(
    (fixture.semanticBehaviorAnchors ?? []).some(
      (anchor) => anchor.baseline === before && anchor.current === after,
    ),
    `unused intentional mapping: ${before}`,
  );
}

if (!emitAnchors) {
  console.log(
    `Actors golden oracle valid: ${fixture.semanticBehaviorAnchors.length} semantic anchors, ${fixture.immutableCorpora.length} immutable corpora`,
  );
}
