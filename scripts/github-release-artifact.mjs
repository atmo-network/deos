#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { inflateRawSync } from 'node:zlib';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';
import { canonicalJson } from './validation-evidence.mjs';
import { validateCandidateManifest } from './release-evidence.mjs';

const MAX_ARCHIVE_BYTES = 1024 * 1024 * 1024;
const MAX_MEMBER_BYTES = 512 * 1024 * 1024;
const MAX_TOTAL_BYTES = 1024 * 1024 * 1024;
const MAX_ENTRIES = 512;
function fail(message) { throw new Error(message); }
function sha256(bytes) { return createHash('sha256').update(bytes).digest('hex'); }
function relative(value) {
  if (!value || value.includes('\\') || value.includes('\0') || path.posix.isAbsolute(value) || path.posix.normalize(value) !== value || value === '..' || value.startsWith('../') || value.split('/').includes('..')) fail(`Unsafe ZIP member path: ${value}`);
  return value;
}
function exactKeys(value, keys, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || canonicalJson(Object.keys(value).sort()) !== canonicalJson([...keys].sort())) fail(`${label} fields are invalid`);
}
function readEntry(zip, entry) {
  if (zip.readUInt32LE(entry.localOffset) !== 0x04034b50) fail(`ZIP local header is invalid: ${entry.name}`);
  const flags = zip.readUInt16LE(entry.localOffset + 6);
  const method = zip.readUInt16LE(entry.localOffset + 8);
  const nameLength = zip.readUInt16LE(entry.localOffset + 26);
  const extraLength = zip.readUInt16LE(entry.localOffset + 28);
  const nameStart = entry.localOffset + 30;
  const dataStart = nameStart + nameLength + extraLength;
  if (flags !== entry.flags || method !== entry.method || zip.subarray(nameStart, nameStart + nameLength).toString('utf8') !== entry.name || dataStart + entry.compressedSize > zip.length) fail(`ZIP local/central identity mismatch: ${entry.name}`);
  const compressed = zip.subarray(dataStart, dataStart + entry.compressedSize);
  const bytes = method === 0 ? compressed : inflateRawSync(compressed, { maxOutputLength: entry.uncompressedSize });
  if (bytes.length !== entry.uncompressedSize) fail(`ZIP member size mismatch: ${entry.name}`);
  return bytes;
}
function zipEntries(zip) {
  if (!Buffer.isBuffer(zip) || zip.length === 0 || zip.length > MAX_ARCHIVE_BYTES) fail('ZIP archive size is invalid');
  let eocd = -1;
  for (let offset = zip.length - 22; offset >= Math.max(0, zip.length - 65557); offset -= 1) if (zip.readUInt32LE(offset) === 0x06054b50) { eocd = offset; break; }
  if (eocd < 0) fail('ZIP end-of-central-directory is unavailable');
  const disk = zip.readUInt16LE(eocd + 4); const centralDisk = zip.readUInt16LE(eocd + 6);
  const diskEntries = zip.readUInt16LE(eocd + 8); const entriesCount = zip.readUInt16LE(eocd + 10);
  const centralSize = zip.readUInt32LE(eocd + 12); const centralOffset = zip.readUInt32LE(eocd + 16);
  const commentLength = zip.readUInt16LE(eocd + 20);
  if (disk !== 0 || centralDisk !== 0 || diskEntries !== entriesCount || entriesCount === 0 || entriesCount > MAX_ENTRIES || centralOffset + centralSize !== eocd || eocd + 22 + commentLength !== zip.length || [diskEntries, entriesCount].includes(0xffff) || [centralSize, centralOffset].includes(0xffffffff)) fail('ZIP topology is unsupported or unbounded');
  const entries = []; const names = new Set(); let cursor = centralOffset; let total = 0;
  for (let index = 0; index < entriesCount; index += 1) {
    if (cursor + 46 > eocd || zip.readUInt32LE(cursor) !== 0x02014b50) fail('ZIP central directory is malformed');
    const flags = zip.readUInt16LE(cursor + 8); const method = zip.readUInt16LE(cursor + 10);
    const compressedSize = zip.readUInt32LE(cursor + 20); const uncompressedSize = zip.readUInt32LE(cursor + 24);
    const nameLength = zip.readUInt16LE(cursor + 28); const extraLength = zip.readUInt16LE(cursor + 30); const entryCommentLength = zip.readUInt16LE(cursor + 32);
    const external = zip.readUInt32LE(cursor + 38); const localOffset = zip.readUInt32LE(cursor + 42);
    const end = cursor + 46 + nameLength + extraLength + entryCommentLength;
    if (end > eocd || flags & 1 || ![0, 8].includes(method) || [compressedSize, uncompressedSize, localOffset].includes(0xffffffff)) fail('ZIP member uses unsupported encryption, method, or ZIP64');
    const name = relative(zip.subarray(cursor + 46, cursor + 46 + nameLength).toString('utf8'));
    const unixMode = external >>> 16; const type = unixMode & 0o170000;
    if (name.endsWith('/') || (type !== 0 && type !== 0o100000)) fail(`ZIP member is not a regular file: ${name}`);
    if (names.has(name)) fail(`Duplicate ZIP member: ${name}`);
    if (uncompressedSize <= 0 || uncompressedSize > MAX_MEMBER_BYTES || (compressedSize === 0 && method !== 0) || (compressedSize > 0 && uncompressedSize / compressedSize > 200)) fail(`ZIP member is empty or exceeds bounds: ${name}`);
    total += uncompressedSize; if (total > MAX_TOTAL_BYTES) fail('ZIP expanded size exceeds bound');
    names.add(name); entries.push({ name, flags, method, compressedSize, uncompressedSize, localOffset }); cursor = end;
  }
  if (cursor !== eocd) fail('ZIP central directory has trailing data');
  return entries.map((entry) => ({ ...entry, bytes: readEntry(zip, entry) }));
}
export function preflightCandidateZip(zip, expectedManifestSha256 = null) {
  const entries = zipEntries(zip);
  const manifestEntry = entries.find((entry) => entry.name === 'candidate-manifest.json');
  if (!manifestEntry || manifestEntry.uncompressedSize > 16 * 1024 * 1024) fail('ZIP candidate manifest is missing or oversized');
  const manifestBytes = manifestEntry.bytes;
  if (expectedManifestSha256 && `sha256:${sha256(manifestBytes)}` !== expectedManifestSha256) fail('ZIP candidate manifest digest mismatch');
  const manifest = validateCandidateManifest(JSON.parse(manifestBytes));
  if (!manifestBytes.equals(Buffer.from(`${canonicalJson(manifest)}\n`))) fail('ZIP candidate manifest is not canonical JSON');
  const names = new Set(entries.map((entry) => entry.name)); const expected = new Set(['candidate-manifest.json', ...manifest.members.map((entry) => `files/${entry.path}`)]);
  if (expected.size !== names.size || [...expected].some((name) => !names.has(name))) fail('ZIP contains missing or extra candidate entries');
  return { manifest, manifestSha256: `sha256:${sha256(manifestBytes)}`, entries: entries.map(({ name, compressedSize, uncompressedSize }) => ({ name, compressedSize, uncompressedSize })) };
}
export function preflightSingleFileZip(zip, expectedName, expectedSha256) {
  relative(expectedName); if (!/^sha256:[0-9a-f]{64}$/.test(expectedSha256 ?? '')) fail('Expected file digest is missing or invalid');
  const entries = zipEntries(zip); if (entries.length !== 1 || entries[0].name !== expectedName) fail('ZIP single-file inventory is missing or extra');
  if (`sha256:${sha256(entries[0].bytes)}` !== expectedSha256) fail('ZIP file digest mismatch'); return entries[0].bytes;
}
async function api(url, token) {
  const response = await fetch(url, { headers: { Authorization: `Bearer ${token}`, Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28' }, signal: AbortSignal.timeout(30_000) });
  if (!response.ok) fail(`GitHub API ${url} failed with HTTP ${response.status}`);
  return response.json();
}
export async function verifyGithubArtifactProvenance(options, request = api) {
  if (!/^(0|[1-9][0-9]*)$/.test(options.expectedArtifactId ?? '')) fail('Expected producer artifact ID is missing or invalid');
  if (!/^sha256:[0-9a-f]{64}$/.test(options.expectedArtifactDigest ?? '')) fail('Expected producer artifact digest is missing or invalid');
  const payloadDigest = options.expectedManifestSha256 ?? options.expectedFileSha256;
  if (!/^sha256:[0-9a-f]{64}$/.test(payloadDigest ?? '')) fail(options.expectedFileSha256 !== undefined ? 'Expected producer file digest is missing or invalid' : 'Expected producer manifest digest is missing or invalid');
  const base = `https://api.github.com/repos/${options.repository}`;
  const repository = await request(`https://api.github.com/repos/${options.repository}`, options.token);
  if (String(repository.id) !== options.repositoryId || repository.full_name !== options.repository) fail('GitHub repository identity mismatch');
  const run = await request(`${base}/actions/runs/${options.runId}/attempts/${options.runAttempt}`, options.token);
  if (String(run.id) !== options.runId || String(run.run_attempt) !== options.runAttempt || run.repository?.id !== repository.id || run.event !== 'push' || run.head_sha !== options.headSha || run.head_branch !== options.tagName || run.path !== '.github/workflows/release-candidate.yml') fail('GitHub workflow run/attempt/tag provenance mismatch');
  const jobs = await request(`${base}/actions/runs/${options.runId}/attempts/${options.runAttempt}/jobs?per_page=100`, options.token);
  const matchingJobs = jobs.jobs?.filter((job) => job.name === options.jobName && job.head_sha === options.headSha && job.status === 'completed' && job.conclusion === 'success') ?? [];
  if (matchingJobs.length !== 1) fail('Exact successful producer job is unavailable or ambiguous');
  const uploadStepName = options.uploadStepName ?? 'Upload immutable candidate handoff';
  const uploadSteps = matchingJobs[0].steps?.filter((step) => step.name === uploadStepName && step.status === 'completed' && step.conclusion === 'success') ?? [];
  if (uploadSteps.length !== 1) fail('Exact successful candidate upload step is unavailable or ambiguous');
  const listed = await request(`${base}/actions/runs/${options.runId}/artifacts?per_page=100`, options.token);
  const matches = listed.artifacts?.filter((artifact) => artifact.name === options.artifactName && String(artifact.id) === options.expectedArtifactId) ?? [];
  if (matches.length !== 1) fail('Exact attempt-named producer artifact ID is unavailable or ambiguous');
  const artifact = await request(`${base}/actions/artifacts/${options.expectedArtifactId}`, options.token);
  if (String(artifact.id) !== options.expectedArtifactId || artifact.id !== matches[0].id || artifact.name !== options.artifactName || artifact.expired !== false || artifact.workflow_run?.id !== Number(options.runId) || artifact.workflow_run?.repository_id !== repository.id || artifact.workflow_run?.head_sha !== options.headSha || artifact.digest !== options.expectedArtifactDigest || matches[0].digest !== options.expectedArtifactDigest) fail('GitHub artifact ID/name/expiry/digest differs from producer outputs or run metadata');
  return { artifactId: String(artifact.id), artifactName: artifact.name, artifactDigest: artifact.digest, jobId: String(matchingJobs[0].id), archiveUrl: `${base}/actions/artifacts/${artifact.id}/zip` };
}
function take(args, flag) { const index = args.indexOf(flag); if (index < 0 || !args[index + 1]) fail(`${flag} is required`); const value = args[index + 1]; args.splice(index, 2); return value; }
async function main(args) {
  if (args.includes('--help') || args.length === 0) { console.log('Usage: github-release-artifact.mjs download|download-file [exact producer options]'); return; }
  const command = args.shift(); if (!['download', 'download-file'].includes(command)) fail('Only download and download-file are supported');
  const options = { repository: take(args, '--repository'), repositoryId: take(args, '--repository-id'), runId: take(args, '--run-id'), runAttempt: take(args, '--run-attempt'), jobName: take(args, '--job-name'), tagName: take(args, '--tag-name'), headSha: take(args, '--head-sha'), artifactName: take(args, '--artifact-name'), expectedArtifactId: take(args, '--expected-artifact-id'), expectedArtifactDigest: take(args, '--expected-artifact-digest'), token: process.env.GH_TOKEN };
  if (command === 'download') options.expectedManifestSha256 = take(args, '--expected-manifest-sha256');
  else { options.expectedFileSha256 = take(args, '--expected-file-sha256'); options.uploadStepName = take(args, '--upload-step-name'); }
  const archive = path.resolve(take(args, '--archive')); const output = path.resolve(take(args, '--output')); const githubOutput = path.resolve(take(args, '--github-output')); if (command === 'download-file') options.expectedFileName = take(args, '--expected-file-name');
  if (args.length || !options.token) fail('Unknown arguments or GH_TOKEN is unavailable');
  const provenance = await verifyGithubArtifactProvenance(options);
  const response = await fetch(provenance.archiveUrl, { headers: { Authorization: `Bearer ${options.token}`, 'X-GitHub-Api-Version': '2022-11-28' }, redirect: 'follow', signal: AbortSignal.timeout(120_000) });
  if (!response.ok) fail(`Exact artifact archive download failed with HTTP ${response.status}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  const archiveDigest = `sha256:${sha256(bytes)}`;
  if (archiveDigest !== provenance.artifactDigest) fail('Raw archive SHA-256 differs from GitHub artifact digest');
  const inventory = command === 'download' ? preflightCandidateZip(bytes, options.expectedManifestSha256) : null;
  const fileBytes = command === 'download-file' ? preflightSingleFileZip(bytes, options.expectedFileName, options.expectedFileSha256) : null;
  await writeFile(archive, bytes, { flag: 'wx', mode: 0o600 });
  if (command === 'download') {
    await mkdir(output, { recursive: true, mode: 0o700 });
    const unzip = spawnSync('unzip', ['-q', archive, '-d', output], { encoding: 'utf8' });
    if (unzip.error || unzip.status !== 0) fail(`Preflighted ZIP extraction failed: ${String(unzip.stderr || unzip.stdout).trim()}`);
  } else { await mkdir(path.dirname(output), { recursive: true, mode: 0o700 }); await writeFile(output, fileBytes, { flag: 'wx', mode: 0o600 }); }
  const outputs = command === 'download' ? { ...provenance, archiveSha256: archiveDigest, manifestSha256: inventory.manifestSha256 } : { ...provenance, archiveSha256: archiveDigest, fileSha256: options.expectedFileSha256 };
  exactKeys(outputs, ['artifactId', 'artifactName', 'artifactDigest', 'jobId', 'archiveUrl', 'archiveSha256', command === 'download' ? 'manifestSha256' : 'fileSha256'], 'Download outputs');
  await writeFile(githubOutput, Object.entries(outputs).map(([key, value]) => `${key.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}=${value}`).join('\n') + '\n', { flag: 'a' });
}
if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(new URL(import.meta.url).pathname)) main(process.argv.slice(2)).catch((error) => { console.error(`github-release-artifact: ${error.message}`); process.exitCode = 1; });
