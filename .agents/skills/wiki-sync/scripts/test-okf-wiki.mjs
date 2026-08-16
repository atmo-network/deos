#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  cpSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { loadAndVerifyPinned } from './okf-reference.mjs';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const skillDir = resolve(scriptDir, '..');
const projectRoot = resolve(skillDir, '../../..');
const sourceWiki = join(projectRoot, 'wiki');
const validator = join(scriptDir, 'validate-okf-wiki.mjs');
const migrator = join(scriptDir, 'migrate-okf-wiki.mjs');
const baselineGraph = join(sourceWiki, '_meta/graph.json');
const okfReference = join(skillDir, 'references/okf-reference.md');
const sandboxes = [];

function sandbox() {
  const root = mkdtempSync(join(tmpdir(), 'deos-okf-wiki-'));
  sandboxes.push(root);
  cpSync(sourceWiki, join(root, 'wiki'), { recursive: true });
  for (const entry of readdirSync(projectRoot, { withFileTypes: true })) {
    if (entry.name === 'wiki' || entry.name === '.git' || entry.name === 'node_modules') continue;
    symlinkSync(join(projectRoot, entry.name), join(root, entry.name), entry.isDirectory() ? 'dir' : 'file');
  }
  return { root, wiki: join(root, 'wiki') };
}

test.after(() => {
  for (const path of sandboxes) rmSync(path, { recursive: true, force: true });
});

function run(script, args, expectedStatus = 0) {
  const result = spawnSync(process.execPath, [script, ...args], {
    cwd: projectRoot,
    encoding: 'utf8',
  });
  assert.equal(result.status, expectedStatus, `${result.stdout}\n${result.stderr}`);
  return `${result.stdout}\n${result.stderr}`;
}

function validate(wiki, expectedStatus = 0, withBaseline = false) {
  const args = ['--wiki-dir', wiki];
  if (withBaseline) args.push('--migration-baseline', baselineGraph);
  return run(validator, args, expectedStatus);
}

function mutateJson(path, mutate) {
  const value = JSON.parse(readFileSync(path, 'utf8'));
  mutate(value);
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function mutatePage(wiki, relativePath, mutate) {
  const path = join(wiki, relativePath);
  writeFileSync(path, mutate(readFileSync(path, 'utf8')));
}

test('bundled OKF reference retains its embedded body/source identity and adoption', () => {
  const reference = loadAndVerifyPinned(okfReference);
  assert.equal(reference.lock.pinned_version, '0.2');
  assert.equal(reference.lock.adoption.okf_version, '0.2');
  assert.equal(reference.lock.adoption.status, 'adopted');
});

test('current bundle and migration preservation baseline pass', () => {
  const { wiki } = sandbox();
  const output = validate(wiki, 0, true);
  assert.match(output, /94 concepts, 47 page IDs, 47 graph nodes, 214 typed edges/);
});

test('migration baseline fails closed when one prior typed edge is lost', () => {
  const { wiki } = sandbox();
  mutateJson(join(wiki, '_meta/graph.json'), (graph) => graph.edges.pop());
  assert.match(validate(wiki, 1, true), /migration baseline: missing prior typed edge/);
});

test('edge preservation is migration-scoped rather than a permanent graph freeze', () => {
  const { wiki } = sandbox();
  mutateJson(join(wiki, '_meta/graph.json'), (graph) => graph.edges.pop());
  validate(wiki);
});

test('graph rejects duplicate, invalid, and dangling typed edges', async (t) => {
  await t.test('duplicate identity', () => {
    const { wiki } = sandbox();
    mutateJson(join(wiki, '_meta/graph.json'), (graph) => graph.edges.push({ ...graph.edges[0] }));
    assert.match(validate(wiki, 1), /duplicate typed edge/);
  });
  await t.test('invalid type', () => {
    const { wiki } = sandbox();
    mutateJson(join(wiki, '_meta/graph.json'), (graph) => { graph.edges[0].type = 7; });
    assert.match(validate(wiki, 1), /type must be a non-empty string/);
  });
  await t.test('dangling endpoint', () => {
    const { wiki } = sandbox();
    mutateJson(join(wiki, '_meta/graph.json'), (graph) => { graph.edges[0].to = 'missing-page'; });
    assert.match(validate(wiki, 1), /dangling to endpoint missing-page/);
  });
});

test('conformant unknown type and block-scalar extension are tolerated', () => {
  const { wiki } = sandbox();
  mutatePage(wiki, 'concepts/generated-wiki.en.md', (text) => text
    .replace('type: concept', 'type: Future Knowledge Kind')
    .replace('status: stable', 'future_note: |\n  A producer-defined block scalar: valid YAML.\n  Consumers preserve this extension.\nstatus: stable'));
  validate(wiki);
});

test('invalid unquoted colon scalar and duplicate YAML key are rejected', async (t) => {
  await t.test('colon scalar', () => {
    const { wiki } = sandbox();
    mutatePage(wiki, 'concepts/generated-wiki.en.md', (text) => text.replace(/^description:.*$/m, 'description: Topic: details'));
    assert.match(validate(wiki, 1), /invalid YAML frontmatter/);
  });
  await t.test('duplicate key', () => {
    const { wiki } = sandbox();
    mutatePage(wiki, 'concepts/generated-wiki.en.md', (text) => text.replace('type: concept', 'type: concept\ntype: duplicate'));
    assert.match(validate(wiki, 1), /Map keys must be unique/);
  });
});

test('missing and malformed reserved root index fail', async (t) => {
  await t.test('missing index', () => {
    const { wiki } = sandbox();
    unlinkSync(join(wiki, 'index.md'));
    assert.match(validate(wiki, 1), /index\.md/);
  });
  await t.test('malformed index', () => {
    const { wiki } = sandbox();
    writeFileSync(join(wiki, 'index.md'), '---\nokf_version: [\n---\n# Broken\n');
    assert.match(validate(wiki, 1), /invalid YAML frontmatter/);
  });
});

test('scalar provenance and locale mirror drift fail', async (t) => {
  await t.test('scalar provenance', () => {
    const { wiki } = sandbox();
    mutatePage(wiki, 'concepts/generated-wiki.en.md', (text) => text.replace('  - resource: ../../docs/README.md', '  - ../../docs/README.md'));
    assert.match(validate(wiki, 1), /sources\[0\] must be a mapping/);
  });
  await t.test('missing mirror', () => {
    const { wiki } = sandbox();
    unlinkSync(join(wiki, 'concepts/generated-wiki.ru.md'));
    assert.match(validate(wiki, 1), /generated-wiki: missing ru mirror/);
  });
});

test('frontend manifest path drift fails', () => {
  const { wiki } = sandbox();
  mutateJson(join(wiki, '_meta/locales.json'), (locales) => { locales.pages['generated-wiki'].en = 'wrong.en.md'; });
  assert.match(validate(wiki, 1), /_meta\/locales\.json: path drift/);
});

test('migration is idempotent and preserves body bytes', () => {
  const root = mkdtempSync(join(tmpdir(), 'deos-okf-migration-'));
  sandboxes.push(root);
  const wiki = join(root, 'wiki');
  cpSync(sourceWiki, wiki, { recursive: true });
  const path = join(wiki, 'concepts/generated-wiki.en.md');
  const original = readFileSync(path, 'utf8');
  const delimiter = original.indexOf('\n---\n', 4);
  const body = `${original.slice(delimiter + 5)}\nAUTHORED-BODY-SENTINEL\n`;
  const legacy = original.slice(0, delimiter + 5)
    .replace('type: concept', 'page_type: concept')
    .replace('description:', 'summary:')
    .replace(/  - resource: /g, '  - ')
    .replace('status: stable', 'status: active');
  writeFileSync(path, `${legacy}${body}`);
  run(migrator, ['--wiki-dir', wiki, '--write']);
  const once = readFileSync(path, 'utf8');
  assert.equal(once.slice(once.indexOf('\n---\n', 4) + 5), body);
  run(migrator, ['--wiki-dir', wiki, '--write']);
  assert.equal(readFileSync(path, 'utf8'), once);
});
