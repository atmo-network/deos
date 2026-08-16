import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { gunzipSync } from 'node:zlib';
import { chmod, lstat, mkdir, mkdtemp, readFile, rm, symlink, unlink, writeFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { assembleIdentity, canonicalJson } from './validation-evidence.mjs';
import { acceptanceInventory, appendNetworkProof, deterministicDescriptorArchive, generateSbom, lockInventory, NETWORK_PROOF_ORDER, releaseInventoryNames, releasePayloadNames, validateCandidateManifest, validateNetworkSummary, validateProofLedger, validateSbom, verifyCandidate, verifyChainCode, verifyReleaseBundle } from './release-evidence.mjs';
import { preflightCandidateZip, preflightSingleFileZip, verifyGithubArtifactProvenance } from './github-release-artifact.mjs';
import { validateToolLock, verifyInstalledTools } from './install-release-tools.mjs';

const oid = '1'.repeat(40);
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const sha = (bytes) => `sha256:${sha256(bytes)}`;
const outputs = [
  { path: 'template/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm', kind: 'file', requiredMembers: [] },
  { path: 'web-client/.papi/metadata/deos.scale', kind: 'file', requiredMembers: [] },
  { path: 'web-client/.papi/descriptors', kind: 'directory', requiredMembers: ['package.json', 'generated.json', 'dist/index.js', 'dist/index.d.ts'] },
  { path: 'web-client/src/lib/automation/actors-abi-manifest.json', kind: 'file', requiredMembers: [] },
  { path: 'web-client/src/lib/automation/actors-semantic-manifest.json', kind: 'file', requiredMembers: [] },
  { path: 'web-client/src/lib/automation/actors-fee-envelope-vectors.json', kind: 'file', requiredMembers: [] },
  { path: 'web-client/src/lib/automation/ingress-runtime-evidence.generated.ts', kind: 'file', requiredMembers: [] },
  { path: 'web-client/src/lib/observation/runtime-evidence.generated.ts', kind: 'file', requiredMembers: [] },
];
const sourcePaths = outputs.flatMap((output) => output.kind === 'file' ? [output.path] : output.requiredMembers.map((member) => `${output.path}/${member}`));
function run(cwd, command, args) { const result = spawnSync(command, args, { cwd, encoding: 'utf8' }); if (result.status !== 0) throw new Error(result.stderr); return result.stdout.trim(); }
async function fixture() {
  const base = await mkdtemp(path.join(tmpdir(), 'deos-candidate-test-')); const repo = path.join(base, 'repo'); const root = path.join(base, 'candidate');
  await mkdir(path.join(repo, 'scripts'), { recursive: true }); await mkdir(path.join(repo, 'template'), { recursive: true });
  await writeFile(path.join(repo, 'template/Cargo.toml'), '[workspace.package]\nversion = "0.7.18"\n');
  await writeFile(path.join(repo, 'scripts/validation-authority.v1.json'), `${JSON.stringify({ schema: 'validation-authority/v2', roots: ['scripts', 'template'], fullArtifactOutputs: outputs, immutableRefInputs: [] }, null, 2)}\n`);
  run(repo, 'git', ['init', '-q']); run(repo, 'git', ['config', 'user.email', 'test@example.com']); run(repo, 'git', ['config', 'user.name', 'Test']); run(repo, 'git', ['add', '.']); run(repo, 'git', ['commit', '-qm', 'fixture']); run(repo, 'git', ['tag', 'v0.7.18']);
  const tree = run(repo, 'git', ['rev-parse', 'HEAD^{tree}']); const commit = run(repo, 'git', ['rev-parse', 'HEAD']);
  const candidate = { treeOid: tree, indexTreeOid: tree, trackedClean: true, stagedClean: true, untracked: [], clean: true };
  const authorityIdentity = { schema: 'validation-authority/v2', sha256: sha('authority') }; const environment = { schema: 'deos-validation-environment/v2', values: {}, sha256: sha(canonicalJson({})) };
  const identity = assembleIdentity({ candidate, authority: authorityIdentity, environment, profile: 'full' });
  const bytes = new Map(sourcePaths.map((member) => [member, Buffer.from(member.endsWith('wasm') ? 'wasm' : member)]));
  const entries = [...bytes].sort((a, b) => Buffer.compare(Buffer.from(a[0]), Buffer.from(b[0]))).map(([member, value]) => ({ path: member, bytes: value.length, sha256: sha(value) }));
  const record = { schema: 'deos-validation-evidence/v2', key: identity.key, candidate, authority: authorityIdentity, environment, refIdentity: identity.refIdentity, freshness: identity.freshness, invocation: identity.invocation, repetition: identity.repetition, artifacts: { schema: 'deos-validation-full-artifacts/v2', entries }, conclusion: 'success', startedAt: '2026-01-01T00:00:00.000Z', completedAt: '2026-01-01T00:01:00.000Z' };
  const summary = { schema: 'deos-validation-summary/v1', conclusion: 'success', key: identity.key, treeOid: tree, startedAt: record.startedAt, completedAt: record.completedAt, artifactCount: entries.length };
  bytes.set('validation/full-evidence.json', Buffer.from(`${canonicalJson(record)}\n`)); bytes.set('validation/validation-summary.json', Buffer.from(`${canonicalJson(summary)}\n`));
  for (const [member, value] of bytes) { const file = path.join(root, 'files', member); await mkdir(path.dirname(file), { recursive: true }); await writeFile(file, value); }
  const members = [...bytes].sort((a, b) => Buffer.compare(Buffer.from(a[0]), Buffer.from(b[0]))).map(([member, value]) => ({ path: member, bytes: value.length, sha256: sha(value) }));
  const manifest = validateCandidateManifest({ schema: 'deos-release-candidate/v1', repositoryId: '7', tag: { version: '0.7.18', ref: 'refs/tags/v0.7.18', oid: commit, commitOid: commit, treeOid: tree }, workflow: { runId: '8', runAttempt: '1' }, validation: { key: identity.key, recordSha256: sha(bytes.get('validation/full-evidence.json')), summarySha256: sha(bytes.get('validation/validation-summary.json')) }, members });
  await writeFile(path.join(root, 'candidate-manifest.json'), `${canonicalJson(manifest)}\n`);
  return { base, repo, root, manifest, record };
}
async function rejectsMutation(mutator, pattern) { const state = await fixture(); try { await mutator(state); await assert.rejects(() => verifyCandidate({ input: state.root, repo: state.repo }), pattern); } finally { await rm(state.base, { recursive: true, force: true }); } }

test('candidate verifier accepts only the candidate-tree authority plus successful full record inventory', async () => {
  const state = await fixture(); try { const result = await verifyCandidate({ input: state.root, repo: state.repo, repositoryId: '7', tagRef: 'refs/tags/v0.7.18', runId: '8', runAttempt: '1' }); assert.equal(result.manifest.members.length, sourcePaths.length + 2); } finally { await rm(state.base, { recursive: true, force: true }); }
});
test('acceptance inventory independently requires every authority family and forbids record extras', () => {
  const entries = sourcePaths.map((entry) => ({ path: entry, bytes: 1, sha256: sha('x') }));
  assert.equal(acceptanceInventory({ fullArtifactOutputs: outputs }, entries).length, sourcePaths.length);
  assert.throws(() => acceptanceInventory({ fullArtifactOutputs: outputs }, entries.filter((entry) => !entry.path.includes('actors-fee'))), /exact file family/);
  assert.throws(() => acceptanceInventory({ fullArtifactOutputs: outputs }, [...entries, { path: '.hidden', bytes: 1, sha256: sha('x') }]), /outside candidate-tree authority/);
  assert.throws(() => acceptanceInventory({ fullArtifactOutputs: outputs.slice(0, 7) }, entries), /eight/);
});
test('full-record path, size, digest and candidate bytes mismatches fail closed', async () => {
  await rejectsMutation(async ({ root }) => writeFile(path.join(root, 'files', sourcePaths[0]), 'other'), /identity mismatch/);
  await rejectsMutation(async ({ root }) => {
    const changed = Buffer.from('other'); const manifestFile = path.join(root, 'candidate-manifest.json'); const manifest = JSON.parse(await readFile(manifestFile));
    const entry = manifest.members.find((member) => member.path === sourcePaths[0]); entry.bytes = changed.length; entry.sha256 = sha(changed);
    manifest.members.sort((a, b) => Buffer.compare(Buffer.from(a.path), Buffer.from(b.path)));
    await writeFile(path.join(root, 'files', sourcePaths[0]), changed); await writeFile(manifestFile, `${canonicalJson(manifest)}\n`);
  }, /differs from successful full record/);
  await rejectsMutation(async ({ root }) => { const file = path.join(root, 'files/validation/full-evidence.json'); const record = JSON.parse(await readFile(file)); record.artifacts.entries[0].bytes += 1; await writeFile(file, `${canonicalJson(record)}\n`); }, /record identity|Evidence key|member identity/);
});
test('physical hidden/extra/missing/empty/symlink members and traversal fail closed', async () => {
  await rejectsMutation(({ root }) => unlink(path.join(root, 'files', sourcePaths[0])), /missing or extra/);
  await rejectsMutation(({ root }) => writeFile(path.join(root, '.hidden'), 'x'), /missing or extra/);
  await rejectsMutation(({ root }) => mkdir(path.join(root, '.hidden-directory')), /extra or missing directory/);
  await rejectsMutation(({ root }) => writeFile(path.join(root, 'files', sourcePaths[0]), ''), /nonempty/);
  await rejectsMutation(async ({ root }) => { const file = path.join(root, 'files', sourcePaths[0]); await unlink(file); await symlink('/etc/passwd', file); }, /Symlink/);
  const state = await fixture(); try { const bad = structuredClone(state.manifest); bad.members[0].path = '../escape'; assert.throws(() => validateCandidateManifest(bad), /traversal|relative/); } finally { await rm(state.base, { recursive: true, force: true }); }
});
test('chain-spec genesis :code must equal candidate Wasm bytes', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'deos-chain-code-')); try { const wasm = path.join(root, 'runtime.wasm'); const spec = path.join(root, 'spec.json'); await writeFile(wasm, 'wasm'); await writeFile(spec, JSON.stringify({ genesis: { raw: { top: { '0x3a636f6465': `0x${Buffer.from('wasm').toString('hex')}` } } } })); await verifyChainCode(wasm, spec); await writeFile(spec, JSON.stringify({ genesis: { raw: { top: { '0x3a636f6465': '0x00' } } } })); await assert.rejects(() => verifyChainCode(wasm, spec), /do not exactly equal/); } finally { await rm(root, { recursive: true, force: true }); }
});
function zip(entries) {
  const local = []; const central = []; let offset = 0;
  for (const entry of entries) { const name = Buffer.from(entry.name); const data = Buffer.from(entry.data ?? 'x'); const localHeader = Buffer.alloc(30); localHeader.writeUInt32LE(0x04034b50); localHeader.writeUInt16LE(20, 4); localHeader.writeUInt32LE(data.length, 18); localHeader.writeUInt32LE(entry.size ?? data.length, 22); localHeader.writeUInt16LE(name.length, 26); local.push(localHeader, name, data); const header = Buffer.alloc(46); header.writeUInt32LE(0x02014b50); header.writeUInt16LE(0x0314, 4); header.writeUInt16LE(20, 6); header.writeUInt32LE(data.length, 20); header.writeUInt32LE(entry.size ?? data.length, 24); header.writeUInt16LE(name.length, 28); header.writeUInt32LE(((entry.mode ?? 0o100644) << 16) >>> 0, 38); header.writeUInt32LE(offset, 42); central.push(header, name); offset += localHeader.length + name.length + data.length; }
  const centralBytes = Buffer.concat(central); const end = Buffer.alloc(22); end.writeUInt32LE(0x06054b50); end.writeUInt16LE(entries.length, 8); end.writeUInt16LE(entries.length, 10); end.writeUInt32LE(centralBytes.length, 12); end.writeUInt32LE(offset, 16); return Buffer.concat([...local, centralBytes, end]);
}
test('ZIP preflight rejects traversal, symlink, duplicate, bomb, extra and producer manifest mismatch before extraction', async () => {
  const state = await fixture(); try { const manifest = await readFile(path.join(state.root, 'candidate-manifest.json')); const validEntries = [{ name: 'candidate-manifest.json', data: manifest }, ...state.manifest.members.map((member) => ({ name: `files/${member.path}`, data: 'x' }))]; assert.throws(() => preflightCandidateZip(zip([{ name: '../escape', data: 'x' }])), /Unsafe/); assert.throws(() => preflightCandidateZip(zip([{ name: 'candidate-manifest.json', data: manifest, mode: 0o120777 }])), /regular/); assert.throws(() => preflightCandidateZip(zip([validEntries[0], validEntries[0]])), /Duplicate/); assert.throws(() => preflightCandidateZip(zip([{ name: 'candidate-manifest.json', data: manifest, size: 600 * 1024 * 1024 }])), /bound/); assert.throws(() => preflightCandidateZip(zip([...validEntries, { name: 'extra', data: 'x' }])), /missing or extra/); assert.throws(() => preflightCandidateZip(zip(validEntries), sha('wrong manifest')), /manifest digest mismatch/); } finally { await rm(state.base, { recursive: true, force: true }); }
});
test('GitHub API provenance requests only real exact routes and binds producer outputs', async () => {
  const options = { repository: 'o/r', repositoryId: '7', runId: '8', runAttempt: '2', jobName: 'Full Release Validation / full validation', tagName: 'v0.7.18', headSha: oid, artifactName: 'deos-candidate-8-2', expectedArtifactId: '10', expectedArtifactDigest: sha('zip'), expectedManifestSha256: sha('manifest'), token: 'x' };
  const base = 'https://api.github.com/repos/o/r';
  const urls = [`${base}`, `${base}/actions/runs/8/attempts/2`, `${base}/actions/runs/8/attempts/2/jobs?per_page=100`, `${base}/actions/runs/8/artifacts?per_page=100`, `${base}/actions/artifacts/10`];
  const responses = new Map([
    [urls[0], { id: 7, full_name: 'o/r' }],
    [urls[1], { id: 8, run_attempt: 2, repository: { id: 7 }, event: 'push', head_sha: oid, head_branch: 'v0.7.18', path: '.github/workflows/release-candidate.yml' }],
    [urls[2], { jobs: [{ id: 9, name: options.jobName, head_sha: oid, status: 'completed', conclusion: 'success', steps: [{ name: 'Upload immutable candidate handoff', status: 'completed', conclusion: 'success' }] }] }],
    [urls[3], { artifacts: [{ id: 10, name: options.artifactName, digest: options.expectedArtifactDigest }] }],
    [urls[4], { id: 10, name: options.artifactName, expired: false, digest: options.expectedArtifactDigest, workflow_run: { id: 8, repository_id: 7, head_sha: oid } }],
  ]);
  const requested = [];
  const request = async (url) => { requested.push(url); assert.ok(responses.has(url), `nonexistent or unexpected GitHub API route: ${url}`); return structuredClone(responses.get(url)); };
  assert.equal((await verifyGithubArtifactProvenance(options, request)).artifactId, '10');
  assert.deepEqual(requested, urls);
  assert.ok(requested.every((url) => !url.includes('/attempts/2/artifacts')));
  await assert.rejects(() => verifyGithubArtifactProvenance({ ...options, expectedArtifactId: '' }, request), /producer artifact ID/);
  await assert.rejects(() => verifyGithubArtifactProvenance({ ...options, expectedArtifactDigest: '' }, request), /producer artifact digest/);
  await assert.rejects(() => verifyGithubArtifactProvenance({ ...options, expectedManifestSha256: '' }, request), /producer manifest digest/);
  await assert.rejects(() => verifyGithubArtifactProvenance({ ...options, expectedArtifactId: '11' }, async (url) => { assert.ok(responses.has(url), `nonexistent route: ${url}`); return structuredClone(responses.get(url)); }), /producer artifact ID/);
  const mismatch = new Map(responses); mismatch.set(urls[4], { ...responses.get(urls[4]), digest: sha('other') });
  await assert.rejects(() => verifyGithubArtifactProvenance(options, async (url) => { assert.ok(mismatch.has(url), `nonexistent route: ${url}`); return structuredClone(mismatch.get(url)); }), /producer outputs/);
});
test('proof ledger is append-only, exact, ordered and cannot summarize failed or missing steps', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'deos-proof-')); const ledger = path.join(root, 'proof.jsonl'); try { await assert.rejects(() => appendNetworkProof(ledger, NETWORK_PROOF_ORDER[1]), /Expected next/); for (const id of NETWORK_PROOF_ORDER) await appendNetworkProof(ledger, id); assert.deepEqual(validateProofLedger(await readFile(ledger, 'utf8')).map((record) => record.id), NETWORK_PROOF_ORDER); await writeFile(ledger, `${canonicalJson({ schema: 'deos-release-network-proof/v1', sequence: 9, id: 'extra', completedAt: new Date().toISOString() })}\n`, { flag: 'a' }); const invalid = await readFile(ledger); assert.throws(() => validateProofLedger(requireText(invalid)), /invalid|extra/); } finally { await rm(root, { recursive: true, force: true }); }
});
function requireText(bytes) { return bytes.toString('utf8'); }
test('release workflow output chain, entrypoint and network order are statically enforced', async () => {
  const reusable = await readFile(new URL('../.github/workflows/_validate.yml', import.meta.url), 'utf8');
  const workflow = await readFile(new URL('../.github/workflows/release-candidate.yml', import.meta.url), 'utf8');
  for (const source of [reusable, workflow]) for (const use of source.matchAll(/uses:\s*([^\s]+)/g)) if (!use[1].startsWith('./')) assert.match(use[1], /@[0-9a-f]{40}$/);
  for (const output of ['candidate-artifact-id', 'candidate-artifact-digest', 'candidate-manifest-sha256']) {
    assert.match(reusable, new RegExp(`value: \\$\\{\\{ jobs\\.validate\\.outputs\\.${output} \\}\\}`));
    assert.match(workflow, new RegExp(`needs\\.full\\.outputs\\.${output}`));
  }
  assert.match(reusable, /id: candidate-create[\s\S]*--github-output "\$GITHUB_OUTPUT"/);
  assert.match(reusable, /id: candidate-upload/);
  assert.match(reusable, /candidate-artifact-id: \$\{\{ steps\.candidate-upload\.outputs\.artifact-id \}\}[\s\S]*candidate-artifact-digest: \$\{\{ format\('sha256:\{0\}', steps\.candidate-upload\.outputs\.artifact-digest\) \}\}[\s\S]*candidate-manifest-sha256: \$\{\{ steps\.candidate-create\.outputs\.manifest-sha256 \}\}/);
  assert.match(workflow, /--expected-artifact-id "\$\{\{ needs\.full\.outputs\.candidate-artifact-id \}\}"[\s\S]*--expected-artifact-digest "\$\{\{ needs\.full\.outputs\.candidate-artifact-digest \}\}"[\s\S]*--expected-manifest-sha256 "\$\{\{ needs\.full\.outputs\.candidate-manifest-sha256 \}\}"/);
  assert.ok(workflow.indexOf('github-release-artifact.mjs download') < workflow.indexOf('release-evidence.mjs verify'));
  const patchMode = (await lstat(new URL('./patch-chain-spec.mjs', import.meta.url))).mode; assert.notEqual(patchMode & 0o111, 0);
  const network = await readFile(new URL('./network-assurance-local.sh', import.meta.url), 'utf8'); const ordered = ['record_proof finalizedRelayAndTwoCollators', 'verify_collator_participation 1', 'verify_collator_failover', 'record_proof signedPreRestartTransfer', 'restart_dave_with_persisted_state', 'record_proof signedPostRestartTransfer', 'WS_ENDPOINT="ws://127.0.0.1:9999"', 'record_proof routerOracleBurnActor', 'cleanup_owned_processes', 'write_candidate_summary']; let cursor = -1; for (const anchor of ordered) { const next = network.indexOf(anchor, cursor + 1); assert.ok(next > cursor, `missing/out-of-order ${anchor}`); cursor = next; } assert.doesNotMatch(network.slice(network.indexOf('cleanup_owned_processes()'), network.indexOf('failure_cleanup()')), /stop_background_process/);
});
test('stable2606 tool lock has one exact inventory and rejects extra/missing/weak pins and PATH binaries', async () => {
  const lock = JSON.parse(await readFile(new URL('./release-tools.v1.json', import.meta.url))); assert.equal(validateToolLock(lock).schema, 'deos-release-tools/v2'); assert.deepEqual(lock.tools.map((tool) => tool.name), ['chain-spec-builder', 'polkadot', 'polkadot-omni-node', 'zombienet']); assert.ok(lock.tools.filter((tool) => tool.source.includes('polkadot-sdk')).every((tool) => tool.url.includes('polkadot-stable2606-1'))); const cargoToml = await readFile(new URL('../template/Cargo.toml', import.meta.url), 'utf8'); assert.match(cargoToml, /polkadot-sdk = \{ version = "2606\.0\.0"/); const cargoLock = await readFile(new URL('../template/Cargo.lock', import.meta.url), 'utf8'); const packageBlock = cargoLock.match(/name = "staging-chain-spec-builder"[\s\S]*?\n\]/)?.[0] ?? ''; const chain = lock.tools[0].cargoPackage; assert.match(packageBlock, new RegExp(`version = "${chain.version.replaceAll('.', '\\.')}"`)); assert.match(packageBlock, new RegExp(`checksum = "${chain.checksum}"`)); for (const mutate of [(value) => value.tools.pop(), (value) => value.tools.push(structuredClone(value.tools[0])), (value) => { value.tools[0].cargoPackage.checksum = '0'.repeat(64); }, (value) => { value.tools.push({ name: 'syft' }); }, (value) => { value.spdxSchema.url = 'https://raw.githubusercontent.com/spdx/spdx-spec/master/schemas/spdx-schema.json'; }]) { const bad = structuredClone(lock); mutate(bad); assert.throws(() => validateToolLock(bad)); }
  const bin = await mkdtemp(path.join(tmpdir(), 'deos-tools-')); try { for (const tool of lock.tools) { const file = path.join(bin, tool.name); await writeFile(file, '#!/bin/sh\necho wrong\n'); await chmod(file, 0o700); } await assert.rejects(() => verifyInstalledTools(lock, bin, bin), /digest differs/); await writeFile(path.join(bin, 'extra'), 'x'); await assert.rejects(() => verifyInstalledTools(lock, bin, bin), /extra or missing/); } finally { await rm(bin, { recursive: true, force: true }); }
});
test('release schema validator has one direct exact dependency and a coherent npm lock', async () => {
  const packageJson = JSON.parse(await readFile(new URL('./release-tooling/package.json', import.meta.url))); const packageLock = JSON.parse(await readFile(new URL('./release-tooling/package-lock.json', import.meta.url))); const source = await readFile(new URL('./release-tooling/validate-spdx.mjs', import.meta.url), 'utf8'); assert.deepEqual(packageJson.dependencies, { ajv: '8.20.0' }); assert.deepEqual(packageLock.packages[''].dependencies, packageJson.dependencies); assert.equal(packageLock.packages['node_modules/ajv'].version, packageJson.dependencies.ajv); assert.deepEqual([...source.matchAll(/from '([^']+)'/g)].map((match) => match[1]), ['ajv']); assert.ok(Object.keys(packageLock.packages).every((member) => member === '' || member.startsWith('node_modules/')));
});
test('SBOM production authority has no residual external scanner dependency or claim', async () => {
  for (const member of ['release-evidence.mjs', 'install-release-tools.mjs', 'release-tools.v1.json', 'README.md', '../.github/workflows/release-candidate.yml']) assert.doesNotMatch(await readFile(new URL(member, import.meta.url), 'utf8'), /syft/i, member);
});
test('candidate lock inventory has the exact release package cardinality', async () => {
  const owners = ['template/Cargo.lock', 'web-client/package-lock.json', '.agents/skills/wiki-sync/package-lock.json', 'scripts/release-tooling/package-lock.json']; const locks = new Map(); for (const owner of owners) locks.set(owner, await readFile(new URL(`../${owner}`, import.meta.url))); const inventory = lockInventory(locks); assert.equal(inventory.length, 1984); assert.equal(inventory.filter((row) => row.ecosystem === 'cargo').length, 1555); assert.equal(inventory.filter((row) => row.ecosystem === 'npm').length, 429); assert.deepEqual(Object.fromEntries(owners.map((owner) => [owner, inventory.filter((row) => row.owner === owner).length])), { 'template/Cargo.lock': 1555, 'web-client/package-lock.json': 421, '.agents/skills/wiki-sync/package-lock.json': 2, 'scripts/release-tooling/package-lock.json': 6 });
});
test('canonical authority independently inventories all eight current artifact families', async () => {
  const authority = JSON.parse(await readFile(new URL('./validation-authority.v1.json', import.meta.url))); assert.deepEqual(authority.fullArtifactOutputs.map((output) => output.path), outputs.map((output) => output.path));
});
test('ineffective owned-process cleanup is fatal', () => {
  const command = `source scripts/network-assurance-local.sh; PROCESS_CLEANUP_GRACE_ATTEMPTS=1; OWNED_PIDS=($$); kill(){ [[ \"$1\" == \"-0\" ]] && return 0; return 0; }; cleanup_owned_processes`;
  const result = spawnSync('bash', ['-c', command], { cwd: path.resolve(new URL('..', import.meta.url).pathname), encoding: 'utf8' }); assert.notEqual(result.status, 0); assert.match(`${result.stdout}\n${result.stderr}`, /remains alive/);
});

test('single-file artifact preflight rejects name, digest, and extra inventory', () => {
  const bytes = Buffer.from('summary'); const digest = sha(bytes);
  assert.deepEqual(preflightSingleFileZip(zip([{ name: 'network-summary.json', data: bytes }]), 'network-summary.json', digest), bytes);
  assert.throws(() => preflightSingleFileZip(zip([{ name: 'other.json', data: bytes }]), 'network-summary.json', digest), /inventory/);
  assert.throws(() => preflightSingleFileZip(zip([{ name: 'network-summary.json', data: bytes }, { name: 'extra', data: 'x' }]), 'network-summary.json', digest), /inventory/);
  assert.throws(() => preflightSingleFileZip(zip([{ name: 'network-summary.json', data: bytes }]), 'network-summary.json', sha('wrong')), /digest/);
});

const missing = { state: 'missing' };
const value = (entry) => ({ state: 'value', value: entry });
const sbomInventory = [
  { ecosystem: 'cargo', owner: 'template/Cargo.lock', location: 'package[0]', name: 'deos-runtime', version: '0.7.18', source: missing, checksum: missing },
  { ecosystem: 'npm', owner: 'web-client/package-lock.json', location: '', name: 'web-client', nameField: value('web-client'), version: value('0.7.18'), resolved: missing, integrity: missing, dev: missing, optional: missing, devOptional: missing, peer: missing, inBundle: missing, link: missing },
  { ecosystem: 'npm', owner: 'scripts/release-tooling/package-lock.json', location: '', name: '@deos/release-tooling', nameField: value('@deos/release-tooling'), version: value('0.7.18'), resolved: missing, integrity: missing, dev: missing, optional: missing, devOptional: missing, peer: missing, inBundle: missing, link: missing },
];
const sbomExpected = { name: `deos-locks-v0.7.18-${oid}`, namespace: 'https://github.com/atmo-network/deos/spdx/v0.7.18/tree', created: '2026-01-01T00:00:00Z', version: '0.7.18' };
const spdxTestSchema = { type: 'object', additionalProperties: false, required: ['spdxVersion', 'dataLicense', 'SPDXID', 'name', 'documentNamespace', 'creationInfo', 'packages', 'documentDescribes', 'relationships'], properties: { spdxVersion: { type: 'string' }, dataLicense: { type: 'string' }, SPDXID: { type: 'string' }, name: { type: 'string' }, documentNamespace: { type: 'string' }, creationInfo: { type: 'object' }, packages: { type: 'array' }, documentDescribes: { type: 'array' }, relationships: { type: 'array' } } };
function rowPackage(row) {
  const version = row.ecosystem === 'cargo' ? row.version : row.version.state === 'value' ? row.version.value : undefined;
  const pkg = { SPDXID: `SPDXRef-Package-lock-${sha256(canonicalJson(row)).slice(0, 32)}`, name: row.name, downloadLocation: 'NOASSERTION', filesAnalyzed: false, licenseConcluded: 'NOASSERTION', licenseDeclared: 'NOASSERTION', copyrightText: 'NOASSERTION', comment: `DEOS-LOCK-ROW ${canonicalJson(row)}` };
  if (version) { pkg.versionInfo = version; pkg.externalRefs = [{ referenceCategory: 'PACKAGE-MANAGER', referenceType: 'purl', referenceLocator: `pkg:${row.ecosystem}/${encodeURIComponent(row.name).replaceAll('%2F', '/')}@${encodeURIComponent(version)}` }]; }
  return pkg;
}
function sbomFixture(inventory = sbomInventory) {
  const packages = inventory.map(rowPackage).sort((a, b) => Buffer.compare(Buffer.from(a.SPDXID), Buffer.from(b.SPDXID))); const byId = new Map(inventory.map((row) => [rowPackage(row).SPDXID, row])); const ordered = packages.map((pkg) => byId.get(pkg.SPDXID)); const documentDescribes = packages.map((pkg) => pkg.SPDXID); return { sbom: { spdxVersion: 'SPDX-2.3', dataLicense: 'CC0-1.0', SPDXID: 'SPDXRef-DOCUMENT', name: sbomExpected.name, documentNamespace: sbomExpected.namespace, creationInfo: { created: sbomExpected.created, creators: ['Tool: deos-release-evidence-v1'] }, packages, documentDescribes, relationships: documentDescribes.map((id) => ({ spdxElementId: 'SPDXRef-DOCUMENT', relationshipType: 'DESCRIBES', relatedSpdxElement: id })) }, inventory: ordered };
}

test('canonical SPDX validation applies schema and exact one-to-one lock provenance', () => {
  const fixture = sbomFixture(); const first = Buffer.from(`${canonicalJson(validateSbom(fixture.sbom, fixture.inventory, sbomExpected, spdxTestSchema))}\n`); const secondFixture = sbomFixture(); const second = Buffer.from(`${canonicalJson(validateSbom(secondFixture.sbom, secondFixture.inventory, sbomExpected, spdxTestSchema))}\n`); assert.deepEqual(first, second);
  const forbidden = structuredClone(fixture.sbom); forbidden.forbidden = true; assert.throws(() => validateSbom(forbidden, fixture.inventory, sbomExpected, spdxTestSchema), /additional properties|schema validation/i);
  const omitted = structuredClone(fixture.sbom); omitted.packages.pop(); assert.throws(() => validateSbom(omitted, fixture.inventory, sbomExpected, spdxTestSchema), /one-to-one/);
  const mutateLockRow = (document, predicate, mutate) => { const pkg = document.packages.find((entry) => predicate(JSON.parse(entry.comment.slice('DEOS-LOCK-ROW '.length)))); const row = JSON.parse(pkg.comment.slice('DEOS-LOCK-ROW '.length)); mutate(row); pkg.comment = `DEOS-LOCK-ROW ${canonicalJson(row)}`; };
  for (const mutate of [(x) => { x.spdxVersion = 'SPDX-2.2'; }, (x) => { x.creationInfo.creators = ['Tool: omitted-scan']; }, (x) => { delete x.packages[0].licenseDeclared; }, (x) => { x.packages.push(structuredClone(x.packages[0])); }, (x) => { x.packages[0].name = '/tmp/host-substitution'; }, (x) => { x.relationships[0].relatedSpdxElement = 'SPDXRef-missing'; }, (x) => mutateLockRow(x, (row) => row.ecosystem === 'cargo', (row) => { row.source = value('registry+substituted'); }), (x) => mutateLockRow(x, (row) => row.owner === 'web-client/package-lock.json', (row) => { row.integrity = value('sha512-substituted'); }), (x) => mutateLockRow(x, (row) => row.owner === 'web-client/package-lock.json', (row) => { row.dev = value(true); }), (x) => mutateLockRow(x, (row) => row.owner === 'web-client/package-lock.json', (row) => { row.devOptional = value(true); }), (x) => mutateLockRow(x, (row) => row.owner === 'web-client/package-lock.json', (row) => { row.peer = value(true); }), (x) => mutateLockRow(x, (row) => row.owner === 'web-client/package-lock.json', (row) => { row.inBundle = value(true); }), (x) => mutateLockRow(x, (row) => row.owner === 'web-client/package-lock.json', (row) => { row.resolved = { state: 'null' }; })]) { const malformed = structuredClone(fixture.sbom); mutate(malformed); assert.throws(() => validateSbom(malformed, fixture.inventory, sbomExpected, spdxTestSchema)); }
});

test('lock inventory preserves duplicate locations and exact source, integrity, and npm scope states', () => {
  const npm = { lockfileVersion: 3, packages: { '': { name: 'web-client', version: '0.7.18' }, 'node_modules/dup': { version: '1.0.0', resolved: null, integrity: 'sha512-one', dev: false, devOptional: true, peer: false }, 'node_modules/a/node_modules/dup': { version: '1.0.0', resolved: 'https://example.invalid/dup.tgz', integrity: 'sha512-two', optional: true, inBundle: false, link: false } } };
  const release = { lockfileVersion: 3, packages: { '': { name: '@deos/release-tooling', version: '0.7.18' } } }; const wiki = { lockfileVersion: 3, packages: { '': { name: '@deos/wiki-sync-tooling', version: null } } };
  const locks = new Map([['template/Cargo.lock', Buffer.from('version = 4\n\n[[package]]\nname = "deos-runtime"\nversion = "0.7.18"\nsource = "registry+https://example.invalid/index"\nchecksum = "' + '1'.repeat(64) + '"\n')], ['web-client/package-lock.json', Buffer.from(JSON.stringify(npm))], ['.agents/skills/wiki-sync/package-lock.json', Buffer.from(JSON.stringify(wiki))], ['scripts/release-tooling/package-lock.json', Buffer.from(JSON.stringify(release))]]);
  const inventory = lockInventory(locks); const duplicates = inventory.filter((entry) => entry.name === 'dup'); assert.equal(duplicates.length, 2); assert.notEqual(duplicates[0].location, duplicates[1].location); const direct = duplicates.find((entry) => entry.location === 'node_modules/dup'); const nested = duplicates.find((entry) => entry.location.includes('node_modules/a/')); assert.deepEqual(direct.resolved, { state: 'null' }); assert.deepEqual(direct.devOptional, value(true)); assert.deepEqual(direct.peer, value(false)); assert.deepEqual(nested.optional, value(true)); assert.deepEqual(nested.inBundle, value(false)); assert.deepEqual(inventory.find((entry) => entry.ecosystem === 'cargo').checksum, value('1'.repeat(64))); assert.deepEqual(inventory.find((entry) => entry.owner.includes('wiki')).version, { state: 'null' });
  const malformed = new Map(locks); malformed.set('template/Cargo.lock', Buffer.from('[[package]]\nname = "deos-runtime"\n')); assert.throws(() => lockInventory(malformed), /malformed/);
});

test('DEOS lock SPDX is byte-identical across repository roots, TMPDIR, locale, umask, and cache perturbations', { skip: !process.env.DEOS_SPDX_SCHEMA }, async () => {
  const base = await mkdtemp(path.join(tmpdir(), 'deos-real-sbom-')); const repoA = path.join(base, 'repo-a'); const repoB = path.join(base, 'repo-b'); const outputA = path.join(base, 'absolute-a'); const outputB = path.join(base, 'absolute-b'); const previous = { TMPDIR: process.env.TMPDIR, LANG: process.env.LANG, LC_ALL: process.env.LC_ALL }; const oldUmask = process.umask();
  try {
    const npmLock = (name) => `${JSON.stringify({ name, version: '0.7.18', lockfileVersion: 3, requires: true, packages: { '': { name, version: '0.7.18' } } }, null, 2)}\n`; const locks = new Map([['template/Cargo.lock', 'version = 4\n\n[[package]]\nname = "deos-runtime"\nversion = "0.7.18"\n'], ['web-client/package-lock.json', npmLock('web-client')], ['.agents/skills/wiki-sync/package-lock.json', npmLock('@deos/wiki-sync-tooling')], ['scripts/release-tooling/package-lock.json', npmLock('@deos/release-tooling')]]);
    for (const [member, bytes] of locks) { const file = path.join(repoA, member); await mkdir(path.dirname(file), { recursive: true }); await writeFile(file, bytes); }
    run(repoA, 'git', ['init', '-q']); run(repoA, 'git', ['config', 'user.email', 'test@example.com']); run(repoA, 'git', ['config', 'user.name', 'Test']); run(repoA, 'git', ['add', '.']); run(repoA, 'git', ['commit', '-qm', 'locks']); run(base, 'git', ['clone', '-q', repoA, repoB]); const tag = { version: '0.7.18', commitOid: run(repoA, 'git', ['rev-parse', 'HEAD']), treeOid: run(repoA, 'git', ['rev-parse', 'HEAD^{tree}']) }; const schema = JSON.parse(await readFile(process.env.DEOS_SPDX_SCHEMA));
    await mkdir(outputA); process.env.TMPDIR = path.join(base, 'tmp-a'); process.env.LANG = 'tr_TR.UTF-8'; process.env.LC_ALL = 'tr_TR.UTF-8'; process.umask(0o077); const first = await generateSbom(repoA, tag, 'atmo-network/deos', outputA, schema);
    for (const cache of ['.git/perturbed', 'node_modules/cache', 'template/target/cache', 'output/cache']) { const file = path.join(repoB, cache); await mkdir(path.dirname(file), { recursive: true }); await writeFile(file, path.resolve(file)); }
    await mkdir(outputB); process.env.TMPDIR = path.join(base, 'tmp-b'); process.env.LANG = 'C'; process.env.LC_ALL = 'C'; process.umask(0o022); const second = await generateSbom(repoB, tag, 'atmo-network/deos', outputB, schema); assert.deepEqual(first, second); for (const hostPath of [base, repoA, repoB, outputA, outputB, process.env.TMPDIR]) assert.equal(first.includes(Buffer.from(hostPath)), false, `host path leaked: ${hostPath}`); const document = JSON.parse(first); assert.equal(document.packages.length, 4); assert.deepEqual(document.creationInfo.creators, ['Tool: deos-release-evidence-v1']); assert.ok(document.packages.every((pkg) => pkg.comment.startsWith('DEOS-LOCK-ROW ')));
  } finally { process.umask(oldUmask); for (const [name, value] of Object.entries(previous)) { if (value === undefined) delete process.env[name]; else process.env[name] = value; } await rm(base, { recursive: true, force: true }); }
});

test('descriptor archive bytes, order, and POSIX metadata are deterministic', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'deos-descriptors-')); try { await mkdir(path.join(root, 'dist')); await writeFile(path.join(root, 'z.json'), 'z'); await writeFile(path.join(root, 'dist/a.js'), 'a'); const first = await deterministicDescriptorArchive(root, 1234567890); const second = await deterministicDescriptorArchive(root, 1234567890); assert.deepEqual(first, second); assert.deepEqual([...first.subarray(4, 8)], [0, 0, 0, 0]); const tar = gunzipSync(first); const names = []; for (let offset = 0; offset + 512 <= tar.length && tar[offset] !== 0;) { const header = tar.subarray(offset, offset + 512); const name = header.subarray(0, 100).toString().replace(/\0.*$/, ''); const octal = (start, length) => { const value = header.subarray(start, start + length).toString().replace(/\0.*$/, '').trim(); assert.match(value, /^[0-7]+$/); return [...value].reduce((result, digit) => result * 8 + digit.charCodeAt(0) - 48, 0); }; names.push(name); assert.equal(octal(100, 8), 0o644); assert.equal(octal(108, 8), 0); assert.equal(octal(116, 8), 0); assert.equal(octal(136, 12), 1234567890); offset += 512 + Math.ceil(octal(124, 12) / 512) * 512; } assert.deepEqual(names, ['dist/a.js', 'z.json']); } finally { await rm(root, { recursive: true, force: true }); }
});

async function bundleFixture() {
  const root = await mkdtemp(path.join(tmpdir(), 'deos-bundle-')); const version = '0.7.18'; const assets = []; for (const name of releasePayloadNames(version)) { const bytes = Buffer.from(name); await writeFile(path.join(root, name), bytes); assets.push({ role: 'payload', path: name, bytes: bytes.length, sha256: sha(bytes) }); } const inventory = [...assets, { path: 'release-manifest.json', role: 'self-describing-manifest', digestPolicy: 'intentionally-omitted-to-avoid-self-reference' }, { path: 'SHA256SUMS', role: 'checksum-control', digestPolicy: 'excluded-only-from-itself' }].sort((a, b) => Buffer.compare(Buffer.from(a.path), Buffer.from(b.path))); const manifest = { schema: 'deos-release-manifest/v2', repository: 'atmo-network/deos', repositoryId: '7', tag: { version, ref: `refs/tags/v${version}`, oid, commitOid: oid, treeOid: oid }, workflow: { path: '.github/workflows/release-candidate.yml', runId: '8', runAttempt: '1' }, candidateManifestSha256: sha('candidate'), networkSummarySha256: assets.find((entry) => entry.path.startsWith('network-summary-')).sha256, toolLockSha256: sha('tools'), sbomSha256: assets.find((entry) => entry.path.endsWith('.spdx.json')).sha256, recursionPolicy: { releaseManifest: 'self-listed-without-size-or-digest', sha256sums: 'hashes-all-other-inventory-members-and-excludes-only-itself' }, inventory }; const manifestBytes = Buffer.from(`${canonicalJson(manifest)}\n`); await writeFile(path.join(root, 'release-manifest.json'), manifestBytes); const lines = [...assets.map((entry) => [entry.path, entry.sha256.slice(7)]), ['release-manifest.json', sha256(manifestBytes)]].sort((a, b) => Buffer.compare(Buffer.from(a[0]), Buffer.from(b[0]))).map(([name, digest]) => `${digest}  ${name}`); await writeFile(path.join(root, 'SHA256SUMS'), `${lines.join('\n')}\n`); return root;
}
test('release bundle enforces exact 13-file manifest and checksum-control recursion', async () => {
  const mutations = [null, (root) => writeFile(path.join(root, 'extra'), 'x'), (root) => unlink(path.join(root, 'deos-runtime-v0.7.18.scale')), (root) => writeFile(path.join(root, 'deos-runtime-v0.7.18.scale'), 'changed'), (root) => writeFile(path.join(root, 'SHA256SUMS'), '0'.repeat(64) + '  deos-runtime-v0.7.18.scale\n'), async (root) => { const file = path.join(root, 'release-manifest.json'); const manifest = JSON.parse(await readFile(file)); manifest.inventory = manifest.inventory.filter((entry) => entry.path !== 'SHA256SUMS'); await writeFile(file, `${canonicalJson(manifest)}\n`); }, async (root) => { const file = path.join(root, 'release-manifest.json'); const manifest = JSON.parse(await readFile(file)); manifest.inventory.find((entry) => entry.path === 'release-manifest.json').sha256 = sha('self'); await writeFile(file, `${canonicalJson(manifest)}\n`); }];
  for (const mutate of mutations) { const root = await bundleFixture(); try { if (mutate) await mutate(root); if (mutate) await assert.rejects(() => verifyReleaseBundle(root)); else { const manifest = await verifyReleaseBundle(root); assert.equal(manifest.inventory.length, 13); assert.equal(manifest.inventory.filter((entry) => entry.role === 'payload').length, 11); } } finally { await rm(root, { recursive: true, force: true }); } }
});

test('network summary must match candidate identity, tools, and complete proof order', () => {
  const candidate = { repositoryId: '7', tag: { version: '0.7.18' }, workflow: { runId: '8', runAttempt: '1' } }; const summary = { schema: 'deos-release-network-summary/v1', conclusion: 'success', repositoryId: '7', tag: candidate.tag, workflow: candidate.workflow, candidateManifestSha256: sha('candidate'), wasmSha256: sha('wasm'), chainSpecSha256: sha('chain'), toolLockSha256: sha('tools'), proofLedgerSha256: sha('proof'), proofs: NETWORK_PROOF_ORDER.map((id, index) => ({ schema: 'deos-release-network-proof/v1', sequence: index + 1, id, completedAt: '2026-01-01T00:00:00Z' })) }; validateNetworkSummary(summary, candidate, sha('candidate'), sha('tools')); for (const mutate of [(x) => { x.workflow.runAttempt = '2'; }, (x) => { x.candidateManifestSha256 = sha('other'); }, (x) => { x.proofs.pop(); }]) { const changed = structuredClone(summary); mutate(changed); assert.throws(() => validateNetworkSummary(changed, candidate, sha('candidate'), sha('tools')), /mismatch|proof/); }
});

function yamlSubjectPaths(workflow) {
  const lines = workflow.split('\n'); const marker = lines.findIndex((line) => line.trim() === 'subject-path: |'); assert.notEqual(marker, -1, 'subject-path block'); const indentation = lines[marker].search(/\S/); const paths = [];
  for (const line of lines.slice(marker + 1)) { if (!line.trim()) continue; if (line.search(/\S/) <= indentation) break; const prefix = '${{ env.RELEASE_DIR }}/'; const value = line.trim(); assert.ok(value.startsWith(prefix), `non-authoritative subject path: ${value}`); paths.push(value.slice(prefix.length)); }
  return paths;
}
test('every artifact consumer installs its directly owned validator before importing release evidence', async () => {
  const workflow = await readFile(new URL('../.github/workflows/release-candidate.yml', import.meta.url), 'utf8');
  const network = workflow.slice(workflow.indexOf('  network:'), workflow.indexOf('  package-and-attest:'));
  const packaging = workflow.slice(workflow.indexOf('  package-and-attest:'));
  for (const [label, job] of [['network', network], ['package', packaging]]) {
    const install = job.indexOf('npm ci --ignore-scripts --prefix scripts/release-tooling');
    const consume = job.indexOf('node scripts/github-release-artifact.mjs');
    assert.ok(install >= 0 && consume > install, `${label} validator installation order`);
  }
});

test('final workflow has exact immutable provenance dependency, permissions, subjects, and no release mutation', async () => {
  const workflow = await readFile(new URL('../.github/workflows/release-candidate.yml', import.meta.url), 'utf8'); assert.doesNotMatch(workflow, /runner\.temp/, 'job-level env cannot use the runner context'); const packaging = workflow.slice(workflow.indexOf('  package-and-attest:')); assert.match(packaging, /needs: \[full, network\]/); assert.doesNotMatch(packaging, /03-build-runtime|export-papi-metadata|generate-(actors|ingress|observation)|network-assurance-local/); assert.match(workflow, /actions\/attest-build-provenance@e8998f949152b193b063cb0ec769d69d929409be/); assert.doesNotMatch(workflow, /attest-build-provenance@(v|main|master)/); assert.doesNotMatch(workflow, /contents:\s*write|gh release|create-release|softprops\/action-gh-release/); assert.match(workflow, /package-and-attest:[\s\S]*actions: read[\s\S]*attestations: write[\s\S]*contents: read[\s\S]*id-token: write/); assert.match(workflow, /expected-artifact-id "\$\{\{ needs\.network\.outputs\.network-artifact-id \}\}"[\s\S]*expected-artifact-digest "\$\{\{ needs\.network\.outputs\.network-artifact-digest \}\}"/); assert.match(workflow, /retention-days: 90/); const subjects = yamlSubjectPaths(workflow); const expected = releaseInventoryNames('0.7.18'); assert.equal(subjects.length, 13); assert.equal(new Set(subjects).size, 13); assert.deepEqual([...subjects].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b))), expected); assert.ok(subjects.every((entry) => !entry.includes('*')));
});
