#!/usr/bin/env node

import { appendFileSync, constants as fsConstants, lstatSync, readFileSync, readlinkSync, realpathSync } from 'node:fs';
import { mkdir, open, readFile, readdir, rename, rm, lstat, writeFile } from 'node:fs/promises';
import { createHash, randomBytes } from 'node:crypto';
import { hostname, platform, arch, homedir, tmpdir } from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { spawn, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const EVIDENCE_SCHEMA = 'deos-validation-evidence/v2';
const ENVIRONMENT_SCHEMA = 'deos-validation-environment/v2';
const AUTHORITY_SCHEMA = 'validation-authority/v2';
const REPETITION_SCHEMA = 'deos-validation-repetition/v2';
const BOUNDARY_SCHEMA = 'deos-validation-boundary/v1';
const LOCK_SCHEMA = 'deos-validation-lock/v2';
const FRESHNESS = Object.freeze({ policy: 'immutable-only/v1', inputs: [] });
const CANONICAL_ENTRYPOINT = 'scripts/validate-local.sh';
const CANONICAL_AUTHORITY = 'scripts/validation-authority.v1.json';
const LOCK_WAIT_MS = 30 * 60 * 1000;
const LOCK_PROGRESS_MS = 5_000;
const TOOL_AUTHORITY_PATHS = Object.freeze([
  'template/Cargo.lock', 'template/rust-toolchain.toml', 'web-client/package-lock.json',
  'web-client/package.json', '.agents/skills/wiki-sync/package-lock.json',
  'scripts/release-tooling/package.json', 'scripts/release-tooling/package-lock.json',
]);
const FORBIDDEN_ENV_EXACT = Object.freeze([
  'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTDOCFLAGS', 'RUSTC_WRAPPER',
  'RUSTC_WORKSPACE_WRAPPER', 'RUSTUP_TOOLCHAIN', 'RUSTUP_HOME', 'CARGO_HOME',
  'CARGO_TARGET_DIR', 'NODE_OPTIONS', 'NODE_PATH', 'BASH_ENV', 'ENV',
  'CC', 'CXX', 'CFLAGS', 'CXXFLAGS', 'CPPFLAGS', 'LDFLAGS', 'AR',
  'SOURCE_DATE_EPOCH', 'CARGO', 'RUSTC', 'RUSTDOC', 'RUSTUP', 'NODE', 'NPM',
  'NPM_CONFIG_USERCONFIG', 'npm_config_userconfig', 'NPM_CONFIG_GLOBALCONFIG',
  'npm_config_globalconfig',
]);
const FORBIDDEN_ENV_PREFIXES = Object.freeze([
  'CARGO_', 'RUST_', 'RUSTC_', 'RUSTDOC_', 'RUSTUP_', 'NODE_', 'NPM_',
  'NPM_CONFIG_', 'npm_config_', 'GIT_CONFIG_KEY_', 'GIT_CONFIG_VALUE_',
]);
const SEMANTIC_ENVIRONMENT = Object.freeze({
  fast: Object.freeze({ SKIP_WASM_BUILD: '1', CARGO_INCREMENTAL: '0', CARGO_PROFILE: 'release', INCLUDE_OCCUPANCY_PROFILE: '1', QUICK_MODE: '0', AUDIT_SCOPE: null, RUN_SIMULATOR: null, RUN_CARGO_CHECK: null, RUN_RUNTIME_TESTS: null, LC_ALL: 'C', TZ: 'UTC' }),
  heavy: Object.freeze({ SKIP_WASM_BUILD: '1', CARGO_INCREMENTAL: '0', CARGO_PROFILE: 'release', INCLUDE_OCCUPANCY_PROFILE: '1', QUICK_MODE: '0', AUDIT_SCOPE: 'all', RUN_SIMULATOR: '1', RUN_CARGO_CHECK: '1', RUN_RUNTIME_TESTS: '1', LC_ALL: 'C', TZ: 'UTC' }),
  full: Object.freeze({ SKIP_WASM_BUILD: '0', CARGO_INCREMENTAL: '0', CARGO_PROFILE: 'release', INCLUDE_OCCUPANCY_PROFILE: '1', QUICK_MODE: '0', AUDIT_SCOPE: 'all', RUN_SIMULATOR: '1', RUN_CARGO_CHECK: '1', RUN_RUNTIME_TESTS: '1', LC_ALL: 'C', TZ: 'UTC' }),
});
const ALLOWED_SEMANTIC_ENV = new Set(Object.keys(SEMANTIC_ENVIRONMENT.full));
const ACTORS_BOUNDARIES = Object.freeze([
  'actors.semantic-manifest.baseline-generate',
  'actors.semantic-manifest.current-generate',
  'actors.semantic-manifest.exact-compare',
  'actors.golden-equivalence.baseline-reactive-corpus',
  'actors.golden-equivalence.current-reactive-corpus',
  'actors.golden-equivalence.baseline-semantic-anchor-family',
  'actors.golden-equivalence.current-semantic-anchor-family',
  'actors.scheduler.scheduler_stress_fifo_over_capacity_fairness_matrix',
  'actors.scheduler.scheduler_stress_fifo_dense_vs_sparse_topology_matrix',
  'actors.scheduler.scheduler_stress_fifo_sparse_topology_long_run_liveness',
  'actors.scheduler.stress_10k_actors_queue_scheduler',
  'actors.scheduler.checkpoint_a_s6_dense_10k_wakeups_converge_without_drops',
  'actors.scheduler.profile_scheduler_queue_wakeup_occupancy_10k',
]);
const EXPECTED_BOUNDARIES = Object.freeze({
  fast: Object.freeze([]),
  heavy: ACTORS_BOUNDARIES,
  full: Object.freeze([
    'full.regeneration.canonical-pass-1', 'full.tracked-zero-drift.pass-1',
    ...ACTORS_BOUNDARIES,
    'full.regeneration.canonical-pass-2', 'full.tracked-zero-drift.pass-2',
    'full.ignored-and-generated-artifacts.exact-sha256-compare',
  ]),
});

function fail(message) { throw new Error(message); }
function unsignedSafeInteger(value, label) {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value)) fail(`${label} must be a canonical unsigned integer`);
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed)) fail(`${label} exceeds the safe integer range`);
  return parsed;
}
export function canonicalJson(value) {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return JSON.stringify(value);
  if (typeof value === 'number') { if (!Number.isFinite(value)) fail('Canonical JSON rejects non-finite numbers'); return JSON.stringify(value); }
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (typeof value === 'object') return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`;
  fail(`Canonical JSON rejects ${typeof value}`);
}
function sha256(value) { return createHash('sha256').update(value).digest('hex'); }
function isString(value) { return typeof value === 'string' && value.length > 0; }
function isSha256(value) { return typeof value === 'string' && /^sha256:[0-9a-f]{64}$/.test(value); }
function exactKeys(value, keys, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) fail(`${label} must be an object`);
  if (canonicalJson(Object.keys(value).sort()) !== canonicalJson([...keys].sort())) fail(`${label} fields must be exactly: ${keys.join(', ')}`);
}
function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: options.cwd, env: options.env ?? process.env, encoding: options.encoding ?? 'utf8', maxBuffer: 256 * 1024 * 1024 });
  if (result.error) throw result.error;
  if (result.status !== 0) fail(`${command} ${args.join(' ')} failed${String(result.stderr || result.stdout || '').trim() ? `: ${String(result.stderr || result.stdout).trim()}` : ''}`);
  return result.stdout;
}
function git(repo, args, options = {}) { return run('git', ['-C', repo, ...args], options); }
function gitObject(repo, spec) {
  const result = spawnSync('git', ['-C', repo, 'show', spec], { encoding: 'buffer', maxBuffer: 256 * 1024 * 1024 });
  if (result.error) throw result.error;
  if (result.status !== 0) fail(`Candidate-tree object is unavailable: ${spec}`);
  return result.stdout;
}
function normalizeRepoPath(value, label) {
  if (!isString(value) || value.includes('\\') || path.posix.isAbsolute(value)) fail(`${label} must be a repository-relative POSIX path`);
  const normalized = path.posix.normalize(value);
  if (normalized === '.' || normalized === '..' || normalized.startsWith('../') || normalized !== value) fail(`${label} escapes or is not normalized: ${value}`);
  return normalized;
}

export function semanticEnvironment(profile) {
  if (!SEMANTIC_ENVIRONMENT[profile]) fail(`Unsupported validation profile: ${profile}`);
  return { ...SEMANTIC_ENVIRONMENT[profile] };
}
export function repetitionContract(profile) {
  const ids = EXPECTED_BOUNDARIES[profile];
  if (!ids) fail(`Unsupported validation profile: ${profile}`);
  const contract = { schema: REPETITION_SCHEMA, profile, indivisibleWholeProfile: true, expectedBoundaryIds: [...ids] };
  return { contract, contractSha256: `sha256:${sha256(canonicalJson(contract))}` };
}
export function invocationIdentity(profile) {
  return { entrypoint: CANONICAL_ENTRYPOINT, profile, argv: [profile], semanticEnvironment: semanticEnvironment(profile) };
}
export function evidenceKey(fields) {
  return `sha256:${sha256(canonicalJson({ candidate: fields.candidate, authority: fields.authority, environment: fields.environment, refIdentity: fields.refIdentity, freshness: fields.freshness, invocation: fields.invocation, repetition: fields.repetition }))}`;
}
export function candidateIdentity(repo) {
  const treeOid = git(repo, ['rev-parse', 'HEAD^{tree}']).trim();
  const indexTreeOid = git(repo, ['write-tree']).trim();
  const tracked = spawnSync('git', ['-C', repo, 'diff', '--no-ext-diff', '--quiet']);
  const staged = spawnSync('git', ['-C', repo, 'diff', '--cached', '--no-ext-diff', '--quiet']);
  if (tracked.error || staged.error) throw tracked.error ?? staged.error;
  if (![0, 1].includes(tracked.status) || ![0, 1].includes(staged.status)) fail('Unable to determine candidate state');
  const untracked = git(repo, ['ls-files', '--others', '--exclude-standard', '-z']).split('\0').filter(Boolean);
  const trackedClean = tracked.status === 0;
  const stagedClean = staged.status === 0;
  return { treeOid, indexTreeOid, trackedClean, stagedClean, untracked, clean: trackedClean && stagedClean && untracked.length === 0 && indexTreeOid === treeOid };
}
function validateAuthorityManifest(manifest) {
  exactKeys(manifest, ['schema', 'roots', 'fullArtifactOutputs', 'immutableRefInputs'], 'Authority manifest');
  if (manifest.schema !== AUTHORITY_SCHEMA) fail(`Unsupported authority schema: ${manifest.schema}`);
  if (!Array.isArray(manifest.roots) || manifest.roots.length === 0) fail('Authority roots must be non-empty');
  const roots = manifest.roots.map((entry) => normalizeRepoPath(entry, 'Authority root'));
  if (new Set(roots).size !== roots.length) fail('Authority roots must be unique');
  if (!Array.isArray(manifest.fullArtifactOutputs) || !Array.isArray(manifest.immutableRefInputs)) fail('Authority artifact/ref inputs must be arrays');
  for (const output of manifest.fullArtifactOutputs) {
    exactKeys(output, ['path', 'kind', 'requiredMembers'], 'Full artifact output');
    normalizeRepoPath(output.path, 'Full artifact path');
    if (!['file', 'directory'].includes(output.kind) || !Array.isArray(output.requiredMembers)) fail('Full artifact output kind/members are invalid');
    for (const member of output.requiredMembers) normalizeRepoPath(member, 'Full artifact required member');
  }
  for (const input of manifest.immutableRefInputs) {
    exactKeys(input, ['profiles', 'name', 'commit'], 'Immutable ref input');
    if (!Array.isArray(input.profiles) || !input.profiles.every((p) => ['fast', 'heavy', 'full'].includes(p)) || !isString(input.name) || !/^[0-9a-f]{40}([0-9a-f]{24})?$/.test(input.commit)) fail('Immutable ref input is invalid');
  }
  return manifest;
}
export function readAuthorityManifestFromTree(repo, treeOid, manifestPath = CANONICAL_AUTHORITY) {
  normalizeRepoPath(manifestPath, 'Authority manifest path');
  return validateAuthorityManifest(JSON.parse(gitObject(repo, `${treeOid}:${manifestPath}`).toString('utf8')));
}
function parseLsTree(buffer) {
  const records = [];
  for (const entry of buffer.toString('utf8').split('\0')) {
    if (!entry) continue;
    const match = /^(\d+) ([^ ]+) ([0-9a-f]+)\t(.+)$/.exec(entry);
    if (!match) fail(`Unexpected git ls-tree record: ${entry}`);
    records.push({ mode: match[1], type: match[2], oid: match[3], path: match[4] });
  }
  return records;
}
export async function authorityIdentity(repo, treeOid, manifestPath = CANONICAL_AUTHORITY) {
  const manifest = readAuthorityManifestFromTree(repo, treeOid, manifestPath);
  for (const root of manifest.roots) {
    const exists = spawnSync('git', ['-C', repo, 'cat-file', '-e', `${treeOid}:${root}`]);
    if (exists.status !== 0) fail(`Authority root is absent from candidate tree: ${root}`);
  }
  const literalRoots = manifest.roots.map((root) => `:(literal)${root}`);
  const result = spawnSync('git', ['-C', repo, 'ls-tree', '-rz', '-r', '--full-tree', treeOid, '--', ...literalRoots], { encoding: 'buffer', maxBuffer: 256 * 1024 * 1024 });
  if (result.error) throw result.error;
  if (result.status !== 0) fail(`Unable to enumerate validation authority: ${String(result.stderr)}`);
  const records = parseLsTree(result.stdout).sort((a, b) => Buffer.compare(Buffer.from(a.path), Buffer.from(b.path)));
  if (records.length === 0) fail('Authority roots resolve to no candidate-tree blobs');
  const hash = createHash('sha256');
  for (const record of records) hash.update(record.mode).update('\0').update(record.type).update('\0').update(record.oid).update('\0').update(record.path).update('\0');
  return { identity: { schema: manifest.schema, sha256: `sha256:${hash.digest('hex')}` }, manifest };
}
function commandOutput(command, args, cwd, env) { return run(command, args, { cwd, env }).trim(); }
function commandVersion(command, args, cwd, env) { return commandOutput(command, args, cwd, env).split('\n')[0]; }
function candidateBlobSha256(repo, treeOid, relativePath) {
  const result = spawnSync('git', ['-C', repo, 'show', `${treeOid}:${relativePath}`], { encoding: 'buffer' });
  return result.status === 0 ? `sha256:${sha256(result.stdout)}` : null;
}
function isWithin(root, target) {
  const relative = path.relative(root, target);
  return relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative));
}
function candidateTreeEntry(repo, treeOid, absolute, candidateRoot) {
  if (!isWithin(candidateRoot, absolute)) return null;
  const relative = path.relative(candidateRoot, absolute).split(path.sep).join('/');
  if (!relative || relative.startsWith('../')) return null;
  const result = spawnSync('git', ['-C', repo, 'ls-tree', '-z', treeOid, '--', `:(literal)${relative}`], { encoding: 'buffer' });
  if (result.error) throw result.error;
  if (result.status !== 0) fail(`Unable to inspect candidate-tree configuration path: ${relative}`);
  const entries = parseLsTree(result.stdout);
  return entries.length === 1 && entries[0].path === relative ? entries[0] : null;
}
function treeOwnsRegularFile(repo, treeOid, absolute, candidateRoot, info, resolved) {
  if (!info.isFile() || resolved !== absolute) return false;
  const entry = candidateTreeEntry(repo, treeOid, absolute, candidateRoot);
  if (!entry || entry.type !== 'blob' || !['100644', '100755'].includes(entry.mode)) return false;
  let bytes; let verifiedBytes; let verifiedInfo; let verifiedResolved;
  try {
    bytes = readFileSync(absolute);
    verifiedInfo = lstatSync(absolute);
    verifiedResolved = realpathSync(absolute);
    verifiedBytes = readFileSync(absolute);
  } catch (error) { fail(`Unable to read and verify candidate-tree configuration ${absolute}: ${error.message}`); }
  const stableMetadata = info.dev === verifiedInfo.dev && info.ino === verifiedInfo.ino && info.mode === verifiedInfo.mode && info.size === verifiedInfo.size && info.mtimeMs === verifiedInfo.mtimeMs;
  if (!stableMetadata || resolved !== verifiedResolved || !bytes.equals(verifiedBytes)) fail(`Candidate-tree configuration drifted while being read: ${absolute}`);
  const treeBytes = gitObject(repo, `${treeOid}:${entry.path}`);
  const executable = (info.mode & 0o111) !== 0;
  return bytes.equals(treeBytes) && executable === (entry.mode === '100755');
}
function externalFileIdentity(filePath, repo, treeOid, candidateRoot) {
  const absolute = path.resolve(filePath);
  let info;
  try { info = lstatSync(absolute); } catch (error) {
    if (error.code === 'ENOENT' || error.code === 'ENOTDIR') return null;
    fail(`Unable to inspect external configuration ${absolute}: ${error.message}`);
  }
  let resolved;
  try { resolved = realpathSync(absolute); } catch (error) { fail(`Unable to resolve external configuration ${absolute}: ${error.message}`); }
  if (treeOwnsRegularFile(repo, treeOid, absolute, candidateRoot, info, resolved)) return null;
  if (!info.isFile() && !info.isSymbolicLink()) fail(`External configuration is not a regular file or symlink: ${filePath}`);
  let bytes; let verifiedBytes; let verifiedInfo; let verifiedResolved; let linkTarget = null; let verifiedLinkTarget = null;
  try {
    if (info.isSymbolicLink()) linkTarget = readlinkSync(absolute);
    bytes = readFileSync(absolute);
    verifiedInfo = lstatSync(absolute);
    verifiedResolved = realpathSync(absolute);
    if (verifiedInfo.isSymbolicLink()) verifiedLinkTarget = readlinkSync(absolute);
    verifiedBytes = readFileSync(absolute);
  } catch (error) { fail(`Unable to read and verify external configuration ${absolute}: ${error.message}`); }
  const stableMetadata = info.dev === verifiedInfo.dev && info.ino === verifiedInfo.ino && info.mode === verifiedInfo.mode && info.size === verifiedInfo.size && info.mtimeMs === verifiedInfo.mtimeMs;
  if (!stableMetadata || resolved !== verifiedResolved || linkTarget !== verifiedLinkTarget || !bytes.equals(verifiedBytes)) fail(`External configuration drifted while being read: ${absolute}`);
  return { path: absolute, exists: true, resolvedPath: resolved, type: info.isSymbolicLink() ? 'symlink' : 'file', mode: info.mode & 0o7777, linkTarget, sha256: `sha256:${sha256(bytes)}`, bytes: bytes.length };
}
function ancestorConfigCandidates(cwd, directoryName, names) {
  const result = [];
  let current = path.resolve(cwd);
  while (true) {
    for (const name of names) result.push(path.join(current, directoryName, name));
    const parent = path.dirname(current);
    if (parent === current) break;
    current = parent;
  }
  return result;
}
function npmConfigPaths(cwd, env) {
  const output = commandOutput('npm', ['config', 'get', 'userconfig', 'globalconfig', '--json'], cwd, env);
  const values = {};
  for (const line of output.split('\n').filter(Boolean)) {
    const separator = line.indexOf('=');
    if (separator <= 0) fail(`Unable to parse effective npm configuration path: ${line}`);
    values[line.slice(0, separator)] = line.slice(separator + 1);
  }
  if (!isString(values.userconfig) || !isString(values.globalconfig)) fail('Effective npm user/global configuration paths are unavailable');
  return values;
}
function externalConfigurationIdentity(repo, treeOid, commandEnv) {
  let candidateRoot;
  try { candidateRoot = realpathSync(repo); } catch (error) { fail(`Unable to resolve candidate root: ${error.message}`); }
  const temporaryRoot = path.resolve(commandEnv.TMPDIR || tmpdir());
  const cargoHomePath = path.join(homedir(), '.cargo');
  let resolvedCargoHome;
  try { resolvedCargoHome = realpathSync(cargoHomePath); } catch (error) { fail(`Unable to resolve Cargo home ${cargoHomePath}: ${error.message}`); }
  const cargoCandidates = [
    ...ancestorConfigCandidates(path.join(repo, 'template'), '.cargo', ['config', 'config.toml']),
    ...ancestorConfigCandidates('/tmp/deos-runtime-production-source/template', '.cargo', ['config', 'config.toml']),
    ...ancestorConfigCandidates(temporaryRoot, '.cargo', ['config', 'config.toml']),
    path.join(resolvedCargoHome, 'config'), path.join(resolvedCargoHome, 'config.toml'),
  ];
  const npmCwds = [repo, path.join(repo, 'web-client'), path.join(repo, '.agents/skills/wiki-sync')];
  const effectiveNpm = npmCwds.map((cwd) => {
    const config = npmConfigPaths(cwd, commandEnv);
    return { cwd: path.relative(repo, cwd) || '.', userconfig: path.resolve(cwd, config.userconfig), globalconfig: path.resolve(cwd, config.globalconfig) };
  });
  const npmCandidates = npmCwds.flatMap((cwd) => ancestorConfigCandidates(cwd, '', ['.npmrc']));
  for (const config of effectiveNpm) npmCandidates.push(config.userconfig, config.globalconfig);
  const snapshot = (candidates) => [...new Set(candidates.map((entry) => path.resolve(entry)))].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b))).map((entry) => externalFileIdentity(entry, repo, treeOid, candidateRoot)).filter(Boolean);
  return {
    cargoHome: { path: cargoHomePath, resolvedPath: resolvedCargoHome },
    cargoConfigs: snapshot(cargoCandidates),
    effectiveNpm,
    npmConfigs: snapshot(npmCandidates),
  };
}
function canonicalRepository(repo) {
  try { return realpathSync(path.resolve(repo)); }
  catch (error) { fail(`Unable to resolve candidate repository: ${error.message}`); }
}
function canonicalValidationEnvironment(repo, env) {
  return { ...env, DEOS_PROJECT_ROOT: repo, DEOS_BINARY_DIR: path.join(repo, 'bin') };
}
function rejectInheritedControls(env) {
  if (env.HOME && path.resolve(env.HOME) !== homedir()) fail('Unsupported inherited validation control HOME');
  for (const [name, value] of Object.entries(env)) {
    if (ALLOWED_SEMANTIC_ENV.has(name)) continue;
    if (FORBIDDEN_ENV_EXACT.includes(name) || name === 'GIT_CONFIG_COUNT' || FORBIDDEN_ENV_PREFIXES.some((prefix) => name.startsWith(prefix))) {
      if (value !== undefined && value !== '') fail(`Unsupported inherited validation control ${name}`);
    }
  }
}
export function environmentIdentity(repo, treeOid, overrides = {}, commandEnv = process.env) {
  rejectInheritedControls(commandEnv);
  const template = path.join(repo, 'template');
  let libc = 'not-linux';
  if (platform() === 'linux') {
    try { libc = commandVersion('getconf', ['GNU_LIBC_VERSION'], repo, commandEnv); }
    catch { libc = commandVersion('ldd', ['--version'], repo, commandEnv); }
  }
  const values = {
    os: overrides.os ?? platform(), architecture: overrides.architecture ?? arch(), libc: overrides.libc ?? libc,
    git: overrides.git ?? commandVersion('git', ['--version'], repo, commandEnv), bash: overrides.bash ?? commandVersion('bash', ['--version'], repo, commandEnv),
    python: overrides.python ?? commandVersion('python3', ['--version'], repo, commandEnv), node: overrides.node ?? commandVersion('node', ['--version'], repo, commandEnv), npm: overrides.npm ?? commandVersion('npm', ['--version'], repo, commandEnv),
    rustc: overrides.rustc ?? commandVersion('rustc', ['--version'], template, commandEnv), cargo: overrides.cargo ?? commandVersion('cargo', ['--version'], template, commandEnv), rustup: overrides.rustup ?? commandVersion('rustup', ['--version'], template, commandEnv),
    activeRustToolchain: overrides.activeRustToolchain ?? commandVersion('rustup', ['show', 'active-toolchain'], template, commandEnv).split(/\s+/)[0],
    rustHostTarget: overrides.rustHostTarget ?? commandOutput('rustc', ['-vV'], template, commandEnv).split('\n').find((line) => line.startsWith('host: '))?.slice(6) ?? fail('rustc host target missing'),
    githubRunner: { imageOS: overrides.imageOS ?? commandEnv.ImageOS ?? null, imageVersion: overrides.imageVersion ?? commandEnv.ImageVersion ?? null },
    externalConfiguration: externalConfigurationIdentity(repo, treeOid, commandEnv),
    authorityDigests: Object.fromEntries(TOOL_AUTHORITY_PATHS.map((entry) => [entry, candidateBlobSha256(repo, treeOid, entry)])),
  };
  return { schema: ENVIRONMENT_SCHEMA, sha256: `sha256:${sha256(canonicalJson(values))}`, values };
}
function refIdentity(repo, manifest, profile) {
  const inputs = manifest.immutableRefInputs.filter((input) => input.profiles.includes(profile)).map((input) => ({ name: input.name, commit: input.commit }));
  for (const input of inputs) {
    const result = spawnSync('git', ['-C', repo, 'cat-file', '-e', `${input.commit}^{commit}`]);
    if (result.status !== 0) fail(`Immutable validation commit is unavailable: ${input.name}=${input.commit}`);
  }
  return { policy: 'exact-immutable-commits/v1', inputs };
}
export function assembleIdentity({ candidate, authority, environment, profile, ref = { policy: 'exact-immutable-commits/v1', inputs: [] } }) {
  const repetition = repetitionContract(profile);
  const fields = { candidate, authority, environment, refIdentity: ref, freshness: { ...FRESHNESS, inputs: [] }, invocation: invocationIdentity(profile), repetition: { contractSha256: repetition.contractSha256 } };
  return { ...fields, key: evidenceKey(fields) };
}

function validateBoundaryReport(contents, profile, nonce) {
  const expected = EXPECTED_BOUNDARIES[profile];
  const lines = contents.split('\n').filter(Boolean);
  const actual = lines.map((line, index) => {
    let item;
    try { item = JSON.parse(line); } catch { fail(`Boundary report line ${index + 1} is not JSON`); }
    exactKeys(item, ['schema', 'nonce', 'sequence', 'id'], 'Boundary report item');
    if (item.schema !== BOUNDARY_SCHEMA || item.nonce !== nonce || item.sequence !== index + 1 || !isString(item.id)) fail(`Boundary report line ${index + 1} is invalid`);
    return item.id;
  });
  if (canonicalJson(actual) !== canonicalJson(expected)) fail(`Boundary report mismatch; expected ${expected.join(', ') || '(none)'}, received ${actual.join(', ') || '(none)'}`);
  return actual;
}
function validateRecord(record, expectedKey = null) {
  exactKeys(record, ['schema', 'key', 'candidate', 'authority', 'environment', 'refIdentity', 'freshness', 'invocation', 'repetition', 'artifacts', 'conclusion', 'startedAt', 'completedAt'], 'Evidence record');
  if (record.schema !== EVIDENCE_SCHEMA || !isSha256(record.key) || record.conclusion !== 'success') fail('Evidence schema/key/conclusion is invalid');
  exactKeys(record.candidate, ['treeOid', 'indexTreeOid', 'trackedClean', 'stagedClean', 'untracked', 'clean'], 'Record candidate');
  if (!/^[0-9a-f]{40}([0-9a-f]{24})?$/.test(record.candidate.treeOid) || record.candidate.indexTreeOid !== record.candidate.treeOid || record.candidate.trackedClean !== true || record.candidate.stagedClean !== true || !Array.isArray(record.candidate.untracked) || record.candidate.untracked.length !== 0 || record.candidate.clean !== true) fail('Record candidate is invalid');
  if (record.authority?.schema !== AUTHORITY_SCHEMA || !isSha256(record.authority.sha256)) fail('Record authority is invalid');
  if (record.environment?.schema !== ENVIRONMENT_SCHEMA || `sha256:${sha256(canonicalJson(record.environment.values))}` !== record.environment.sha256) fail('Record environment is invalid');
  if (canonicalJson(record.invocation) !== canonicalJson(invocationIdentity(record.invocation?.profile))) fail('Record invocation is invalid');
  if (record.refIdentity?.policy !== 'exact-immutable-commits/v1' || !Array.isArray(record.refIdentity.inputs)) fail('Record ref identity is invalid');
  for (const input of record.refIdentity.inputs) {
    exactKeys(input, ['name', 'commit'], 'Record immutable ref input');
    if (!isString(input.name) || !/^[0-9a-f]{40}([0-9a-f]{24})?$/.test(input.commit)) fail('Record immutable ref input is invalid');
  }
  if (canonicalJson(record.freshness) !== canonicalJson(FRESHNESS)) fail('Record freshness is invalid');
  const repetition = repetitionContract(record.invocation.profile);
  if (canonicalJson(record.repetition) !== canonicalJson({ contractSha256: repetition.contractSha256 })) fail('Record repetition contract is invalid');
  if (record.invocation.profile === 'full') validateArtifactManifest(record.artifacts); else if (record.artifacts !== null) fail('Non-full record must not contain artifacts');
  if (!Number.isFinite(Date.parse(record.startedAt)) || !Number.isFinite(Date.parse(record.completedAt))) fail('Record timestamps are invalid');
  if (record.key !== evidenceKey(record)) fail('Evidence key does not match semantic fields');
  if (expectedKey && record.key !== expectedKey) fail('Evidence record key does not match requested key');
  return record;
}
export { validateRecord };

export async function writeRecordAtomic(recordPath, record, options = {}) {
  const directory = path.dirname(recordPath);
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const temporary = path.join(directory, `.${path.basename(recordPath)}.${process.pid}.${Date.now()}.tmp`);
  const handle = await open(temporary, fsConstants.O_WRONLY | fsConstants.O_CREAT | fsConstants.O_EXCL, 0o600);
  try { await handle.writeFile(`${JSON.stringify(record, null, 2)}\n`); await handle.sync(); } finally { await handle.close(); }
  if (options.beforeRename) await options.beforeRename(temporary);
  await rename(temporary, recordPath);
  const directoryHandle = await open(directory, fsConstants.O_RDONLY);
  try { await directoryHandle.sync(); } finally { await directoryHandle.close(); }
}
export async function readValidRecord(recordPath, expectedKey) {
  try {
    const record = validateRecord(JSON.parse(await readFile(recordPath, 'utf8')), expectedKey);
    const gate = process.env.DEOS_VALIDATION_TEST_RECORD_READ_GATE;
    if (process.env.DEOS_VALIDATION_TEST_MODE === '1' && gate) {
      await writeFile(`${gate}.ready`, '', { flag: 'wx' });
      while (true) {
        try { await lstat(`${gate}.release`); break; } catch (error) { if (error.code !== 'ENOENT') throw error; }
        await new Promise((resolve) => setTimeout(resolve, 10));
      }
    }
    return record;
  } catch { return null; }
}
export async function gitCommonDirectory(repo) { return path.resolve(repo, git(repo, ['rev-parse', '--git-common-dir']).trim()); }
function processStartIdentity(pid) {
  try {
    if (platform() === 'linux') { const contents = readFileSync(`/proc/${pid}/stat`, 'utf8'); return contents.slice(contents.lastIndexOf(')') + 2).split(' ')[19] ?? null; }
    const result = spawnSync('ps', ['-o', 'lstart=', '-p', String(pid)], { encoding: 'utf8' });
    return result.status === 0 && result.stdout.trim() ? result.stdout.trim() : null;
  } catch { return null; }
}
async function writeJsonAtomic(target, value) {
  const temporary = `${target}.${process.pid}.${randomBytes(6).toString('hex')}.tmp`;
  const handle = await open(temporary, fsConstants.O_WRONLY | fsConstants.O_CREAT | fsConstants.O_EXCL, 0o600);
  try { await handle.writeFile(`${JSON.stringify(value)}\n`); await handle.sync(); } finally { await handle.close(); }
  await rename(temporary, target);
}
async function classifyLock(lockPath) {
  let owner;
  try { owner = JSON.parse(await readFile(path.join(lockPath, 'owner.json'), 'utf8')); } catch { return { state: 'initializing' }; }
  if (owner?.schema !== LOCK_SCHEMA || !isString(owner.token) || !isString(owner.hostname) || !Number.isInteger(owner.pid) || !isString(owner.processStartIdentity)) return { state: 'unverifiable', owner };
  if (owner.hostname !== hostname()) return { state: 'remote', owner };
  const current = processStartIdentity(owner.pid);
  if (current === null) { try { process.kill(owner.pid, 0); return { state: 'unverifiable', owner }; } catch (error) { return { state: error.code === 'ESRCH' ? 'dead' : 'unverifiable', owner }; } }
  return { state: current === owner.processStartIdentity ? 'live' : 'dead', owner };
}
async function quarantineDeadLock(lockPath, observedOwner) {
  const quarantine = `${lockPath}.stale.${observedOwner.token}.${randomBytes(6).toString('hex')}`;
  try { await rename(lockPath, quarantine); } catch (error) { if (error.code === 'ENOENT') return false; throw error; }
  const displaced = await classifyLock(quarantine);
  if (displaced.owner?.token !== observedOwner.token) {
    try { await rename(quarantine, lockPath); } catch { /* Preserve the quarantined live replacement; never delete it. */ }
    fail(`Validation lock changed owner during atomic stale-lock displacement; preserved at ${quarantine}`);
  }
  await rm(quarantine, { recursive: true, force: true });
  return true;
}
async function acquireLock(lockPath, key, waitMs = LOCK_WAIT_MS) {
  await mkdir(path.dirname(lockPath), { recursive: true, mode: 0o700 });
  const started = Date.now(); let lastProgress = 0;
  while (true) {
    const owner = { schema: LOCK_SCHEMA, token: randomBytes(16).toString('hex'), hostname: hostname(), pid: process.pid, processStartIdentity: processStartIdentity(process.pid), key, acquiredAt: new Date().toISOString() };
    if (!owner.processStartIdentity) fail('Unable to verify this process identity');
    try {
      await mkdir(lockPath, { mode: 0o700 });
      await writeJsonAtomic(path.join(lockPath, 'owner.json'), owner);
      return {
        owner,
        async updateKey(nextKey) { owner.key = nextKey; await writeJsonAtomic(path.join(lockPath, 'owner.json'), owner); },
        async release() { const current = await classifyLock(lockPath); if (current.owner?.token === owner.token) await quarantineDeadLock(lockPath, owner); },
      };
    } catch (error) { if (error.code !== 'EEXIST') throw error; }
    const existing = await classifyLock(lockPath);
    if (existing.state === 'dead') {
      const delay = process.env.DEOS_VALIDATION_TEST_MODE === '1'
        ? unsignedSafeInteger(process.env.DEOS_VALIDATION_TEST_RECLAIM_DELAY_MS ?? '0', 'DEOS_VALIDATION_TEST_RECLAIM_DELAY_MS')
        : 0;
      if (delay > 0) await new Promise((resolve) => setTimeout(resolve, delay));
      await quarantineDeadLock(lockPath, existing.owner);
      continue;
    }
    if (['remote', 'unverifiable'].includes(existing.state)) fail(`Validation lock is ${existing.state}; refusing recovery: ${lockPath}`);
    if (existing.state === 'initializing' && Date.now() - started > 1_000) fail(`Validation lock owner is partial: ${lockPath}`);
    if (Date.now() - started >= waitMs) fail(`Timed out waiting for validation lock: ${lockPath}`);
    if (Date.now() - lastProgress >= LOCK_PROGRESS_MS) { console.error(`[INFO] Waiting for validation lock: ${lockPath}`); lastProgress = Date.now(); }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}
function childEnvironment(repo, profile, reportPath, nonce) {
  rejectInheritedControls(process.env);
  const expected = semanticEnvironment(profile);
  const env = { ...canonicalValidationEnvironment(repo, process.env), DEOS_VALIDATION_INTERNAL: '1', DEOS_VALIDATION_BOUNDARY_REPORT: reportPath, DEOS_VALIDATION_BOUNDARY_NONCE: nonce };
  for (const [name, value] of Object.entries(expected)) {
    if (Object.hasOwn(process.env, name) && process.env[name] !== value) fail(`Unsupported semantic override ${name}`);
    if (value === null) delete env[name]; else env[name] = value;
  }
  return env;
}
async function runChild(command, args, cwd, env, lifecycle) {
  lifecycle.active = true;
  const childEnv = canonicalValidationEnvironment(cwd, env);
  await new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, env: childEnv, stdio: 'inherit' });
    let receivedSignal = null;
    const forward = (signal) => { receivedSignal = signal; child.kill(signal); };
    for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) process.once(signal, forward);
    child.once('error', reject);
    child.once('exit', (code, signal) => {
      for (const name of ['SIGINT', 'SIGTERM', 'SIGHUP']) process.removeListener(name, forward);
      lifecycle.active = false;
      if (receivedSignal || signal) reject(new Error(`Canonical validation child terminated by ${receivedSignal ?? signal}`));
      else if (code !== 0) reject(new Error(`Canonical validation child failed with status ${code}`));
      else resolve();
    });
  });
}
export async function calculateIdentity(repo, profile, env = process.env) {
  const before = candidateIdentity(repo);
  if (!before.clean) fail('Reusable validation candidate became dirty');
  const { identity: authority, manifest } = await authorityIdentity(repo, before.treeOid);
  const environment = environmentIdentity(repo, before.treeOid, {}, env);
  const identity = assembleIdentity({ candidate: before, authority, environment, profile, ref: refIdentity(repo, manifest, profile) });
  const after = candidateIdentity(repo);
  if (!after.clean || canonicalJson(before) !== canonicalJson(after)) fail('Candidate changed while calculating validation identity');
  return identity;
}
async function collectFiles(root, output) {
  const absolute = path.join(root, output.path);
  let info;
  try { info = await lstat(absolute); } catch { fail(`Required full artifact is missing: ${output.path}`); }
  if (output.kind === 'file') {
    if (!info.isFile() || info.size === 0) fail(`Required full artifact is missing or empty: ${output.path}`);
    return [output.path];
  }
  if (!info.isDirectory()) fail(`Required full artifact directory is missing: ${output.path}`);
  for (const member of output.requiredMembers) {
    const memberPath = path.posix.join(output.path, member);
    let memberInfo;
    try { memberInfo = await lstat(path.join(root, memberPath)); } catch { fail(`Required full artifact member is missing: ${memberPath}`); }
    if (!memberInfo.isFile() || memberInfo.size === 0) fail(`Required full artifact member is empty: ${memberPath}`);
  }
  const files = [];
  async function walk(relative) {
    for (const entry of (await readdir(path.join(root, relative), { withFileTypes: true })).sort((a, b) => Buffer.compare(Buffer.from(a.name), Buffer.from(b.name)))) {
      const child = path.posix.join(relative, entry.name);
      if (entry.isDirectory()) await walk(child); else if (entry.isFile()) files.push(child); else fail(`Unsupported artifact member type: ${child}`);
    }
  }
  await walk(output.path);
  if (files.length === 0) fail(`Required full artifact directory is empty: ${output.path}`);
  return files;
}
function validateArtifactManifest(value) {
  exactKeys(value, ['schema', 'entries'], 'Artifact manifest');
  if (value.schema !== 'deos-validation-full-artifacts/v2' || !Array.isArray(value.entries) || value.entries.length === 0) fail('Artifact manifest is invalid or empty');
  for (const entry of value.entries) {
    exactKeys(entry, ['path', 'sha256', 'bytes'], 'Artifact entry');
    normalizeRepoPath(entry.path, 'Artifact entry path');
    if (!isSha256(entry.sha256) || !Number.isSafeInteger(entry.bytes) || entry.bytes <= 0) fail('Artifact entry is invalid or empty');
  }
}
export async function artifactManifest(repo, treeOid = candidateIdentity(repo).treeOid) {
  const authority = readAuthorityManifestFromTree(repo, treeOid);
  const files = [];
  for (const output of authority.fullArtifactOutputs) files.push(...await collectFiles(repo, output));
  const unique = [...new Set(files)].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  const entries = [];
  for (const relativePath of unique) { const bytes = await readFile(path.join(repo, relativePath)); entries.push({ path: relativePath, sha256: `sha256:${sha256(bytes)}`, bytes: bytes.length }); }
  const result = { schema: 'deos-validation-full-artifacts/v2', entries };
  validateArtifactManifest(result);
  return result;
}
async function executeWholeProfile(options) {
  const repo = canonicalRepository(options.repo);
  const canonicalEntrypoint = path.join(repo, CANONICAL_ENTRYPOINT);
  const initial = candidateIdentity(repo);
  const ci = Boolean(process.env.CI && !['0', 'false'].includes(process.env.CI));
  if (!initial.clean && (options.profile === 'full' || ci)) fail(`${ci ? 'CI validation' : 'full'} requires a clean candidate`);
  const common = await gitCommonDirectory(repo);
  const root = path.join(common, 'deos-validation', 'v2');
  const lock = await acquireLock(path.join(root, 'lock'), `pending:${options.profile}`, options.lockWaitMs);
  const reportPath = path.join(root, `boundary.${process.pid}.${randomBytes(8).toString('hex')}.jsonl`);
  const nonce = randomBytes(24).toString('hex');
  let held = true;
  let terminating = false;
  const childLifecycle = { active: false };
  const release = async () => { if (held) { held = false; await lock.release(); } };
  const signalCleanup = (signal) => {
    if (terminating) return;
    terminating = true;
    if (childLifecycle.active) return;
    void (async () => {
      await rm(reportPath, { force: true });
      await release();
      process.exit(128 + ({ SIGHUP: 1, SIGINT: 2, SIGTERM: 15 }[signal] ?? 1));
    })();
  };
  for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) process.once(signal, signalCleanup);
  try {
    await writeFile(reportPath, '', { mode: 0o600, flag: 'wx' });
    const env = childEnvironment(repo, options.profile, reportPath, nonce);
    if (!initial.clean) {
      console.error(`[WARNING] ${options.profile} is non-reusable dirty validation`);
      await runChild(canonicalEntrypoint, ['--internal-prepare', options.profile], repo, env, childLifecycle);
      await runChild(canonicalEntrypoint, ['--internal-run', options.profile], repo, env, childLifecycle);
      validateBoundaryReport(await readFile(reportPath, 'utf8'), options.profile, nonce);
      return { outcome: 'executed-dirty', key: null };
    }
    if (canonicalJson(candidateIdentity(repo)) !== canonicalJson(initial)) fail('Candidate changed while waiting for lock');
    await runChild(canonicalEntrypoint, ['--internal-prepare', options.profile], repo, env, childLifecycle);
    const pre = await calculateIdentity(repo, options.profile, env);
    if (canonicalJson(pre.candidate) !== canonicalJson(initial)) fail('Candidate changed during preparation');
    await lock.updateKey(pre.key);
    const records = path.join(root, 'records');
    const recordPath = path.join(records, `${pre.key.slice(7)}.json`);
    if (options.cacheEnabled) {
      await mkdir(records, { recursive: true, mode: 0o700 });
      for (const entry of await readdir(records)) if (entry.endsWith('.tmp')) await rm(path.join(records, entry), { force: true });
    }
    if (options.cacheEnabled && !options.fresh) {
      const record = await readValidRecord(recordPath, pre.key);
      if (record) {
        let reusable = true;
        if (options.profile === 'full') {
          try { reusable = canonicalJson(await artifactManifest(repo, pre.candidate.treeOid)) === canonicalJson(record.artifacts); }
          catch { reusable = false; }
          if (!reusable) await rm(recordPath, { force: true });
        }
        if (reusable) {
          const finalLookupIdentity = await calculateIdentity(repo, options.profile, env);
          if (canonicalJson(finalLookupIdentity) !== canonicalJson(pre)) fail('Validation identity changed during cache lookup');
          console.error(`[SUCCESS] Reused ${options.profile} validation evidence ${pre.key}`);
          return { outcome: 'reused', key: pre.key, recordPath };
        }
      }
    }
    const startedAt = new Date().toISOString();
    await runChild(canonicalEntrypoint, ['--internal-run', options.profile], repo, env, childLifecycle);
    const completedBoundaries = validateBoundaryReport(await readFile(reportPath, 'utf8'), options.profile, nonce);
    const post = await calculateIdentity(repo, options.profile, env);
    if (canonicalJson(pre) !== canonicalJson(post)) fail('Validation identity changed between pre/post snapshots');
    const artifacts = options.profile === 'full' ? await artifactManifest(repo, post.candidate.treeOid) : null;
    if (options.cacheEnabled) {
      const record = { schema: EVIDENCE_SCHEMA, key: pre.key, candidate: pre.candidate, authority: pre.authority, environment: pre.environment, refIdentity: pre.refIdentity, freshness: pre.freshness, invocation: pre.invocation, repetition: pre.repetition, artifacts, conclusion: 'success', startedAt, completedAt: new Date().toISOString() };
      validateRecord(record, pre.key);
      await writeRecordAtomic(recordPath, record);
      console.error(`[SUCCESS] Stored ${options.profile} validation evidence ${pre.key} after ${completedBoundaries.length} required boundaries`);
      return { outcome: 'executed-recorded', key: pre.key, recordPath };
    }
    return { outcome: 'executed-uncached', key: pre.key };
  } finally {
    for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) process.removeListener(signal, signalCleanup);
    await rm(reportPath, { force: true });
    await release();
  }
}
function appendBoundary(id) {
  const report = process.env.DEOS_VALIDATION_BOUNDARY_REPORT;
  const nonce = process.env.DEOS_VALIDATION_BOUNDARY_NONCE;
  if (process.env.DEOS_VALIDATION_INTERNAL !== '1' || !isString(report) || !isString(nonce) || !isString(id)) fail('Boundary reporting is private to canonical internal validation');
  const directory = path.dirname(report);
  const reportInfo = readFileSync(report, 'utf8');
  const sequence = reportInfo.split('\n').filter(Boolean).length + 1;
  appendFileSync(report, `${JSON.stringify({ schema: BOUNDARY_SCHEMA, nonce, sequence, id })}\n`, { encoding: 'utf8', flag: 'a', mode: 0o600 });
}
async function parseRunArgs(args) {
  const options = { repo: '.', profile: null, fresh: false, cacheEnabled: true, lockWaitMs: LOCK_WAIT_MS };
  while (args.length) {
    const arg = args.shift();
    if (arg === '--repo') options.repo = args.shift();
    else if (arg === '--profile') options.profile = args.shift();
    else if (arg === '--fresh') options.fresh = true;
    else if (arg === '--lock-wait-ms' && process.env.DEOS_VALIDATION_TEST_MODE === '1') options.lockWaitMs = Number(args.shift());
    else fail(`Unknown run argument: ${arg}`);
  }
  if (!SEMANTIC_ENVIRONMENT[options.profile]) fail('run requires --profile fast|heavy|full');
  const cache = process.env.DEOS_VALIDATION_CACHE ?? '1';
  if (!['0', '1'].includes(cache)) fail('DEOS_VALIDATION_CACHE must be exactly 0 or 1');
  options.cacheEnabled = cache === '1';
  return options;
}
function usage() { console.log('Usage: validation-evidence.mjs run --profile fast|heavy|full [--repo PATH] [--fresh]\n       validation-evidence.mjs boundary ID\n       validation-evidence.mjs artifact-manifest --repo PATH --output FILE\n       validation-evidence.mjs compare-artifacts FIRST SECOND'); }
async function main(argv) {
  const command = argv.shift();
  if (command === 'run') { const result = await executeWholeProfile(await parseRunArgs(argv)); if (process.env.DEOS_VALIDATION_RESULT_JSON === '1') console.log(JSON.stringify(result)); return; }
  if (command === 'boundary') { if (argv.length !== 1) fail('boundary requires exactly one ID'); appendBoundary(argv[0]); return; }
  if (command === 'artifact-manifest') {
    let repo = '.'; let output;
    while (argv.length) { const arg = argv.shift(); if (arg === '--repo') repo = argv.shift(); else if (arg === '--output') output = argv.shift(); else fail(`Unknown artifact argument: ${arg}`); }
    if (!output) fail('artifact-manifest requires --output');
    await writeFile(output, `${JSON.stringify(await artifactManifest(path.resolve(repo)), null, 2)}\n`, { mode: 0o600 }); return;
  }
  if (command === 'compare-artifacts') { if (argv.length !== 2) fail('compare-artifacts requires two paths'); if (canonicalJson(JSON.parse(await readFile(argv[0], 'utf8'))) !== canonicalJson(JSON.parse(await readFile(argv[1], 'utf8')))) fail('Full artifact manifests differ'); return; }
  if (['-h', '--help', undefined].includes(command)) { usage(); return; }
  fail(`Unknown command: ${command}`);
}
const isEntrypoint = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isEntrypoint) main(process.argv.slice(2)).catch((error) => { console.error(`[ERROR] ${error.message}`); process.exitCode = 1; });
