#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { constants as fsConstants, readFileSync } from 'node:fs';
import { lstat, mkdir, mkdtemp, open, readFile, readdir, rm, writeFile, copyFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { gzipSync } from 'node:zlib';
import path from 'node:path';
import process from 'node:process';
import { canonicalJson, calculateIdentity, gitCommonDirectory, readAuthorityManifestFromTree, readValidRecord, validateRecord } from './validation-evidence.mjs';
import { validateToolLock } from './install-release-tools.mjs';
import { validateSpdxSchema } from './release-tooling/validate-spdx.mjs';

const CANDIDATE_SCHEMA = 'deos-release-candidate/v1';
const SUMMARY_SCHEMA = 'deos-release-network-summary/v1';
const CANDIDATE_MANIFEST = 'candidate-manifest.json';
const FILES_ROOT = 'files';
const WASM_PATH = 'template/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm';
const GENERATED_MEMBERS = Object.freeze(['validation/full-evidence.json', 'validation/validation-summary.json']);
const PROOF_SCHEMA = 'deos-release-network-proof/v1';
const RELEASE_MANIFEST_SCHEMA = 'deos-release-manifest/v2';
const LOCK_PATHS = Object.freeze(['template/Cargo.lock', 'web-client/package-lock.json', '.agents/skills/wiki-sync/package-lock.json', 'scripts/release-tooling/package-lock.json']);
const SBOM_CREATOR = 'Tool: deos-release-evidence-v1';
export const NETWORK_PROOF_ORDER = Object.freeze([
  'finalizedRelayAndTwoCollators', 'charlieAuthored', 'daveAuthored',
  'charliePauseDaveFinality', 'signedPreRestartTransfer', 'persistedDaveRestart',
  'signedPostRestartTransfer', 'routerOracleBurnActor',
]);

function fail(message) { throw new Error(message); }
function sha256(bytes) { return createHash('sha256').update(bytes).digest('hex'); }
function sha(value) { return `sha256:${sha256(value)}`; }
function exactKeys(value, keys, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || canonicalJson(Object.keys(value).sort()) !== canonicalJson([...keys].sort())) fail(`${label} fields are invalid`);
}
function identifier(value, label, pattern = /^[A-Za-z0-9._:/-]+$/) {
  if (typeof value !== 'string' || !pattern.test(value)) fail(`${label} is invalid`);
  return value;
}
function unsigned(value, label) {
  identifier(value, label, /^(0|[1-9][0-9]*)$/);
  if (!Number.isSafeInteger(Number(value))) fail(`${label} exceeds the safe integer range`);
  return value;
}
function oid(value, label) { return identifier(value, label, /^[0-9a-f]{40}([0-9a-f]{24})?$/); }
function digest(value, label) { return identifier(value, label, /^sha256:[0-9a-f]{64}$/); }
function relative(value, label) {
  if (typeof value !== 'string' || value.includes('\\') || value.includes('\0') || path.posix.isAbsolute(value)) fail(`${label} is not a relative POSIX path`);
  const normalized = path.posix.normalize(value);
  if (!value || value === '.' || normalized !== value || value === '..' || value.startsWith('../') || value.split('/').includes('..')) fail(`${label} contains traversal or is not normalized: ${value}`);
  return value;
}
function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  if (result.error) throw result.error;
  if (result.status !== 0) fail(`${command} ${args.join(' ')} failed: ${String(result.stderr || result.stdout).trim()}`);
  return result.stdout.trim();
}
function git(repo, ...args) { return run('git', ['-C', repo, ...args], repo); }
function workspaceVersion(repo) {
  const source = requireTextSync(path.join(repo, 'template/Cargo.toml'));
  const match = /^\[workspace\.package\]\s*$[\s\S]*?^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"\s*$/m.exec(source);
  if (!match) fail('Workspace version is unavailable');
  return match[1];
}
function requireTextSync(file) {
  const value = readFileSync(file, 'utf8');
  if (!value) fail(`Required text is unavailable: ${file}`);
  return value;
}
export function resolveTagIdentity(repo, tagRef, expectedVersion = workspaceVersion(repo)) {
  if (tagRef !== `refs/tags/v${expectedVersion}`) fail(`Tag ref must exactly equal refs/tags/v${expectedVersion}`);
  const tagOid = oid(git(repo, 'rev-parse', '--verify', tagRef), 'Tag OID');
  const commitOid = oid(git(repo, 'rev-parse', '--verify', `${tagRef}^{commit}`), 'Commit OID');
  const treeOid = oid(git(repo, 'rev-parse', '--verify', `${tagRef}^{tree}`), 'Tree OID');
  if (git(repo, 'rev-parse', 'HEAD') !== commitOid) fail('Checkout HEAD does not equal the tagged commit');
  if (git(repo, 'rev-parse', 'HEAD^{tree}') !== treeOid) fail('Checkout tree does not equal the tagged tree');
  return { version: expectedVersion, ref: tagRef, oid: tagOid, commitOid, treeOid };
}
async function regularNonempty(file, label) {
  const info = await lstat(file);
  if (!info.isFile() || info.isSymbolicLink() || info.size <= 0) fail(`${label} must be a nonempty regular file`);
  return info;
}
function acceptanceInventory(authority, artifactEntries) {
  if (!authority || !Array.isArray(authority.fullArtifactOutputs) || authority.fullArtifactOutputs.length !== 8) fail('Candidate authority must define all eight release artifact families');
  const entries = artifactEntries.map(validateMember);
  const paths = entries.map((entry) => entry.path);
  if (new Set(paths).size !== paths.length) fail('Full record artifact paths must be unique');
  const claimed = new Set();
  for (const output of authority.fullArtifactOutputs) {
    relative(output.path, 'Full artifact output path');
    const family = output.kind === 'file' ? entries.filter((entry) => entry.path === output.path) : entries.filter((entry) => entry.path.startsWith(`${output.path}/`));
    if (output.kind === 'file' && family.length !== 1) fail(`Full record must contain exact file family: ${output.path}`);
    if (output.kind === 'directory' && family.length === 0) fail(`Full record directory family is empty: ${output.path}`);
    if (!['file', 'directory'].includes(output.kind)) fail(`Full artifact output kind is invalid: ${output.path}`);
    for (const member of output.requiredMembers) if (!family.some((entry) => entry.path === path.posix.join(output.path, member))) fail(`Full record required member is missing: ${output.path}/${member}`);
    for (const entry of family) {
      if (claimed.has(entry.path)) fail(`Full record artifact belongs to overlapping families: ${entry.path}`);
      claimed.add(entry.path);
    }
  }
  if (claimed.size !== entries.length) fail('Full record contains an artifact outside candidate-tree authority');
  return entries.sort((a, b) => Buffer.compare(Buffer.from(a.path), Buffer.from(b.path)));
}
async function verifyRecordedSources(repo, entries) {
  for (const entry of entries) {
    const file = path.join(repo, entry.path);
    const info = await regularNonempty(file, entry.path);
    const bytes = await readFile(file);
    if (info.size !== entry.bytes || sha(bytes) !== entry.sha256) fail(`Workspace artifact differs from successful full record: ${entry.path}`);
  }
}
export { acceptanceInventory };
function validateMember(entry) {
  exactKeys(entry, ['path', 'bytes', 'sha256'], 'Candidate member');
  relative(entry.path, 'Candidate member path');
  if (!Number.isSafeInteger(entry.bytes) || entry.bytes <= 0) fail(`Candidate member size is invalid: ${entry.path}`);
  digest(entry.sha256, `Candidate member digest ${entry.path}`);
  return entry;
}
export function validateCandidateManifest(manifest) {
  exactKeys(manifest, ['schema', 'repositoryId', 'tag', 'workflow', 'validation', 'members'], 'Candidate manifest');
  if (manifest.schema !== CANDIDATE_SCHEMA) fail('Candidate schema is unsupported');
  unsigned(manifest.repositoryId, 'Repository ID');
  exactKeys(manifest.tag, ['version', 'ref', 'oid', 'commitOid', 'treeOid'], 'Candidate tag');
  identifier(manifest.tag.version, 'Version', /^[0-9]+\.[0-9]+\.[0-9]+$/);
  if (manifest.tag.ref !== `refs/tags/v${manifest.tag.version}`) fail('Candidate tag ref/version mismatch');
  oid(manifest.tag.oid, 'Tag OID'); oid(manifest.tag.commitOid, 'Commit OID'); oid(manifest.tag.treeOid, 'Tree OID');
  exactKeys(manifest.workflow, ['runId', 'runAttempt'], 'Candidate workflow');
  unsigned(manifest.workflow.runId, 'Run ID'); unsigned(manifest.workflow.runAttempt, 'Run attempt');
  exactKeys(manifest.validation, ['key', 'recordSha256', 'summarySha256'], 'Candidate validation');
  digest(manifest.validation.key, 'Validation key'); digest(manifest.validation.recordSha256, 'Validation record digest'); digest(manifest.validation.summarySha256, 'Validation summary digest');
  if (!Array.isArray(manifest.members) || manifest.members.length === 0) fail('Candidate members are empty');
  const paths = manifest.members.map(validateMember).map((entry) => entry.path);
  const sorted = [...paths].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  if (new Set(paths).size !== paths.length || canonicalJson(paths) !== canonicalJson(sorted)) fail('Candidate member paths must be unique and bytewise sorted');
  for (const required of GENERATED_MEMBERS) if (!paths.includes(required)) fail(`Candidate member is missing: ${required}`);
  return manifest;
}
async function writeCanonical(file, value) { await writeFile(file, `${canonicalJson(value)}\n`, { flag: 'wx', mode: 0o600 }); }
export async function createCandidate(options) {
  const repo = path.resolve(options.repo);
  const output = path.resolve(options.output);
  const tag = resolveTagIdentity(repo, options.tagRef);
  const identity = await calculateIdentity(repo, 'full');
  if (identity.candidate.treeOid !== tag.treeOid || !identity.candidate.clean) fail('Full validation identity is not the clean tagged tree');
  const common = await gitCommonDirectory(repo);
  const recordPath = path.join(common, 'deos-validation/v2/records', `${identity.key.slice(7)}.json`);
  const record = await readValidRecord(recordPath, identity.key);
  if (!record) fail('Fresh successful full validation record is unavailable for the candidate');
  validateRecord(record, identity.key);
  if (options.requireFresh === true && record.completedAt < options.startedAt) fail('Full validation record predates this candidate job');
  await mkdir(path.join(output, FILES_ROOT, 'validation'), { recursive: true, mode: 0o700 });
  const recordBytes = Buffer.from(`${canonicalJson(record)}\n`);
  const summary = { schema: 'deos-validation-summary/v1', conclusion: 'success', key: identity.key, treeOid: tag.treeOid, startedAt: record.startedAt, completedAt: record.completedAt, artifactCount: record.artifacts.entries.length };
  const summaryBytes = Buffer.from(`${canonicalJson(summary)}\n`);
  const authority = readAuthorityManifestFromTree(repo, tag.treeOid);
  const recordedEntries = acceptanceInventory(authority, record.artifacts.entries);
  await verifyRecordedSources(repo, recordedEntries);
  const source = recordedEntries.map((entry) => entry.path);
  for (const member of source) {
    const destination = path.join(output, FILES_ROOT, member);
    await mkdir(path.dirname(destination), { recursive: true, mode: 0o700 });
    await copyFile(path.join(repo, member), destination, 0);
  }
  await writeFile(path.join(output, FILES_ROOT, GENERATED_MEMBERS[0]), recordBytes, { flag: 'wx', mode: 0o600 });
  await writeFile(path.join(output, FILES_ROOT, GENERATED_MEMBERS[1]), summaryBytes, { flag: 'wx', mode: 0o600 });
  const all = [...source, ...GENERATED_MEMBERS].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  const members = [];
  for (const member of all) {
    const bytes = await readFile(path.join(output, FILES_ROOT, member));
    if (bytes.length === 0) fail(`Candidate member is empty: ${member}`);
    members.push({ path: member, bytes: bytes.length, sha256: sha(bytes) });
  }
  const copiedByPath = new Map(members.map((entry) => [entry.path, entry]));
  for (const recorded of recordedEntries) {
    const copied = copiedByPath.get(recorded.path);
    if (!copied || copied.bytes !== recorded.bytes || copied.sha256 !== recorded.sha256) fail(`Copied candidate differs from successful full record: ${recorded.path}`);
  }
  const manifest = validateCandidateManifest({
    schema: CANDIDATE_SCHEMA,
    repositoryId: unsigned(options.repositoryId, 'Repository ID'),
    tag,
    workflow: { runId: unsigned(options.runId, 'Run ID'), runAttempt: unsigned(options.runAttempt, 'Run attempt') },
    validation: { key: identity.key, recordSha256: sha(recordBytes), summarySha256: sha(summaryBytes) },
    members,
  });
  await writeCanonical(path.join(output, CANDIDATE_MANIFEST), manifest);
  return { manifest, manifestSha256: sha(Buffer.from(`${canonicalJson(manifest)}\n`)) };
}
async function enumerate(root) {
  const files = []; const directories = [];
  async function walk(relative) {
    const entries = await readdir(path.join(root, relative), { withFileTypes: true });
    for (const entry of entries) {
      const child = relative ? path.posix.join(relative, entry.name) : entry.name;
      if (entry.isSymbolicLink()) fail(`Symlink is forbidden: ${child}`);
      if (entry.isDirectory()) { directories.push(child); await walk(child); }
      else if (entry.isFile()) files.push(child);
      else fail(`Unsupported artifact member: ${child}`);
    }
  }
  await walk('');
  const sort = (values) => values.sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  return { files: sort(files), directories: sort(directories) };
}
export async function verifyCandidate(options) {
  const input = path.resolve(options.input);
  await regularNonempty(path.join(input, CANDIDATE_MANIFEST), CANDIDATE_MANIFEST);
  const manifestBytes = await readFile(path.join(input, CANDIDATE_MANIFEST));
  const manifest = validateCandidateManifest(JSON.parse(manifestBytes));
  if (!manifestBytes.equals(Buffer.from(`${canonicalJson(manifest)}\n`))) fail('Candidate manifest is not canonical JSON');
  if (options.repositoryId && manifest.repositoryId !== options.repositoryId) fail('Candidate repository ID mismatch');
  if (options.tagRef && manifest.tag.ref !== options.tagRef) fail('Candidate tag ref mismatch');
  if (options.runId && manifest.workflow.runId !== options.runId) fail('Candidate run ID mismatch');
  if (options.runAttempt && manifest.workflow.runAttempt !== options.runAttempt) fail('Candidate run attempt mismatch');
  if (options.manifestSha256 && sha(manifestBytes) !== options.manifestSha256) fail('Candidate manifest digest mismatch');
  if (!options.repo) fail('Candidate-tree repository is required for acceptance inventory verification');
  const repo = path.resolve(options.repo);
  const checkoutIdentity = resolveTagIdentity(repo, manifest.tag.ref, manifest.tag.version);
  if (canonicalJson(checkoutIdentity) !== canonicalJson(manifest.tag)) fail('Candidate checkout/tag identity mismatch');
  const expected = [CANDIDATE_MANIFEST, ...manifest.members.map((entry) => `${FILES_ROOT}/${entry.path}`)].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  const inventory = await enumerate(input);
  if (canonicalJson(inventory.files) !== canonicalJson(expected)) fail('Candidate handoff contains missing or extra members');
  const expectedDirectories = [...new Set(expected.flatMap((member) => {
    const parents = []; let current = path.posix.dirname(member);
    while (current !== '.') { parents.push(current); current = path.posix.dirname(current); }
    return parents;
  }))].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  if (canonicalJson(inventory.directories) !== canonicalJson(expectedDirectories)) fail('Candidate handoff contains an extra or missing directory');
  for (const entry of manifest.members) {
    const file = path.join(input, FILES_ROOT, entry.path);
    const info = await regularNonempty(file, entry.path);
    const bytes = await readFile(file);
    if (info.size !== entry.bytes || sha(bytes) !== entry.sha256) fail(`Candidate member identity mismatch: ${entry.path}`);
  }
  const recordBytes = await readFile(path.join(input, FILES_ROOT, GENERATED_MEMBERS[0]));
  const record = validateRecord(JSON.parse(recordBytes), manifest.validation.key);
  if (!recordBytes.equals(Buffer.from(`${canonicalJson(record)}\n`)) || sha(recordBytes) !== manifest.validation.recordSha256 || record.candidate.treeOid !== manifest.tag.treeOid) fail('Candidate validation record identity mismatch');
  const authority = readAuthorityManifestFromTree(repo, manifest.tag.treeOid);
  const recordedEntries = acceptanceInventory(authority, record.artifacts.entries);
  const expectedMembers = [...recordedEntries.map((entry) => entry.path), ...GENERATED_MEMBERS].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  if (canonicalJson(manifest.members.map((entry) => entry.path)) !== canonicalJson(expectedMembers)) fail('Candidate manifest does not exactly match the full record acceptance inventory');
  const memberByPath = new Map(manifest.members.map((entry) => [entry.path, entry]));
  for (const recorded of recordedEntries) {
    const candidate = memberByPath.get(recorded.path);
    if (!candidate || candidate.bytes !== recorded.bytes || candidate.sha256 !== recorded.sha256) fail(`Candidate member differs from successful full record: ${recorded.path}`);
  }
  const summaryBytes = await readFile(path.join(input, FILES_ROOT, GENERATED_MEMBERS[1]));
  const summary = JSON.parse(summaryBytes);
  exactKeys(summary, ['schema', 'conclusion', 'key', 'treeOid', 'startedAt', 'completedAt', 'artifactCount'], 'Validation summary');
  if (!summaryBytes.equals(Buffer.from(`${canonicalJson(summary)}\n`)) || sha(summaryBytes) !== manifest.validation.summarySha256 || summary.schema !== 'deos-validation-summary/v1' || summary.conclusion !== 'success' || summary.key !== manifest.validation.key || summary.treeOid !== manifest.tag.treeOid || summary.startedAt !== record.startedAt || summary.completedAt !== record.completedAt || summary.artifactCount !== recordedEntries.length) fail('Candidate validation summary identity mismatch');
  if (options.materialize) {
    const destinationRoot = path.resolve(options.materialize);
    for (const entry of manifest.members.filter((item) => !item.path.startsWith('validation/'))) {
      const destination = path.join(destinationRoot, entry.path);
      await mkdir(path.dirname(destination), { recursive: true });
      await copyFile(path.join(input, FILES_ROOT, entry.path), destination);
    }
  }
  return { manifest, manifestSha256: sha(manifestBytes) };
}
function findGenesisCode(spec) {
  const candidates = [spec?.genesis?.raw?.top?.['0x3a636f6465'], spec?.genesis?.runtimeGenesis?.code, spec?.genesis?.runtime?.system?.code];
  const values = candidates.filter((value) => typeof value === 'string');
  if (values.length !== 1 || !/^0x([0-9a-fA-F]{2})+$/.test(values[0])) fail('Chain spec must contain exactly one recognized nonempty genesis :code');
  return Buffer.from(values[0].slice(2), 'hex');
}
export async function verifyChainCode(wasmPath, specPath) {
  const wasm = await readFile(wasmPath);
  if (wasm.length === 0) fail('Candidate Wasm is empty');
  const code = findGenesisCode(JSON.parse(await readFile(specPath, 'utf8')));
  if (!code.equals(wasm)) fail('Genesis :code bytes do not exactly equal candidate Wasm');
  return { wasmSha256: sha(wasm), chainSpecSha256: sha(await readFile(specPath)) };
}
export function validateProofLedger(contents) {
  const lines = contents.split('\n').filter(Boolean);
  const records = lines.map((line, index) => {
    let record;
    try { record = JSON.parse(line); } catch { fail(`Network proof line ${index + 1} is not JSON`); }
    exactKeys(record, ['schema', 'sequence', 'id', 'completedAt'], 'Network proof record');
    if (record.schema !== PROOF_SCHEMA || record.sequence !== index + 1 || record.id !== NETWORK_PROOF_ORDER[index] || !Number.isFinite(Date.parse(record.completedAt))) fail(`Network proof line ${index + 1} is invalid or out of order`);
    return record;
  });
  if (records.length > NETWORK_PROOF_ORDER.length) fail('Network proof ledger contains extra records');
  return records;
}
export async function appendNetworkProof(ledger, id) {
  let contents = '';
  try { contents = await readFile(ledger, 'utf8'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const records = validateProofLedger(contents);
  if (id !== NETWORK_PROOF_ORDER[records.length]) fail(`Expected next network proof ${NETWORK_PROOF_ORDER[records.length] ?? '(none)'}, received ${id}`);
  const record = { schema: PROOF_SCHEMA, sequence: records.length + 1, id, completedAt: new Date().toISOString() };
  const handle = await open(ledger, fsConstants.O_WRONLY | fsConstants.O_APPEND | fsConstants.O_CREAT | fsConstants.O_NOFOLLOW, 0o600);
  try { const info = await handle.stat(); if (!info.isFile()) fail('Network proof ledger is not a regular file'); await handle.writeFile(`${canonicalJson(record)}\n`); await handle.sync(); } finally { await handle.close(); }
  return record;
}
export async function writeNetworkSummary(options) {
  const verified = await verifyCandidate(options);
  const code = await verifyChainCode(options.wasm, options.chainSpec);
  const toolLockBytes = await readFile(options.toolLock);
  const proofBytes = await readFile(options.proofLedger);
  const proofs = validateProofLedger(proofBytes.toString('utf8'));
  if (proofs.length !== NETWORK_PROOF_ORDER.length) fail('Network proof ledger is incomplete');
  const summary = {
    schema: SUMMARY_SCHEMA,
    conclusion: 'success',
    repositoryId: verified.manifest.repositoryId,
    tag: verified.manifest.tag,
    workflow: verified.manifest.workflow,
    candidateManifestSha256: verified.manifestSha256,
    wasmSha256: code.wasmSha256,
    chainSpecSha256: code.chainSpecSha256,
    toolLockSha256: sha(toolLockBytes),
    proofLedgerSha256: sha(proofBytes),
    proofs,
  };
  await writeCanonical(options.output, summary);
  return summary;
}
export function validateNetworkSummary(summary, candidate, candidateManifestSha256, toolLockSha256 = null) {
  exactKeys(summary, ['schema', 'conclusion', 'repositoryId', 'tag', 'workflow', 'candidateManifestSha256', 'wasmSha256', 'chainSpecSha256', 'toolLockSha256', 'proofLedgerSha256', 'proofs'], 'Network summary');
  if (summary.schema !== SUMMARY_SCHEMA || summary.conclusion !== 'success') fail('Network summary is not successful');
  if (summary.repositoryId !== candidate.repositoryId || canonicalJson(summary.tag) !== canonicalJson(candidate.tag) || canonicalJson(summary.workflow) !== canonicalJson(candidate.workflow) || summary.candidateManifestSha256 !== candidateManifestSha256) fail('Candidate/network identity mismatch');
  for (const field of ['wasmSha256', 'chainSpecSha256', 'toolLockSha256', 'proofLedgerSha256']) digest(summary[field], `Network summary ${field}`);
  if (toolLockSha256 && summary.toolLockSha256 !== toolLockSha256) fail('Network summary tool lock mismatch');
  if (!Array.isArray(summary.proofs) || summary.proofs.length !== NETWORK_PROOF_ORDER.length || canonicalJson(summary.proofs) !== canonicalJson(validateProofLedger(summary.proofs.map((entry) => canonicalJson(entry)).join('\n') + '\n'))) fail('Network summary proof inventory is invalid');
  return summary;
}
function writeTarOctal(buffer, offset, length, value) { const text = value.toString(8).padStart(length - 1, '0'); buffer.write(text, offset, length - 1, 'ascii'); buffer[offset + length - 1] = 0; }
function tarHeader(name, size, mtime) {
  if (Buffer.byteLength(name) > 100) fail(`Descriptor archive path is too long: ${name}`);
  const header = Buffer.alloc(512); header.write(name, 0, 100, 'utf8'); writeTarOctal(header, 100, 8, 0o644); writeTarOctal(header, 108, 8, 0); writeTarOctal(header, 116, 8, 0); writeTarOctal(header, 124, 12, size); writeTarOctal(header, 136, 12, mtime); header.fill(0x20, 148, 156); header[156] = 0x30; header.write('ustar\0', 257, 6, 'ascii'); header.write('00', 263, 2, 'ascii'); header.write('root', 265, 4, 'ascii'); header.write('root', 297, 4, 'ascii'); let sum = 0; for (const byte of header) sum += byte; const checksum = sum.toString(8).padStart(6, '0'); header.write(checksum, 148, 6, 'ascii'); header[154] = 0; header[155] = 0x20; return header;
}
export async function deterministicDescriptorArchive(root, commitTime) {
  const inventory = await enumerate(root);
  if (inventory.files.length === 0) fail('Descriptor directory is empty');
  const chunks = [];
  for (const member of inventory.files) {
    const file = path.join(root, member); const info = await regularNonempty(file, `Descriptor ${member}`); const bytes = await readFile(file);
    chunks.push(tarHeader(member, info.size, commitTime), bytes, Buffer.alloc((512 - (bytes.length % 512)) % 512));
  }
  chunks.push(Buffer.alloc(1024));
  return gzipSync(Buffer.concat(chunks), { level: 9, mtime: 0 });
}
function exactField(entry, name) {
  if (!Object.hasOwn(entry, name)) return { state: 'missing' };
  if (entry[name] === null) return { state: 'null' };
  return { state: 'value', value: entry[name] };
}
function fieldValue(field) { return field?.state === 'value' ? field.value : undefined; }
function npmName(location, entry) {
  const declared = fieldValue(exactField(entry, 'name'));
  if (typeof declared === 'string' && declared) return declared;
  const marker = location.lastIndexOf('node_modules/');
  return marker < 0 ? null : location.slice(marker + 'node_modules/'.length);
}
export function lockInventory(lockBytes) {
  const inventory = [];
  const cargo = lockBytes.get('template/Cargo.lock')?.toString('utf8');
  if (!cargo) fail('Cargo.lock is unavailable');
  const cargoBlocks = cargo.split(/^\[\[package\]\]\s*$/m).slice(1);
  for (const [index, block] of cargoBlocks.entries()) {
    const name = /^name = "([^"]+)"$/m.exec(block)?.[1]; const version = /^version = "([^"]+)"$/m.exec(block)?.[1];
    if (!name || !version) fail(`Cargo.lock package is malformed at package[${index}]`);
    inventory.push({ ecosystem: 'cargo', owner: 'template/Cargo.lock', location: `package[${index}]`, name, version, source: exactField({ source: /^source = "([^"]+)"$/m.exec(block)?.[1] }, 'source'), checksum: exactField({ checksum: /^checksum = "([^"]+)"$/m.exec(block)?.[1] }, 'checksum') });
    if (fieldValue(inventory.at(-1).source) === undefined) inventory.at(-1).source = { state: 'missing' };
    if (fieldValue(inventory.at(-1).checksum) === undefined) inventory.at(-1).checksum = { state: 'missing' };
  }
  for (const owner of LOCK_PATHS.slice(1)) {
    let parsed; try { parsed = JSON.parse(lockBytes.get(owner)); } catch { fail(`${owner} is malformed`); }
    if (parsed.lockfileVersion !== 3 || !parsed.packages || typeof parsed.packages !== 'object' || Array.isArray(parsed.packages)) fail(`${owner} package inventory is invalid`);
    for (const [location, entry] of Object.entries(parsed.packages)) {
      if (!entry || typeof entry !== 'object' || Array.isArray(entry)) fail(`${owner} package entry is malformed: ${location}`);
      const name = npmName(location, entry); const version = exactField(entry, 'version');
      if (!name || (fieldValue(version) !== undefined && typeof fieldValue(version) !== 'string')) fail(`${owner} package entry is malformed: ${location}`);
      inventory.push({ ecosystem: 'npm', owner, location, name, nameField: exactField(entry, 'name'), version, resolved: exactField(entry, 'resolved'), integrity: exactField(entry, 'integrity'), dev: exactField(entry, 'dev'), optional: exactField(entry, 'optional'), devOptional: exactField(entry, 'devOptional'), peer: exactField(entry, 'peer'), inBundle: exactField(entry, 'inBundle'), link: exactField(entry, 'link') });
    }
  }
  const locations = new Set();
  for (const entry of inventory) { const key = `${entry.owner}\0${entry.location}`; if (locations.has(key)) fail(`Duplicate lock package location: ${entry.owner}:${entry.location}`); locations.add(key); }
  return inventory.sort((a, b) => Buffer.compare(Buffer.from(`${a.owner}\0${a.location}`), Buffer.from(`${b.owner}\0${b.location}`)));
}
function inventoryRow(entry) { return canonicalJson(entry); }
function lockPackageId(entry) { return `SPDXRef-Package-lock-${sha256(inventoryRow(entry)).slice(0, 32)}`; }
function packagePurl(entry) {
  const version = entry.ecosystem === 'cargo' ? entry.version : fieldValue(entry.version);
  if (!version) return null;
  return `pkg:${entry.ecosystem}/${encodeURIComponent(entry.name).replaceAll('%2F', '/')}@${encodeURIComponent(version)}`;
}
function lockPackage(entry) {
  const version = entry.ecosystem === 'cargo' ? entry.version : fieldValue(entry.version);
  const pkg = { SPDXID: lockPackageId(entry), name: entry.name, downloadLocation: 'NOASSERTION', filesAnalyzed: false, licenseConcluded: 'NOASSERTION', licenseDeclared: 'NOASSERTION', copyrightText: 'NOASSERTION', comment: `DEOS-LOCK-ROW ${inventoryRow(entry)}` };
  if (version) pkg.versionInfo = version;
  const purl = packagePurl(entry);
  if (purl) pkg.externalRefs = [{ referenceCategory: 'PACKAGE-MANAGER', referenceType: 'purl', referenceLocator: purl }];
  return pkg;
}
function packageLockRow(pkg) {
  if (typeof pkg.comment !== 'string' || !pkg.comment.startsWith('DEOS-LOCK-ROW ')) fail(`SPDX package lacks exact lock provenance: ${pkg.SPDXID ?? '(unknown)'}`);
  let row; try { row = JSON.parse(pkg.comment.slice('DEOS-LOCK-ROW '.length)); } catch { fail(`SPDX package lock provenance is malformed: ${pkg.SPDXID ?? '(unknown)'}`); }
  return row;
}
function packageIdentity(pkg) {
  const row = packageLockRow(pkg); const version = row.ecosystem === 'cargo' ? row.version : fieldValue(row.version);
  return `${row.ecosystem}\0${row.name}\0${version ?? ''}`;
}
export function validateSbom(sbom, inventory, expected, schema) {
  validateSpdxSchema(schema, sbom);
  if (sbom.spdxVersion !== 'SPDX-2.3' || sbom.dataLicense !== 'CC0-1.0' || sbom.SPDXID !== 'SPDXRef-DOCUMENT') fail('SPDX document header is invalid');
  if (sbom.name !== expected.name || sbom.documentNamespace !== expected.namespace || sbom.creationInfo?.created !== expected.created || canonicalJson(sbom.creationInfo?.creators) !== canonicalJson([SBOM_CREATOR]) || !Array.isArray(sbom.packages) || !Array.isArray(sbom.relationships)) fail('SPDX deterministic identity or creation authority is invalid');
  if (Array.isArray(sbom.files) && sbom.files.length !== 0) fail('Lock-only SPDX document must not contain files');
  const expectedRows = inventory.map(inventoryRow); const actualRows = sbom.packages.map((pkg) => inventoryRow(packageLockRow(pkg)));
  if (canonicalJson(actualRows) !== canonicalJson(expectedRows)) fail('SPDX packages do not reconcile one-to-one with exact lock rows');
  const ids = new Set();
  for (const [index, pkg] of sbom.packages.entries()) {
    const row = inventory[index]; const version = row.ecosystem === 'cargo' ? row.version : fieldValue(row.version);
    if (pkg.SPDXID !== lockPackageId(row) || ids.has(pkg.SPDXID) || pkg.name !== row.name || pkg.versionInfo !== version || pkg.downloadLocation !== 'NOASSERTION' || pkg.filesAnalyzed !== false || typeof pkg.licenseConcluded !== 'string' || typeof pkg.licenseDeclared !== 'string') fail(`SPDX package differs from exact lock row: ${row.owner}:${row.location}`);
    if ((packagePurl(row) ?? null) !== (pkg.externalRefs?.find((entry) => entry.referenceType === 'purl')?.referenceLocator ?? null)) fail(`SPDX package purl differs from exact lock row: ${row.owner}:${row.location}`);
    ids.add(pkg.SPDXID);
  }
  const described = [...ids].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  if (canonicalJson(sbom.documentDescribes) !== canonicalJson(described)) fail('SPDX documentDescribes does not exactly cover lock packages');
  const expectedRelationships = described.map((id) => ({ spdxElementId: 'SPDXRef-DOCUMENT', relationshipType: 'DESCRIBES', relatedSpdxElement: id }));
  if (canonicalJson(sbom.relationships) !== canonicalJson(expectedRelationships)) fail('SPDX relationships do not exactly describe lock packages');
  const identities = new Set(sbom.packages.map(packageIdentity));
  for (const root of [`cargo\0deos-runtime\0${expected.version}`, `npm\0web-client\0${expected.version}`, `npm\0@deos/release-tooling\0${expected.version}`]) if (!identities.has(root)) fail(`SBOM required root is missing: ${root.split('\0')[1]}`);
  return sbom;
}
async function exactLockBytes(repo, treeOid) {
  const trackedLocks = git(repo, 'ls-tree', '-r', '--name-only', treeOid).split('\n').filter((member) => member.endsWith('Cargo.lock') || member.endsWith('package-lock.json')).sort();
  if (canonicalJson(trackedLocks) !== canonicalJson([...LOCK_PATHS].sort())) fail('Exact tag tree contains an unowned or missing package lock');
  const result = new Map();
  for (const member of LOCK_PATHS) {
    const file = path.join(repo, member); await regularNonempty(file, member); const bytes = await readFile(file); const fileOid = git(repo, 'hash-object', file); const treeOidForFile = git(repo, 'rev-parse', `${treeOid}:${member}`);
    if (fileOid !== treeOidForFile) fail(`Package lock differs from exact tag tree: ${member}`); result.set(member, bytes);
  }
  return result;
}
async function writeLocks(staging, locks) {
  for (const [member, bytes] of locks) { const destination = path.join(staging, member); await mkdir(path.dirname(destination), { recursive: true }); await writeFile(destination, bytes); }
}
async function stagedLockBytes(root) {
  const result = new Map();
  for (const member of LOCK_PATHS) result.set(member, await readFile(path.join(root, member)));
  return result;
}
function canonicalLockSbom(inventory, expected) {
  const packages = inventory.map(lockPackage).sort((a, b) => Buffer.compare(Buffer.from(a.SPDXID), Buffer.from(b.SPDXID)));
  const inventoryById = new Map(inventory.map((entry) => [lockPackageId(entry), entry]));
  const orderedInventory = packages.map((pkg) => inventoryById.get(pkg.SPDXID));
  const documentDescribes = packages.map((pkg) => pkg.SPDXID);
  const relationships = documentDescribes.map((id) => ({ spdxElementId: 'SPDXRef-DOCUMENT', relationshipType: 'DESCRIBES', relatedSpdxElement: id }));
  return { document: { spdxVersion: 'SPDX-2.3', dataLicense: 'CC0-1.0', SPDXID: 'SPDXRef-DOCUMENT', name: expected.name, documentNamespace: expected.namespace, creationInfo: { created: expected.created, creators: [SBOM_CREATOR] }, packages, documentDescribes, relationships }, inventory: orderedInventory };
}
export async function generateSbom(repo, tag, repository, outputParent, schema) {
  const locks = await exactLockBytes(repo, tag.treeOid); const roots = [];
  try {
    roots.push(await mkdtemp(path.join(outputParent, '.sbom-locks-a-')), await mkdtemp(path.join(outputParent, '.sbom-locks-b-')));
    for (const root of roots) await writeLocks(root, locks);
    const expected = { name: `deos-locks-v${tag.version}-${tag.treeOid}`, created: new Date(Number(git(repo, 'show', '-s', '--format=%ct', tag.commitOid)) * 1000).toISOString().replace('.000Z', 'Z'), namespace: `https://github.com/${repository}/spdx/v${tag.version}/${tag.treeOid}`, version: tag.version };
    const generations = [];
    for (const root of roots) {
      const generatedInventory = lockInventory(await stagedLockBytes(root));
      const projected = canonicalLockSbom(generatedInventory, expected);
      const reconciliationInventory = lockInventory(await stagedLockBytes(root));
      validateSbom(projected.document, projected.inventory, expected, schema);
      if (canonicalJson(projected.inventory.map(inventoryRow)) !== canonicalJson(reconciliationInventory.sort((a, b) => Buffer.compare(Buffer.from(lockPackageId(a)), Buffer.from(lockPackageId(b)))).map(inventoryRow))) fail('SPDX generation and independent lock reconciliation differ');
      generations.push(Buffer.from(`${canonicalJson(projected.document)}\n`));
    }
    if (!generations[0].equals(generations[1])) fail('Canonical DEOS SPDX generations from different roots are not byte-identical');
    for (const root of roots) if (generations[0].includes(Buffer.from(root))) fail('Canonical SPDX contains a host staging path');
    return generations[0];
  } finally { for (const root of roots) await rm(root, { recursive: true, force: true }); }
}
export function releasePayloadNames(version) {
  return [`actors-abi-manifest-v${version}.json`, `actors-fee-envelope-vectors-v${version}.json`, `actors-semantic-manifest-v${version}.json`, `deos-descriptors-v${version}.tar.gz`, `deos-runtime-v${version}.compact.compressed.wasm`, `deos-runtime-v${version}.scale`, `deos-v${version}.spdx.json`, `ingress-runtime-evidence-v${version}.ts`, `network-summary-v${version}.json`, `observation-runtime-evidence-v${version}.ts`, `validation-summary-v${version}.json`].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
}
export function releaseInventoryNames(version) { return [...releasePayloadNames(version), 'release-manifest.json', 'SHA256SUMS'].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b))); }
function releaseInventory(payload) {
  return [...payload.map((entry) => ({ role: 'payload', ...entry })), { path: 'release-manifest.json', role: 'self-describing-manifest', digestPolicy: 'intentionally-omitted-to-avoid-self-reference' }, { path: 'SHA256SUMS', role: 'checksum-control', digestPolicy: 'excluded-only-from-itself' }].sort((a, b) => Buffer.compare(Buffer.from(a.path), Buffer.from(b.path)));
}
function validateReleaseInventoryEntry(entry) {
  if (entry?.role === 'payload') { exactKeys(entry, ['path', 'role', 'bytes', 'sha256'], 'Release payload inventory entry'); validateMember({ path: entry.path, bytes: entry.bytes, sha256: entry.sha256 }); return entry; }
  if (entry?.role === 'self-describing-manifest') { exactKeys(entry, ['path', 'role', 'digestPolicy'], 'Release manifest inventory entry'); if (entry.path !== 'release-manifest.json' || entry.digestPolicy !== 'intentionally-omitted-to-avoid-self-reference') fail('Release manifest recursion policy is invalid'); return entry; }
  if (entry?.role === 'checksum-control') { exactKeys(entry, ['path', 'role', 'digestPolicy'], 'Release checksum inventory entry'); if (entry.path !== 'SHA256SUMS' || entry.digestPolicy !== 'excluded-only-from-itself') fail('Release checksum recursion policy is invalid'); return entry; }
  fail('Release inventory role is invalid');
}
async function copyAsset(source, destination) { await regularNonempty(source, source); await copyFile(source, destination, 0); }
export async function createReleaseBundle(options) {
  const repo = path.resolve(options.repo); const output = path.resolve(options.output); const verified = await verifyCandidate({ input: options.candidate, repo, repositoryId: options.repositoryId, tagRef: options.tagRef, runId: options.runId, runAttempt: options.runAttempt, manifestSha256: options.candidateManifestSha256 });
  const lockBytes = await readFile(options.toolLock); const lock = validateToolLock(JSON.parse(lockBytes)); const lockSha = sha(lockBytes);
  const schemaBytes = await readFile(options.spdxSchema); if (sha256(schemaBytes) !== lock.spdxSchema.sha256) fail('SPDX schema differs from exact tool lock'); const schema = JSON.parse(schemaBytes); if (schema.$schema !== 'http://json-schema.org/draft-07/schema#' || schema.$id !== 'http://spdx.org/rdf/terms/2.3') fail('Pinned SPDX schema identity is invalid');
  const networkBytes = await readFile(options.networkSummary); const network = validateNetworkSummary(JSON.parse(networkBytes), verified.manifest, verified.manifestSha256, lockSha); if (!networkBytes.equals(Buffer.from(`${canonicalJson(network)}\n`))) fail('Network summary is not canonical JSON');
  await mkdir(output, { recursive: false, mode: 0o700 }); const version = verified.manifest.tag.version; const source = path.join(path.resolve(options.candidate), FILES_ROOT);
  const assets = [
    [`deos-runtime-v${version}.compact.compressed.wasm`, path.join(source, WASM_PATH)],
    [`deos-runtime-v${version}.scale`, path.join(source, 'web-client/.papi/metadata/deos.scale')],
    [`actors-abi-manifest-v${version}.json`, path.join(source, 'web-client/src/lib/automation/actors-abi-manifest.json')],
    [`actors-semantic-manifest-v${version}.json`, path.join(source, 'web-client/src/lib/automation/actors-semantic-manifest.json')],
    [`actors-fee-envelope-vectors-v${version}.json`, path.join(source, 'web-client/src/lib/automation/actors-fee-envelope-vectors.json')],
    [`ingress-runtime-evidence-v${version}.ts`, path.join(source, 'web-client/src/lib/automation/ingress-runtime-evidence.generated.ts')],
    [`observation-runtime-evidence-v${version}.ts`, path.join(source, 'web-client/src/lib/observation/runtime-evidence.generated.ts')],
    [`validation-summary-v${version}.json`, path.join(source, 'validation/validation-summary.json')],
  ];
  for (const [name, file] of assets) await copyAsset(file, path.join(output, name));
  await writeFile(path.join(output, `network-summary-v${version}.json`), networkBytes, { flag: 'wx', mode: 0o600 });
  const commitTime = Number(git(repo, 'show', '-s', '--format=%ct', verified.manifest.tag.commitOid)); const archive = await deterministicDescriptorArchive(path.join(source, 'web-client/.papi/descriptors'), commitTime); await writeFile(path.join(output, `deos-descriptors-v${version}.tar.gz`), archive, { flag: 'wx', mode: 0o600 });
  const sbom = await generateSbom(repo, verified.manifest.tag, options.repository, path.dirname(output), schema); await writeFile(path.join(output, `deos-v${version}.spdx.json`), sbom, { flag: 'wx', mode: 0o600 });
  const payload = (await enumerate(output)).files; const entries = []; for (const member of payload.sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)))) { const bytes = await readFile(path.join(output, member)); entries.push({ path: member, bytes: bytes.length, sha256: sha(bytes) }); }
  const manifest = { schema: RELEASE_MANIFEST_SCHEMA, repository: options.repository, repositoryId: verified.manifest.repositoryId, tag: verified.manifest.tag, workflow: { path: '.github/workflows/release-candidate.yml', ...verified.manifest.workflow }, candidateManifestSha256: verified.manifestSha256, networkSummarySha256: sha(networkBytes), toolLockSha256: lockSha, sbomSha256: sha(sbom), recursionPolicy: { releaseManifest: 'self-listed-without-size-or-digest', sha256sums: 'hashes-all-other-inventory-members-and-excludes-only-itself' }, inventory: releaseInventory(entries) };
  await writeCanonical(path.join(output, 'release-manifest.json'), manifest);
  const checksummed = (await enumerate(output)).files.sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b))); const sums = []; for (const member of checksummed) sums.push(`${sha256(await readFile(path.join(output, member)))}  ${member}`); await writeFile(path.join(output, 'SHA256SUMS'), `${sums.join('\n')}\n`, { flag: 'wx', mode: 0o600 });
  await verifyReleaseBundle(output); return { manifest, assets: releaseInventoryNames(version) };
}
export async function verifyReleaseBundle(root) {
  const inventory = await enumerate(root); const names = inventory.files; if (!names.includes('release-manifest.json') || !names.includes('SHA256SUMS')) fail('Release bundle control files are missing');
  const manifestBytes = await readFile(path.join(root, 'release-manifest.json')); const manifest = JSON.parse(manifestBytes); exactKeys(manifest, ['schema', 'repository', 'repositoryId', 'tag', 'workflow', 'candidateManifestSha256', 'networkSummarySha256', 'toolLockSha256', 'sbomSha256', 'recursionPolicy', 'inventory'], 'Release manifest'); if (manifest.schema !== RELEASE_MANIFEST_SCHEMA || !manifestBytes.equals(Buffer.from(`${canonicalJson(manifest)}\n`))) fail('Release manifest is invalid or noncanonical');
  identifier(manifest.repository, 'Release repository', /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/); unsigned(manifest.repositoryId, 'Release repository ID'); exactKeys(manifest.tag, ['version', 'ref', 'oid', 'commitOid', 'treeOid'], 'Release tag'); identifier(manifest.tag.version, 'Release version', /^[0-9]+\.[0-9]+\.[0-9]+$/); if (manifest.tag.ref !== `refs/tags/v${manifest.tag.version}`) fail('Release tag/version mismatch'); oid(manifest.tag.oid, 'Release tag OID'); oid(manifest.tag.commitOid, 'Release commit OID'); oid(manifest.tag.treeOid, 'Release tree OID'); exactKeys(manifest.workflow, ['path', 'runId', 'runAttempt'], 'Release workflow'); if (manifest.workflow.path !== '.github/workflows/release-candidate.yml') fail('Release workflow path is invalid'); unsigned(manifest.workflow.runId, 'Release run ID'); unsigned(manifest.workflow.runAttempt, 'Release run attempt'); for (const field of ['candidateManifestSha256', 'networkSummarySha256', 'toolLockSha256', 'sbomSha256']) digest(manifest[field], `Release ${field}`);
  exactKeys(manifest.recursionPolicy, ['releaseManifest', 'sha256sums'], 'Release recursion policy'); if (manifest.recursionPolicy.releaseManifest !== 'self-listed-without-size-or-digest' || manifest.recursionPolicy.sha256sums !== 'hashes-all-other-inventory-members-and-excludes-only-itself') fail('Release recursion policy is invalid');
  if (!Array.isArray(manifest.inventory) || manifest.inventory.length !== 13) fail('Release manifest must persist exactly 13 inventory entries'); const entries = manifest.inventory.map(validateReleaseInventoryEntry); const inventoryNames = entries.map((entry) => entry.path); if (new Set(inventoryNames).size !== inventoryNames.length || canonicalJson(inventoryNames) !== canonicalJson(releaseInventoryNames(manifest.tag.version))) fail('Release manifest inventory allowlist is invalid'); const payload = entries.filter((entry) => entry.role === 'payload'); if (payload.length !== 11 || canonicalJson(payload.map((entry) => entry.path)) !== canonicalJson(releasePayloadNames(manifest.tag.version))) fail('Release payload inventory is invalid'); const byPath = new Map(payload.map((entry) => [entry.path, entry])); if (byPath.get(`network-summary-v${manifest.tag.version}.json`)?.sha256 !== manifest.networkSummarySha256 || byPath.get(`deos-v${manifest.tag.version}.spdx.json`)?.sha256 !== manifest.sbomSha256) fail('Release manifest bound digest mismatch'); if (canonicalJson(names) !== canonicalJson(inventoryNames) || inventory.directories.length !== 0) fail('Release bundle contains missing, extra, or nested files');
  for (const entry of payload) { const bytes = await readFile(path.join(root, entry.path)); if (bytes.length !== entry.bytes || sha(bytes) !== entry.sha256) fail(`Release asset differs from manifest: ${entry.path}`); }
  const sumBytes = await readFile(path.join(root, 'SHA256SUMS')); const lines = sumBytes.toString('utf8').split('\n').filter(Boolean); const sumNames = []; for (const line of lines) { const match = /^([0-9a-f]{64})  ([^\\\n]+)$/.exec(line); if (!match) fail('SHA256SUMS format is invalid'); relative(match[2], 'Checksum path'); if (match[2] === 'SHA256SUMS') fail('SHA256SUMS must not cover itself'); const bytes = await readFile(path.join(root, match[2])); if (sha256(bytes) !== match[1]) fail(`Checksum mismatch: ${match[2]}`); sumNames.push(match[2]); }
  const expectedSums = names.filter((name) => name !== 'SHA256SUMS'); if (canonicalJson(sumNames) !== canonicalJson(expectedSums)) fail('SHA256SUMS inventory is missing, extra, or unsorted'); return manifest;
}
function value(args, name) { const index = args.indexOf(name); if (index < 0 || !args[index + 1]) fail(`${name} is required`); const result = args[index + 1]; args.splice(index, 2); return result; }
async function main(argv) {
  const command = argv.shift();
  if (!command || ['-h', '--help'].includes(command)) {
    console.log('Usage: release-evidence.mjs create|verify|verify-chain-code|network-summary|package|verify-bundle [options]\n       create requires --github-output FILE'); return;
  }
  if (command === 'create') {
    const startedAt = new Date(value(argv, '--started-at'));
    if (!Number.isFinite(startedAt.valueOf())) fail('--started-at is invalid');
    const githubOutput = value(argv, '--github-output');
    const result = await createCandidate({ repo: value(argv, '--repo'), output: value(argv, '--output'), repositoryId: value(argv, '--repository-id'), tagRef: value(argv, '--tag-ref'), runId: value(argv, '--run-id'), runAttempt: value(argv, '--run-attempt'), startedAt: startedAt.toISOString(), requireFresh: true });
    if (argv.length) fail(`Unknown arguments: ${argv.join(' ')}`);
    await writeFile(githubOutput, `manifest-sha256=${result.manifestSha256}\n`, { flag: 'a' });
    console.log(`manifest-sha256=${result.manifestSha256}`);
  } else if (command === 'verify') {
    const options = { input: value(argv, '--input') };
    for (const [flag, key] of [['--repo','repo'],['--repository-id','repositoryId'],['--tag-ref','tagRef'],['--run-id','runId'],['--run-attempt','runAttempt'],['--manifest-sha256','manifestSha256'],['--materialize','materialize']]) if (argv.includes(flag)) options[key] = value(argv, flag);
    if (argv.length) fail(`Unknown arguments: ${argv.join(' ')}`);
    const result = await verifyCandidate(options); console.log(`manifest-sha256=${result.manifestSha256}`);
  } else if (command === 'verify-chain-code') {
    const result = await verifyChainCode(value(argv, '--wasm'), value(argv, '--chain-spec'));
    if (argv.length) fail(`Unknown arguments: ${argv.join(' ')}`);
    console.log(`${canonicalJson(result)}`);
  } else if (command === 'append-proof') {
    await appendNetworkProof(value(argv, '--ledger'), value(argv, '--id'));
    if (argv.length) fail(`Unknown arguments: ${argv.join(' ')}`);
  } else if (command === 'network-summary') {
    const options = { input: value(argv, '--input'), repo: value(argv, '--repo'), repositoryId: value(argv, '--repository-id'), tagRef: value(argv, '--tag-ref'), runId: value(argv, '--run-id'), runAttempt: value(argv, '--run-attempt'), manifestSha256: value(argv, '--manifest-sha256'), wasm: value(argv, '--wasm'), chainSpec: value(argv, '--chain-spec'), toolLock: value(argv, '--tool-lock'), proofLedger: value(argv, '--proof-ledger'), output: value(argv, '--output') };
    if (argv.length) fail(`Unknown arguments: ${argv.join(' ')}`);
    await writeNetworkSummary(options);
  } else if (command === 'package') {
    const options = { candidate: value(argv, '--candidate'), networkSummary: value(argv, '--network-summary'), repo: value(argv, '--repo'), repository: value(argv, '--repository'), repositoryId: value(argv, '--repository-id'), tagRef: value(argv, '--tag-ref'), runId: value(argv, '--run-id'), runAttempt: value(argv, '--run-attempt'), candidateManifestSha256: value(argv, '--candidate-manifest-sha256'), toolLock: value(argv, '--tool-lock'), spdxSchema: value(argv, '--spdx-schema'), output: value(argv, '--output') };
    if (argv.length) fail(`Unknown arguments: ${argv.join(' ')}`); const result = await createReleaseBundle(options); console.log(`release-assets=${result.assets.length}`);
  } else if (command === 'verify-bundle') {
    await verifyReleaseBundle(value(argv, '--input')); if (argv.length) fail(`Unknown arguments: ${argv.join(' ')}`);
  } else fail(`Unknown command: ${command}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(new URL(import.meta.url).pathname)) main(process.argv.slice(2)).catch((error) => { console.error(`release-evidence: ${error.message}`); process.exitCode = 1; });
