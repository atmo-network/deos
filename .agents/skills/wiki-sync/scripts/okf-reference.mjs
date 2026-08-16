#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  closeSync,
  fsyncSync,
  openSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseFrontmatter, splitFrontmatter } from './okf-frontmatter.mjs';

const scriptDir = dirname(fileURLToPath(import.meta.url));
export const skillDir = resolve(scriptDir, '..');
export const projectRoot = resolve(skillDir, '../../..');
export const defaultReferencePath = join(skillDir, 'references/okf-reference.md');
const repository = 'GoogleCloudPlatform/knowledge-catalog';
const upstreamPath = 'okf/SPEC.md';
const upstreamRef = 'main';
const sourceUrl = `https://github.com/${repository}/blob/${upstreamRef}/${upstreamPath}`;
const embedding = 'deos-reference-v2-trim-final-newlines';
const versionPattern = /^\*\*Version ([0-9]+)\.([0-9]+)\*\*$/gm;
let temporarySequence = 0;

export function sha256(value) {
  return createHash('sha256').update(value).digest('hex');
}

function gitBlobSha(value) {
  return createHash('sha1').update(`blob ${value.length}\0`).update(value).digest('hex');
}

export function parseVersion(source) {
  const matches = [...source.toString('utf8').matchAll(versionPattern)];
  if (matches.length !== 1) throw new Error('OKF source must contain exactly one parseable **Version major.minor** declaration');
  return { text: `${matches[0][1]}.${matches[0][2]}`, major: Number(matches[0][1]), minor: Number(matches[0][2]) };
}

function compareVersion(left, right) {
  return left.major === right.major ? Math.sign(left.minor - right.minor) : Math.sign(left.major - right.major);
}

function scalar(value) {
  return JSON.stringify(value);
}

export function renderReference(source, upstream) {
  const version = upstream.version.text;
  const body = Buffer.from(`${sourceUrl}\n\n${source.toString('utf8').replace(/\n+$/u, '')}`);
  const frontmatter = `---
type: Reference
title: Open Knowledge Format v${version}
description: Full supplied specification for the Open Knowledge Format version ${version}.
status: stable
sources:
  - resource: ${sourceUrl}
    title: Upstream OKF specification
lock:
  schema_version: 2
  standard: Open Knowledge Format
  pinned_version: ${scalar(version)}
  upstream:
    repository: ${repository}
    path: ${upstreamPath}
    ref: ${upstreamRef}
    commit: ${upstream.commit}
    revision_date: ${scalar(upstream.revisionDate)}
    blob_sha1: ${upstream.blobSha1}
    source_sha256: ${upstream.sourceSha256}
    source_trailing_newlines: ${upstream.sourceTrailingNewlines}
  reference:
    embedding: ${embedding}
    body_sha256: ${sha256(body)}
  adoption:
    bundle: wiki/index.md
    okf_version: ${scalar(version)}
    status: adopted
---
`;
  return Buffer.concat([Buffer.from(frontmatter), body]);
}

function requireString(value, label, pattern) {
  if (typeof value !== 'string' || (pattern && !pattern.test(value))) throw new Error(`Malformed OKF reference ${label}`);
}

export function loadAndVerifyPinned(referencePath = defaultReferencePath) {
  const reference = readFileSync(referencePath);
  const text = reference.toString('utf8');
  const split = splitFrontmatter(text, referencePath);
  const meta = parseFrontmatter(split.frontmatter, referencePath);
  const lock = meta.lock;
  if (!lock || lock.schema_version !== 2 || lock.standard !== 'Open Knowledge Format') throw new Error('Malformed OKF reference lock metadata');
  requireString(lock.pinned_version, 'pinned version', /^\d+\.\d+$/);
  const upstream = lock.upstream;
  if (!upstream || upstream.repository !== repository || upstream.path !== upstreamPath || upstream.ref !== upstreamRef) {
    throw new Error('Malformed OKF reference upstream authority');
  }
  requireString(upstream.commit, 'commit', /^[0-9a-f]{40}$/);
  requireString(upstream.blob_sha1, 'blob SHA-1', /^[0-9a-f]{40}$/);
  requireString(upstream.source_sha256, 'source SHA-256', /^[0-9a-f]{64}$/);
  if (!Number.isInteger(upstream.source_trailing_newlines) || upstream.source_trailing_newlines < 0) {
    throw new Error('Malformed OKF source newline identity');
  }
  if (!lock.reference || lock.reference.embedding !== embedding) throw new Error('Malformed OKF reference embedding');
  requireString(lock.reference.body_sha256, 'body SHA-256', /^[0-9a-f]{64}$/);
  const body = Buffer.from(split.body);
  if (sha256(body) !== lock.reference.body_sha256) throw new Error('Pinned OKF reference body SHA-256 mismatch');
  const marker = Buffer.from('# Open Knowledge Format (OKF)');
  const offset = body.indexOf(marker);
  if (offset < 0) throw new Error('Pinned OKF reference does not embed the upstream source');
  const source = Buffer.concat([body.subarray(offset), Buffer.from('\n'.repeat(upstream.source_trailing_newlines))]);
  if (sha256(source) !== upstream.source_sha256) throw new Error('Pinned OKF source SHA-256 mismatch');
  if (gitBlobSha(source) !== upstream.blob_sha1) throw new Error('Pinned OKF source blob SHA-1 mismatch');
  if (parseVersion(source).text !== lock.pinned_version) throw new Error('Pinned OKF source version does not match lock metadata');
  if (!lock.adoption || lock.adoption.bundle !== 'wiki/index.md' || lock.adoption.okf_version !== lock.pinned_version || lock.adoption.status !== 'adopted') {
    throw new Error('Malformed OKF reference adoption metadata');
  }
  return { lock, meta, referencePath, reference, body, source };
}

function headers(token) {
  return {
    accept: 'application/vnd.github+json',
    'x-github-api-version': '2022-11-28',
    ...(token ? { authorization: `Bearer ${token}` } : {}),
  };
}

async function request(fetchImpl, url, init, label) {
  let response;
  try {
    response = await fetchImpl(url, init);
  } catch (error) {
    const unavailable = new Error(`${label} unavailable: ${error.message}`);
    unavailable.code = 'UPSTREAM_UNAVAILABLE';
    throw unavailable;
  }
  if (!response.ok) {
    const unavailable = new Error(`${label} returned HTTP ${response.status}`);
    unavailable.code = 'UPSTREAM_UNAVAILABLE';
    throw unavailable;
  }
  return response;
}

export async function fetchLatestUpstream({ fetchImpl = globalThis.fetch, token = process.env.GITHUB_TOKEN, lock }) {
  const { repository: owner, path, ref } = lock.upstream;
  const api = `https://api.github.com/repos/${owner}`;
  const commitResponse = await request(fetchImpl, `${api}/commits?path=${encodeURIComponent(path)}&sha=${encodeURIComponent(ref)}&per_page=1`, { headers: headers(token) }, 'GitHub commit lookup');
  const commits = await commitResponse.json();
  if (!Array.isArray(commits) || commits.length !== 1 || !/^[0-9a-f]{40}$/.test(commits[0]?.sha ?? '')) throw new Error('GitHub returned ambiguous OKF revision metadata');
  const commit = commits[0].sha;
  const contentResponse = await request(fetchImpl, `${api}/contents/${path}?ref=${commit}`, { headers: headers(token) }, 'GitHub source metadata lookup');
  const metadata = await contentResponse.json();
  if (metadata.path !== path || metadata.type !== 'file' || !/^[0-9a-f]{40}$/.test(metadata.sha ?? '')) throw new Error('GitHub returned mismatched OKF source metadata');
  const rawUrl = `https://raw.githubusercontent.com/${owner}/${commit}/${path}`;
  const sourceResponse = await request(fetchImpl, rawUrl, { headers: headers(token) }, 'GitHub immutable source fetch');
  const source = Buffer.from(await sourceResponse.arrayBuffer());
  if (source.length !== metadata.size || gitBlobSha(source) !== metadata.sha) throw new Error('Immutable OKF source does not match GitHub metadata');
  return {
    commit,
    revisionDate: commits[0].commit?.committer?.date ?? commits[0].commit?.author?.date ?? null,
    blobSha1: metadata.sha,
    source,
    sourceSha256: sha256(source),
    sourceTrailingNewlines: source.toString('utf8').match(/\n*$/u)[0].length,
    version: parseVersion(source),
  };
}

export function classify(pinned, latest) {
  const pinnedVersion = parseVersion(pinned.source);
  const order = compareVersion(latest.version, pinnedVersion);
  if (order < 0) return { state: 'review-pending', change_kind: 'downgrade', adoptable: false };
  if (latest.sourceSha256 === pinned.lock.upstream.source_sha256) {
    return { state: 'current', change_kind: latest.commit === pinned.lock.upstream.commit ? 'none' : 'metadata-only', adoptable: false };
  }
  if (order === 0) return { state: 'review-pending', change_kind: 'same-version', adoptable: true };
  if (latest.version.major === pinnedVersion.major) return { state: 'review-pending', change_kind: 'minor-version', adoptable: true };
  return { state: 'review-pending', change_kind: 'major-version', adoptable: true };
}

const defaultOperations = { writeFileSync, renameSync, rmSync, openSync, fsyncSync, closeSync };

export function atomicWrite(path, content, options = {}) {
  const operations = options.operations ?? defaultOperations;
  const inject = options.inject ?? (() => {});
  const directory = dirname(path);
  const temporary = join(directory, `.${basename(path)}.${process.pid}.${temporarySequence += 1}.tmp`);
  let published = false;
  let fileDescriptor;
  let directoryDescriptor;
  try {
    operations.writeFileSync(temporary, content, { flag: 'wx' });
    fileDescriptor = operations.openSync(temporary, 'r');
    operations.fsyncSync(fileDescriptor);
    operations.closeSync(fileDescriptor);
    fileDescriptor = undefined;
    inject('after-temporary-sync');
    operations.renameSync(temporary, path);
    published = true;
    inject('after-rename');
    directoryDescriptor = operations.openSync(directory, 'r');
    operations.fsyncSync(directoryDescriptor);
    operations.closeSync(directoryDescriptor);
    directoryDescriptor = undefined;
    inject('after-directory-sync');
  } catch (error) {
    if (fileDescriptor !== undefined) operations.closeSync(fileDescriptor);
    if (directoryDescriptor !== undefined) operations.closeSync(directoryDescriptor);
    operations.rmSync(temporary, { force: true });
    error.publication = published ? 'published-coherent' : 'not-published';
    throw error;
  }
}

function adoptedWikiVersion(root) {
  const text = readFileSync(join(root, 'wiki/index.md'), 'utf8');
  const match = text.match(/^okf_version:\s*["']?([0-9]+\.[0-9]+)["']?\s*$/m);
  if (!match) throw new Error('DEOS Wiki root has no parseable okf_version adoption declaration');
  return match[1];
}

function runStrictTests(directory) {
  const result = spawnSync('npm', ['test', '--prefix', directory], { cwd: projectRoot, stdio: 'inherit' });
  if (result.status !== 0) throw new Error('Strict Wiki/OKF tests failed; adoption refused');
}

export function acceptLatest(pinned, latest, classification, options = {}) {
  if (classification.state === 'current') return pinned;
  if (!classification.adoptable) throw new Error('Upstream OKF downgrade refused');
  if (!options.reviewed) throw new Error('Reference update requires --reviewed after human/agent semantic review');
  if (classification.change_kind === 'same-version' && !options.allowSameVersionRevision) {
    throw new Error('Changed content under the same OKF version requires --allow-same-version-revision');
  }
  if (classification.change_kind === 'minor-version' && !options.allowVersionChange) {
    throw new Error('New OKF minor version adoption requires --allow-version-change');
  }
  if (classification.change_kind === 'major-version' && !options.allowBreakingVersion) {
    throw new Error('New OKF major version adoption requires --allow-breaking-version');
  }
  if (adoptedWikiVersion(options.root ?? projectRoot) !== latest.version.text) {
    throw new Error(`DEOS Wiki has not adopted OKF ${latest.version.text}; migrate schema and index.md before reference adoption`);
  }
  (options.runTests ?? runStrictTests)(skillDir);
  const referencePath = options.referencePath ?? pinned.referencePath;
  atomicWrite(referencePath, renderReference(latest.source, latest), options);
  return loadAndVerifyPinned(referencePath);
}

function report(state, pinned, latest, classification, extra = {}) {
  return {
    state,
    change_kind: classification?.change_kind ?? null,
    pinned_version: pinned.lock.pinned_version,
    upstream_version: latest?.version.text ?? null,
    pinned_commit: pinned.lock.upstream.commit,
    upstream_commit: latest?.commit ?? null,
    pinned_source_sha256: pinned.lock.upstream.source_sha256,
    upstream_source_sha256: latest?.sourceSha256 ?? null,
    ...extra,
  };
}

export async function synchronize(options = {}) {
  const mode = options.mode;
  const referencePath = options.referencePath ?? defaultReferencePath;
  const pinned = loadAndVerifyPinned(referencePath);
  let latest;
  try {
    latest = await fetchLatestUpstream({ fetchImpl: options.fetchImpl, token: options.token, lock: pinned.lock });
  } catch (error) {
    if (error.code !== 'UPSTREAM_UNAVAILABLE') throw error;
    return report('unknown', pinned, null, null, { pinned_valid: true, reason: error.message });
  }
  const classification = classify(pinned, latest);
  if (mode === 'check' || classification.state === 'current') return report(classification.state, pinned, latest, classification);
  const adopted = acceptLatest(pinned, latest, classification, { ...options, referencePath });
  return report('adopted', adopted, latest, classification, { adoption_status: adopted.lock.adoption.status });
}

function usage() {
  console.log(`Usage: okf-reference.mjs --check | --sync [adoption flags]\n\n--check   Verify the pinned reference and report upstream lifecycle without writing.\n--sync    Adopt a reviewed upstream source after strict tests and Wiki adoption checks.\n\nAdoption flags:\n  --reviewed                     Confirm semantic review was completed.\n  --allow-same-version-revision  Adopt changed semantics under the same version.\n  --allow-version-change         Adopt a newer non-breaking version.\n  --allow-breaking-version       Adopt a newer major version.\n\nLifecycle states are current, unknown, review-pending, and adopted. GITHUB_TOKEN\nis optional. Unknown freshness leaves the verified local atom usable and unchanged.`);
}

export async function main(argv = process.argv.slice(2)) {
  if (argv.includes('-h') || argv.includes('--help')) { usage(); return 0; }
  const mode = argv.includes('--sync') ? 'sync' : argv.includes('--check') ? 'check' : null;
  if (!mode || (argv.includes('--sync') && argv.includes('--check'))) throw new Error('Choose exactly one of --check or --sync');
  const result = await synchronize({
    mode,
    reviewed: argv.includes('--reviewed'),
    allowSameVersionRevision: argv.includes('--allow-same-version-revision'),
    allowVersionChange: argv.includes('--allow-version-change'),
    allowBreakingVersion: argv.includes('--allow-breaking-version'),
  });
  console.log(JSON.stringify(result));
  return 0;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  main().then((status) => { process.exitCode = status; }).catch((error) => { console.error(error.message); process.exitCode = 1; });
}
