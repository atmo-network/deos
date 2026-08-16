#!/usr/bin/env node

import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import test from 'node:test';
import {
  acceptLatest,
  atomicWrite,
  classify,
  defaultReferencePath,
  fetchLatestUpstream,
  loadAndVerifyPinned,
  parseVersion,
  sha256,
  synchronize,
} from './okf-reference.mjs';

const pinned = loadAndVerifyPinned();
const source = pinned.source;
const sourceWiki = resolve(dirname(defaultReferencePath), '../../../..', 'wiki');
const sandboxes = [];

function gitBlobSha(value) {
  return createHash('sha1').update(`blob ${value.length}\0`).update(value).digest('hex');
}

function response(body, { json = false, status = 200 } = {}) {
  const bytes = Buffer.isBuffer(body) ? body : Buffer.from(json ? JSON.stringify(body) : body);
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => JSON.parse(bytes.toString('utf8')),
    arrayBuffer: async () => bytes,
  };
}

function fakeFetchFor(candidate, options = {}) {
  const commit = options.commit ?? 'f'.repeat(40);
  return async (url, init) => {
    options.requests?.push({ url, headers: init.headers });
    if (url.includes('/commits?')) return response([{ sha: commit, commit: { committer: { date: '2026-08-16T00:00:00Z' } } }], { json: true });
    if (url.includes('/contents/')) {
      return response({
        path: options.metadataPath ?? pinned.lock.upstream.path,
        type: options.metadataType ?? 'file',
        sha: options.blobSha ?? gitBlobSha(candidate),
        size: options.size ?? candidate.length,
      }, { json: true });
    }
    if (url.includes('raw.githubusercontent.com')) return response(candidate);
    throw new Error(`unexpected URL ${url}`);
  };
}

async function latest(candidate, options) {
  return fetchLatestUpstream({ fetchImpl: fakeFetchFor(candidate, options), token: options?.token ?? '', lock: pinned.lock });
}

function sandbox(wikiVersion = pinned.lock.pinned_version) {
  const root = mkdtempSync(join(tmpdir(), 'deos-okf-reference-'));
  sandboxes.push(root);
  const referencePath = join(root, 'okf-reference.md');
  cpSync(defaultReferencePath, referencePath);
  const wiki = join(root, 'wiki');
  writeFileSync(join(root, 'placeholder'), '');
  return {
    root,
    referencePath,
    prepareWiki() {
      rmSync(wiki, { recursive: true, force: true });
      // mkdir is avoided in production publication; this fixture uses recursive copy for the directory.
      cpSync(sourceWiki, wiki, { recursive: true });
      const indexPath = join(wiki, 'index.md');
      writeFileSync(indexPath, readFileSync(indexPath, 'utf8').replace(/okf_version:\s*"[^"]+"/, `okf_version: "${wikiVersion}"`));
    },
  };
}

function adoptionOptions(box, extra = {}) {
  box.prepareWiki();
  return {
    root: box.root,
    referencePath: box.referencePath,
    reviewed: true,
    runTests: () => {},
    ...extra,
  };
}

function changedSameVersion() {
  return Buffer.from(source.toString('utf8').replace('OKF is an open,', 'OKF is a portable,'));
}

function changedVersion(version) {
  return Buffer.from(source.toString('utf8').replace('**Version 0.2**', `**Version ${version}**`));
}

test.after(() => {
  for (const path of sandboxes) rmSync(path, { recursive: true, force: true });
});

test('one Markdown atom binds source, body, upstream authority, and canonical adoption', () => {
  assert.equal(pinned.lock.schema_version, 2);
  assert.equal(sha256(pinned.body), pinned.lock.reference.body_sha256);
  assert.equal(sha256(pinned.source), pinned.lock.upstream.source_sha256);
  assert.equal(parseVersion(pinned.source).text, pinned.lock.adoption.okf_version);
  assert.equal(pinned.lock.adoption.status, 'adopted');
});

test('lifecycle classification exposes current and review-pending states', async (t) => {
  await t.test('current', async () => {
    const value = await latest(source, { commit: pinned.lock.upstream.commit });
    assert.deepEqual(classify(pinned, value), { state: 'current', change_kind: 'none', adoptable: false });
  });
  await t.test('same content at a newer commit remains current', async () => {
    const value = await latest(source);
    assert.deepEqual(classify(pinned, value), { state: 'current', change_kind: 'metadata-only', adoptable: false });
  });
  await t.test('same version', async () => {
    assert.deepEqual(classify(pinned, await latest(changedSameVersion())), { state: 'review-pending', change_kind: 'same-version', adoptable: true });
  });
  await t.test('minor version', async () => {
    assert.deepEqual(classify(pinned, await latest(changedVersion('0.3'))), { state: 'review-pending', change_kind: 'minor-version', adoptable: true });
  });
  await t.test('major version', async () => {
    assert.deepEqual(classify(pinned, await latest(changedVersion('1.0'))), { state: 'review-pending', change_kind: 'major-version', adoptable: true });
  });
  await t.test('downgrade', async () => {
    assert.deepEqual(classify(pinned, await latest(changedVersion('0.1'))), { state: 'review-pending', change_kind: 'downgrade', adoptable: false });
  });
});

test('successful same-version adoption returns only final adopted truth', async () => {
  const box = sandbox();
  const candidate = changedSameVersion();
  const result = await synchronize({
    mode: 'sync',
    referencePath: box.referencePath,
    fetchImpl: fakeFetchFor(candidate),
    ...adoptionOptions(box, { allowSameVersionRevision: true }),
  });
  assert.equal(result.state, 'adopted');
  assert.equal(result.change_kind, 'same-version');
  assert.equal(result.adoption_status, 'adopted');
  assert.equal(result.pinned_source_sha256, sha256(candidate));
  assert.equal(loadAndVerifyPinned(box.referencePath).lock.adoption.status, 'adopted');
});

test('successful minor-version adoption returns adopted after Wiki compatibility', async () => {
  const candidate = changedVersion('0.3');
  const box = sandbox('0.3');
  const result = await synchronize({
    mode: 'sync',
    referencePath: box.referencePath,
    fetchImpl: fakeFetchFor(candidate),
    ...adoptionOptions(box, { allowVersionChange: true }),
  });
  assert.equal(result.state, 'adopted');
  assert.equal(result.pinned_version, '0.3');
  assert.equal(loadAndVerifyPinned(box.referencePath).lock.adoption.okf_version, '0.3');
});

test('review and version flags fail closed without mutation', async (t) => {
  const cases = [
    ['missing reviewed', changedSameVersion(), {}, /requires --reviewed/],
    ['same version flag', changedSameVersion(), { reviewed: true }, /allow-same-version-revision/],
    ['minor flag', changedVersion('0.3'), { reviewed: true }, /allow-version-change/],
    ['major flag', changedVersion('1.0'), { reviewed: true }, /allow-breaking-version/],
  ];
  for (const [name, candidate, flags, pattern] of cases) {
    await t.test(name, async () => {
      const version = parseVersion(candidate).text;
      const box = sandbox(version);
      box.prepareWiki();
      const before = readFileSync(box.referencePath);
      const value = await latest(candidate);
      assert.throws(() => acceptLatest(loadAndVerifyPinned(box.referencePath), value, classify(pinned, value), {
        root: box.root,
        referencePath: box.referencePath,
        runTests: () => {},
        ...flags,
      }), pattern);
      assert.deepEqual(readFileSync(box.referencePath), before);
    });
  }
});

test('downgrade and Wiki-version incompatibility refuse without mutation', async (t) => {
  await t.test('downgrade', async () => {
    const box = sandbox('0.1');
    const before = readFileSync(box.referencePath);
    const value = await latest(changedVersion('0.1'));
    assert.throws(() => acceptLatest(loadAndVerifyPinned(box.referencePath), value, classify(pinned, value), adoptionOptions(box)), /downgrade refused/);
    assert.deepEqual(readFileSync(box.referencePath), before);
  });
  await t.test('Wiki version mismatch', async () => {
    const box = sandbox('0.2');
    const before = readFileSync(box.referencePath);
    const value = await latest(changedVersion('0.3'));
    assert.throws(() => acceptLatest(loadAndVerifyPinned(box.referencePath), value, classify(pinned, value), adoptionOptions(box, { allowVersionChange: true })), /has not adopted OKF 0.3/);
    assert.deepEqual(readFileSync(box.referencePath), before);
  });
});

test('strict-test failure refuses publication', async () => {
  const box = sandbox();
  const before = readFileSync(box.referencePath);
  const value = await latest(changedSameVersion());
  assert.throws(() => acceptLatest(loadAndVerifyPinned(box.referencePath), value, classify(pinned, value), adoptionOptions(box, {
    allowSameVersionRevision: true,
    runTests: () => { throw new Error('strict fixture failed'); },
  })), /strict fixture failed/);
  assert.deepEqual(readFileSync(box.referencePath), before);
});

test('malformed source and immutable metadata mismatches fail closed', async (t) => {
  await t.test('malformed version', async () => {
    const candidate = Buffer.from(source.toString('utf8').replace('**Version 0.2**', '**Version next**'));
    await assert.rejects(latest(candidate), /exactly one parseable/);
  });
  await t.test('path mismatch', async () => {
    await assert.rejects(latest(source, { metadataPath: 'other/SPEC.md' }), /mismatched OKF source metadata/);
  });
  await t.test('blob mismatch', async () => {
    await assert.rejects(latest(source, { blobSha: '0'.repeat(40) }), /does not match GitHub metadata/);
  });
  await t.test('size mismatch', async () => {
    await assert.rejects(latest(source, { size: source.length + 1 }), /does not match GitHub metadata/);
  });
});

test('embedded lock/body/source mismatches are rejected', async (t) => {
  await t.test('body mismatch', () => {
    const box = sandbox();
    const text = readFileSync(box.referencePath, 'utf8').replace('OKF is an open,', 'OKF is a changed,');
    writeFileSync(box.referencePath, text);
    assert.throws(() => loadAndVerifyPinned(box.referencePath), /body SHA-256 mismatch/);
  });
  await t.test('adoption status mismatch', () => {
    const box = sandbox();
    const text = readFileSync(box.referencePath, 'utf8').replace('status: adopted', 'status: accepted');
    writeFileSync(box.referencePath, text);
    assert.throws(() => loadAndVerifyPinned(box.referencePath), /adoption metadata/);
  });
});

test('public and authenticated requests use explicit GitHub headers', async (t) => {
  for (const token of ['', 'secret-token']) {
    await t.test(token ? 'authenticated' : 'public', async () => {
      const requests = [];
      await latest(source, { requests, token });
      assert.equal(requests.length, 3);
      for (const request of requests) {
        assert.equal(request.headers.accept, 'application/vnd.github+json');
        assert.equal(request.headers['x-github-api-version'], '2022-11-28');
        if (token) assert.equal(request.headers.authorization, `Bearer ${token}`);
        else assert.equal('authorization' in request.headers, false);
      }
    });
  }
});

test('check and unknown lifecycle states never mutate the atom', async (t) => {
  await t.test('check reports review-pending read-only', async () => {
    const box = sandbox();
    const before = readFileSync(box.referencePath);
    const result = await synchronize({ mode: 'check', referencePath: box.referencePath, fetchImpl: fakeFetchFor(changedSameVersion()) });
    assert.equal(result.state, 'review-pending');
    assert.deepEqual(readFileSync(box.referencePath), before);
  });
  await t.test('unknown keeps the valid pin', async () => {
    const box = sandbox();
    const before = readFileSync(box.referencePath);
    const result = await synchronize({ mode: 'sync', referencePath: box.referencePath, fetchImpl: async () => { throw new Error('offline'); } });
    assert.equal(result.state, 'unknown');
    assert.equal(result.pinned_valid, true);
    assert.deepEqual(readFileSync(box.referencePath), before);
  });
});

test('single-atom failure and crash injection always restart from one coherent state', async (t) => {
  await t.test('rename failure retains old adopted atom', async () => {
    const box = sandbox();
    const before = loadAndVerifyPinned(box.referencePath);
    const value = await latest(changedSameVersion());
    assert.throws(() => acceptLatest(before, value, classify(before, value), adoptionOptions(box, {
      allowSameVersionRevision: true,
      inject: (stage) => { if (stage === 'after-temporary-sync') throw new Error('simulated pre-rename crash'); },
    })), (error) => error.message.includes('pre-rename crash') && error.publication === 'not-published');
    const restarted = loadAndVerifyPinned(box.referencePath);
    assert.equal(restarted.lock.upstream.source_sha256, before.lock.upstream.source_sha256);
  });
  await t.test('crash after rename restarts from new adopted atom', async () => {
    const box = sandbox();
    const before = loadAndVerifyPinned(box.referencePath);
    const candidate = changedSameVersion();
    const value = await latest(candidate);
    assert.throws(() => acceptLatest(before, value, classify(before, value), adoptionOptions(box, {
      allowSameVersionRevision: true,
      inject: (stage) => { if (stage === 'after-rename') throw new Error('simulated post-rename crash'); },
    })), (error) => error.message.includes('post-rename crash') && error.publication === 'published-coherent');
    const restarted = loadAndVerifyPinned(box.referencePath);
    assert.equal(restarted.lock.upstream.source_sha256, sha256(candidate));
    assert.equal(restarted.lock.adoption.status, 'adopted');
  });
  await t.test('atomic writer replaces one file on success', () => {
    const box = sandbox();
    const path = join(box.root, 'atom');
    writeFileSync(path, 'old');
    atomicWrite(path, 'new');
    assert.equal(readFileSync(path, 'utf8'), 'new');
  });
});
