#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { lstat, mkdtemp, readdir, readFile, readlink, realpath, rm, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { homedir, tmpdir, userInfo } from 'node:os';
import { dirname, join, relative, resolve } from 'node:path';

// Source exclusions require independently reviewed advisory prerequisites, not just
// membership in a native graph. The ledger/reference retain their exact limits.
const reachabilityClasses = new Set([
  'lockfile-only',
  'native-observability-only',
  'trusted-generation-only',
  'source-prerequisite-excluded',
  'source-backport-reviewed',
  'root-normal-build-absent',
]);

const registrySource = 'registry+https://github.com/rust-lang/crates.io-index';
const rootRole = {
  role: 'root-workspace-normal-build', features: 'all', target: 'all', edges: 'normal,build',
};
const rootTreeArgs = ['+1.94.1', 'tree', '--locked', '--offline', '--workspace', '--all-features',
  '--target', 'all', '--edges', 'normal,build', '--prefix', 'depth', '--format', '{p}|{f}'];
const sha256 = value => createHash('sha256').update(value).digest('hex');
const canonical = value => JSON.stringify(value, (_, item) => item && typeof item === 'object' && !Array.isArray(item)
  ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a.localeCompare(b))) : item);
function validateRootProof(proof) {
  assert.ok(proof && typeof proof === 'object', 'root absence requires rootNormalBuildProof');
  assert.deepEqual(Object.keys(proof).sort(), [...Object.keys(rootRole), 'inputs', 'treeSha256'].sort(),
    'unknown or missing root proof setting');
  for (const [key, value] of Object.entries(rootRole)) assert.equal(proof[key], value, `unsupported root role ${key}`);
  assert.match(proof.treeSha256, /^[a-f0-9]{64}$/, 'invalid root tree hash');
  assert.ok(Array.isArray(proof.inputs), 'root proof inputs required');
  assert.deepEqual(proof.inputs.map(file => file.path).sort(),
    ['template/Cargo.lock', 'template/Cargo.toml', 'template/rust-toolchain.toml'],
    'root proof requires exact unique source inputs');
  for (const file of proof.inputs) {
    assert.deepEqual(Object.keys(file).sort(), ['path', 'sha256'], 'unknown root input setting');
    assert.match(file.sha256, /^[a-f0-9]{64}$/, 'invalid root input hash');
  }
}
function assertRootAbsence(entries, cargoFindings, metadata, tree, proof, root) {
  assert.equal(sha256(tree.replaceAll(root, '$REPO')), proof.treeSha256, 'root tree/parent feature identity drift');
  const lines = tree.trimEnd().split('\n').filter(line => line !== '');
  assert.ok(lines.length > 0 && lines.every(line => /^\d+[^ |]+ v[^ |]+(?: .*?)?\|[^|]*$/.test(line)),
    'unsupported or empty generated Cargo tree');
  for (const entry of entries) {
    assert.equal(entry.ecosystem, 'cargo', 'root absence requires Cargo');
    assert.equal(entry.reachability, 'root-normal-build-absent', 'conflicting root absence classification');
    const pkg = entry.absentFromRootNormalBuild;
    assert.ok(pkg, 'root absence requires package identity');
    assert.deepEqual(Object.keys(pkg).sort(), ['name', 'source', 'version'], 'unknown root package setting');
    const affected = cargoFindings.filter(f => f.advisory.id === entry.id);
    assert.ok(affected.length > 0, 'stale root absence decision');
    assert.ok(affected.every(f => f.package.name === pkg.name && f.package.version === pkg.version &&
      f.package.source === pkg.source), 'root absence finding source identity mismatch');
    const matches = metadata.packages.filter(p => p.name === pkg.name && p.version === pkg.version);
    assert.equal(matches.length, 1, 'root absence package identity mismatch');
    assert.equal(matches[0].source, pkg.source, 'root absence metadata source identity mismatch');
    assert.ok(!lines.some(line => {
      const match = /^\d+([^ |]+) v([^ |]+)/.exec(line);
      return match[1] === pkg.name && match[2] === pkg.version;
    }), `excluded root package activated: ${pkg.name}@${pkg.version}`);
  }
}
async function enforceRootAbsence(entries, proof, cargoFindings, metadata, root, lockPath, lock, externalWorkspace) {
  validateRootProof(proof);
  assert.equal(externalWorkspace, false, 'root absence cannot transfer to external workspace');
  root = await realpath(root);
  const workspace = await realpath(join(root, 'template'));
  assert.equal(await realpath(lockPath), join(workspace, 'Cargo.lock'), 'root absence lock role mismatch');
  assert.equal(metadata.workspace_root, workspace, 'root absence metadata role mismatch');
  for (const file of proof.inputs) {
    assert.equal(sha256(await readFile(join(root, file.path))), file.sha256, `root proof input drift: ${file.path}`);
  }
  assert.equal(await readFile(lockPath, 'utf8'), lock, 'root lock changed during review');
  const temporary = await mkdtemp(join(tmpdir(), 'deos-root-role.'));
  try {
    const run = args => {
      const result = spawnSync('cargo', args, { cwd: workspace, encoding: 'utf8', maxBuffer: 128 * 1024 * 1024,
        env: { ...process.env, CARGO_TARGET_DIR: temporary, CARGO_TERM_COLOR: 'never' } });
      assert.equal(result.status, 0, `current root Cargo proof failed: ${result.error ?? result.stderr}`);
      return result.stdout;
    };
    // Fresh Cargo selection, never normal/build adjacency in dev-unified metadata.
    // Neither command compiles code or executes a node; both fail offline/locked.
    const current = JSON.parse(run(['+1.94.1', 'metadata', '--locked', '--offline', '--all-features', '--format-version', '1']));
    const graph = value => ({ packages: value.packages, workspace_members: value.workspace_members,
      resolve: value.resolve, workspace_root: value.workspace_root });
    assert.equal(canonical(graph(current)), canonical(graph(metadata)), 'stale full metadata/source graph');
    assertRootAbsence(entries, cargoFindings, current, run(rootTreeArgs), proof, root);
    for (const file of proof.inputs) {
      assert.equal(sha256(await readFile(join(root, file.path))), file.sha256, 'root proof input changed during review');
    }
    console.log('[INFO] Current root workspace all-feature/all-target normal/build absence enforced; no dev/test, SDK, binary or launch clearance');
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

// This is a source review prerequisite, not an interlock on arbitrary builds or
// launches. Only the explicitly selected Native role may consume these decisions.
const nativeRole = {
  role: 'native-omni-source-normal-build', purpose: 'source-only',
  package: 'polkadot-omni-node', features: 'runtime-benchmarks', edges: 'normal,build',
  target: 'x86_64-unknown-linux-gnu', toolchain: '1.94.1',
  profile: 'production: opt3, unwind, fat LTO, codegen-units 1',
};
const nativeInputs = ['build-environment.json', 'build-tools.txt', 'candidate-closure.json',
  'candidate-metadata.json', 'prepared.json', 'sdk/Cargo.lock', 'sdk/Cargo.toml', 'sdk/.cargo/config.toml'];
const nativeProducers = ['scripts/build-corrected-node.sh', 'scripts/build-corrected-node.mjs', 'scripts/_common.sh'];
function exactKeys(value, keys, label) {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value), `${label} required`);
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), `unknown or missing ${label} setting`);
}
async function regularOwnerFile(root, filePath) {
  assert.ok(typeof filePath === 'string' && filePath.length > 0, 'Native file path required');
  const path = resolve(root, filePath);
  assert.equal(relative(root, path), filePath, 'Native file must be canonical owner-relative');
  assert.ok(!filePath.startsWith('../'), 'Native file outside owner');
  const stat = await lstat(path);
  assert.ok(stat.isFile() && !stat.isSymbolicLink(), 'Native source symlink/path substitution or nonregular file');
  assert.equal(await realpath(path), path, 'Native source symlink/path substitution');
  return path;
}
async function boundFile(root, file) {
  exactKeys(file, ['path', 'sha256'], 'Native file');
  assert.match(file.sha256, /^[a-f0-9]{64}$/, 'invalid Native source digest');
  const path = await regularOwnerFile(root, file.path);
  assert.equal(sha256(await readFile(path)), file.sha256, `Native source hash drift: ${file.path}`);
}
async function sourceInventory(root, prefix = 'src') {
  const path = join(root, prefix);
  const stat = await lstat(path);
  assert.ok(!stat.isSymbolicLink(), 'Native inventory symlink substitution');
  if (stat.isFile()) return [prefix];
  assert.ok(stat.isDirectory(), 'unsupported Native source file type');
  return (await Promise.all((await readdir(path)).sort().map(name => sourceInventory(root, `${prefix}/${name}`)))).flat();
}
// Reviewed SDK configuration is policy input, not something a new snapshot can bless.
const nativeCargoConfigSha256 = 'b19bf3b1b8fe09e7af28738a31dac7584490c7e97457b6372363351fc87f49f5';
const nativeHome = userInfo().homedir;
function checkNativeEnvironment(env = process.env) {
  const ordinaryCargo = /^(CARGO_BUILD_JOBS|CARGO_TARGET_DIR|CARGO_NET_OFFLINE|CARGO_TERM_[A-Z_]+|CARGO_HOME)$/;
  for (const key of Object.keys(env)) {
    assert.ok(!(key.startsWith('CARGO_') && !ordinaryCargo.test(key)) &&
      !/^(RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|RUSTDOCFLAGS|CARGO_ENCODED_RUSTDOCFLAGS|RUSTC|RUSTDOC|RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|RUSTC_BOOTSTRAP)$/.test(key),
    `unsupported Native compiler/configuration override: ${key}`);
  }
  assert.equal(env.HOME ?? nativeHome, nativeHome, 'unsupported Native HOME');
  // Cargo resolves relative homes in its child cwd, not the verifier's cwd.
  assert.equal(env.CARGO_HOME ?? join(nativeHome, '.cargo'), join(nativeHome, '.cargo'), 'unsupported Native CARGO_HOME');
  assert.equal(env.RUSTUP_HOME ?? join(nativeHome, '.rustup'), join(nativeHome, '.rustup'), 'unsupported Native RUSTUP_HOME');
  if (env.RUSTUP_TOOLCHAIN !== undefined) assert.equal(env.RUSTUP_TOOLCHAIN, '1.94.1', 'unsupported Native RUSTUP_TOOLCHAIN');
}
async function checkNativeConfiguration(workspace, cargoHome = join(nativeHome, '.cargo')) {
  checkNativeEnvironment();
  const reviewed = join(workspace, '.cargo', 'config.toml');
  const paths = new Set();
  for (let cwd = workspace; ; cwd = dirname(cwd)) {
    for (const name of ['config', 'config.toml']) paths.add(join(cwd, '.cargo', name));
    if (dirname(cwd) === cwd) break;
  }
  for (const name of ['config', 'config.toml']) paths.add(join(cargoHome, name));
  for (const path of paths) {
    const stat = await lstat(path).catch(error => { if (error.code === 'ENOENT') return null; throw error; });
    if (!stat) { assert.notEqual(path, reviewed, 'Native reviewed Cargo config missing'); continue; }
    assert.equal(path, reviewed, `unreviewed Native Cargo configuration: ${path}`);
    assert.ok(stat.isFile() && !stat.isSymbolicLink(), 'Native Cargo config substitution');
    assert.equal(await realpath(path), path, 'Native Cargo config path substitution');
    assert.equal(sha256(await readFile(path)), nativeCargoConfigSha256, 'Native reviewed Cargo config drift');
  }
}
function nativeInstanceBindings(proof, affected) {
  const optional = Object.hasOwn(proof ?? {}, 'absentAffectedPackages') ? ['absentAffectedPackages'] : [];
  exactKeys(proof, [...Object.keys(nativeRole), 'workspaceRoot', 'treeSha256', 'inputs', 'producers',
    'advisories', 'packages', 'affectedPackages', ...optional], 'Native proof');
  const { affectedPackages, absentAffectedPackages = [], ...bindings } = proof;
  assert.ok(Array.isArray(affectedPackages) && affectedPackages.length > 0, 'Native affected packages required');
  assert.ok(Array.isArray(absentAffectedPackages), 'Native absent affected packages required');
  for (const pkg of [...affectedPackages, ...absentAffectedPackages]) {
    exactKeys(pkg, ['name', 'version', 'source'], 'Native affected package');
    assert.ok(typeof pkg.name === 'string' && pkg.name.length > 0 &&
      typeof pkg.version === 'string' && pkg.version.length > 0 &&
      (pkg.source === null || (typeof pkg.source === 'string' && pkg.source.length > 0)),
    'invalid Native affected package identity');
  }
  assert.ok(affected.length > 0, 'stale Native source decision');
  const actual = [...new Set(affected.map(f => canonical({ name: f.package.name,
    version: f.package.version, source: f.package.source })))].sort();
  // Sorting without deduplication rejects duplicates, overlaps and unlisted instances.
  assert.deepEqual([...affectedPackages, ...absentAffectedPackages].map(p => canonical(p)).sort(), actual,
    'Native advisory package/version/source coverage mismatch');
  return bindings;
}
function assertNativeAbsentInstances(packages, metadata, selected) {
  for (const pkg of packages) {
    const matches = metadata.packages.filter(p => p.name === pkg.name && p.version === pkg.version);
    assert.equal(matches.length, 1, 'Native absent package metadata identity mismatch');
    assert.equal(matches[0].source, pkg.source, 'Native absent package metadata source mismatch');
    assert.ok(!selected.has(matches[0].id), 'Native absent affected package activated in rooted normal/build selection');
  }
}
async function enforceNativeSources(entries, cargoFindings, metadata, repoRoot, lockPath, lock, externalWorkspace) {
  assert.equal(externalWorkspace, true, 'Native source decisions require external workspace');
  checkNativeEnvironment();
  const proof = entries[0].sourceCallerNative;
  const entryBindings = entries.map(entry => nativeInstanceBindings(entry.sourceCallerNative,
    cargoFindings.filter(f => f.advisory.id === entry.id)));
  for (const [key, value] of Object.entries(nativeRole)) assert.equal(proof[key], value, `unsupported Native ${key}`);
  assert.equal(process.platform, 'linux', 'Native source role requires Linux');
  assert.equal(process.arch, 'x64', 'Native source role requires x86_64');
  assert.match(proof.treeSha256, /^[a-f0-9]{64}$/, 'invalid Native tree digest');
  const workspace = await realpath(proof.workspaceRoot);
  assert.equal(proof.workspaceRoot, workspace, 'Native workspace path substitution');
  assert.equal(await realpath(lockPath), join(workspace, 'Cargo.lock'), 'Native lock role mismatch');
  assert.equal(metadata.workspace_root, workspace, 'Native metadata role mismatch');
  const candidate = dirname(workspace);
  const repo = await realpath(repoRoot);
  assert.deepEqual(proof.inputs.map(f => f.path).sort(), [...nativeInputs].sort(), 'Native exact input coverage required');
  assert.deepEqual(proof.producers.map(f => f.path).sort(), [...nativeProducers].sort(), 'Native producer coverage required');
  const checkInputs = async () => {
    await checkNativeConfiguration(workspace);
    assert.equal(proof.inputs.find(f => f.path === 'sdk/.cargo/config.toml').sha256,
      nativeCargoConfigSha256, 'Native Cargo config policy identity mismatch');
    for (const file of proof.inputs) await boundFile(candidate, file);
    for (const file of proof.producers) await boundFile(repo, file);
    for (const file of proof.advisories) {
      assert.match(file.path, /^crates\/[a-z0-9-]+\/RUSTSEC-\d{4}-\d{4}\.md$/, 'invalid Native advisory owner');
      await boundFile(join(process.env.CARGO_HOME || join(homedir(), '.cargo'), 'advisory-db'), file);
    }
  };
  await checkInputs();
  const prepared = await readJson(join(candidate, 'prepared.json'));
  assert.equal(prepared.toolchain, proof.toolchain, 'Native prepared toolchain drift');
  assert.equal(prepared.profile, proof.profile, 'Native prepared profile drift');
  assert.deepEqual(prepared.features, [proof.features], 'Native prepared feature drift');
  assert.equal(prepared.candidateLockSha256, sha256(lock), 'Native prepared lock drift');
  assert.deepEqual([...prepared.scripts].sort(), proof.producers.map(f => [join(repo, f.path), f.sha256]).sort(), 'Native producer identity drift');
  const environment = await readJson(join(candidate, 'build-environment.json'));
  assert.deepEqual(environment.globalCargoConfigs, [], 'unsupported Native recorded global Cargo configuration');
  assert.ok(!environment.environment?.CARGO_HOME ||
    environment.environment.CARGO_HOME === join(nativeHome, '.cargo'), 'unsupported Native recorded CARGO_HOME');
  // Use the fixed installed toolchain directly, not PATH proxies or inherited selection.
  const toolchainBin = join(nativeHome, '.rustup', 'toolchains', '1.94.1-x86_64-unknown-linux-gnu', 'bin');
  const rustc = join(toolchainBin, 'rustc');
  for (const executable of ['cargo', 'rustc']) {
    const path = join(toolchainBin, executable);
    assert.equal(await realpath(path), path, 'Native fixed toolchain path substitution');
  }
  const compiler = spawnSync(rustc, ['-vV'], { encoding: 'utf8' });
  assert.equal(compiler.status, 0, 'Native fixed compiler unavailable');
  assert.match(compiler.stdout, /^release: 1\.94\.1$/m, 'Native compiler release mismatch');
  assert.match(compiler.stdout, /^host: x86_64-unknown-linux-gnu$/m, 'Native compiler host mismatch');
  const temporary = await mkdtemp(join(tmpdir(), 'deos-native-source.'));
  try {
    const run = args => {
      const result = spawnSync(join(toolchainBin, 'cargo'), args, { cwd: workspace, encoding: 'utf8', maxBuffer: 128 * 1024 * 1024,
        env: { ...process.env, RUSTC: rustc, CARGO_TARGET_DIR: temporary, CARGO_TERM_COLOR: 'never', CARGO_NET_OFFLINE: 'true',
          GIT_ALLOW_PROTOCOL: '', GIT_TERMINAL_PROMPT: '0' } });
      assert.equal(result.status, 0, `current Native Cargo proof failed: ${result.error ?? result.stderr}`);
      return result.stdout;
    };
    const current = JSON.parse(run(['metadata', '--locked', '--offline', '--features',
      'polkadot-omni-node/runtime-benchmarks', '--format-version', '1']));
    const graph = value => ({ packages: value.packages, workspace_members: value.workspace_members,
      resolve: value.resolve, workspace_root: value.workspace_root });
    const recorded = await readJson(join(candidate, 'candidate-metadata.json'));
    assert.equal(canonical(graph(current)), canonical(graph(recorded)), 'Native prepared metadata/source graph drift');
    assert.equal(canonical(graph(current)), canonical(graph(metadata)), 'Native supplied metadata/source graph drift');
    const tree = run(['tree', '--locked', '--offline', '-p', proof.package, '--features', proof.features,
      '--target', proof.target, '--edges', proof.edges, '--prefix', 'depth', '--format', '{p}|{f}']);
    assert.equal(sha256(tree.replaceAll(candidate, '$NATIVE')), proof.treeSha256, 'Native rooted parent/source/feature drift');
    const selected = new Set();
    for (const line of tree.trimEnd().split('\n')) {
      const match = /^(\d+)([^\s]+) v(\S+)(?: \(proc-macro\))?(?: \(([^)]+)\))?\|[^|]*$/.exec(line);
      assert.ok(match, 'unsupported Native tree');
      const [, , name, version, source] = match;
      const matches = current.packages.filter(p => p.name === name && p.version === version &&
        (source?.startsWith('/') ? dirname(p.manifest_path) === source : p.source === registrySource));
      assert.equal(matches.length, 1, 'ambiguous Native selected source');
      selected.add(matches[0].id);
    }
    const checkPackages = async () => {
      const identities = new Set();
      assert.ok(Array.isArray(proof.packages) && proof.packages.length > 0, 'Native caller packages required');
      for (const review of proof.packages) {
        exactKeys(review, ['name', 'version', 'source', 'completeSrc', 'files'], 'Native package');
        const key = `${review.name}@${review.version}`;
        assert.ok(!identities.has(key), 'duplicate Native caller package');
        identities.add(key);
        const matches = current.packages.filter(p => p.name === review.name && p.version === review.version && p.source === review.source);
        assert.equal(matches.length, 1, 'Native caller source identity mismatch');
        assert.ok(selected.has(matches[0].id), 'Native caller not in rooted normal/build selection');
        assert.equal(await realpath(matches[0].manifest_path), matches[0].manifest_path, 'Native package path substitution');
        const root = dirname(matches[0].manifest_path);
        const paths = review.files.map(f => f.path);
        assert.equal(new Set(paths).size, paths.length, 'duplicate Native caller file');
        assert.ok(paths.includes('Cargo.toml') && paths.length > 1, 'Native caller manifest/source required');
        assert.equal(typeof review.completeSrc, 'boolean', 'Native inventory policy required');
        if (review.completeSrc) assert.deepEqual(paths.filter(p => p.startsWith('src/')).sort(),
          (await sourceInventory(root)).sort(), `Native source inventory drift: ${key}`);
        for (const file of review.files) await boundFile(root, file);
      }
    };
    await checkPackages();
    for (const [index, entry] of entries.entries()) {
      const { affectedPackages, absentAffectedPackages = [] } = entry.sourceCallerNative;
      assert.equal(canonical(entryBindings[index]), canonical(entryBindings[0]), 'conflicting Native source proofs');
      assert.equal(entry.ecosystem, 'cargo', 'Native source review requires Cargo');
      assert.equal(entry.reachability, 'source-prerequisite-excluded', 'conflicting Native classification');
      assertNativeAbsentInstances(absentAffectedPackages, current, selected);
      assert.ok(proof.advisories.some(f => f.path.endsWith(`/${entry.id}.md`)), 'Native advisory binding missing');
      for (const pkg of affectedPackages) {
        assert.ok(proof.packages.some(p => p.name === pkg.name && p.version === pkg.version &&
          p.source === pkg.source), 'Native finding package/version/source not reviewed');
      }
    }
    assert.equal(await readFile(lockPath, 'utf8'), lock, 'Native lock changed during review');
    await checkInputs();
    await checkPackages();
    console.log('[INFO] Current Native Omni Linux host normal/build SOURCE prerequisites enforced; no binary, C-library, Root or launch clearance');
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

// A separate source-preparation case: one shared, fixed identity, never a
// caller-selected hash or the historical prepared/build receipt contract.
// This proposal binds reviewed sources; final producer refresh and independent
// implementation/policy admission remain required before installation.
const nativePreparationProofSha256 = 'e0861ad5b0b2ea4b3362b092ff3bea3bab86feddba283b89a279b53eaa01b9e5';
const nativePreparationStage = 'SOURCEPREPARATION';
const nativePreparationProofFiles = ['git-refs.txt', 'source-tools.txt', 'composed-metadata.json',
  'composed-tree.txt', 'composed-closure.json', 'source-consumers.json'];
function checkNativePreparationEnvironment(env = process.env) {
  checkNativeEnvironment(env);
  for (const key of Object.keys(env)) assert.ok(
    !/^(RUST|WASM_|SKIP_WASM_BUILD|SUBSTRATE_)/.test(key) || ['RUSTUP_HOME', 'RUSTUP_TOOLCHAIN'].includes(key),
    `unsupported Native SOURCEPREPARATION override: ${key}`);
}
function validateNativePreparationProof(proof, externalWorkspace, expected = nativePreparationProofSha256) {
  assert.equal(externalWorkspace, true, 'Native SOURCEPREPARATION cannot transfer to Root');
  exactKeys(proof, ['schema', 'stage', 'preflight', 'review', 'audits', 'rows'], 'Native SOURCEPREPARATION proof');
  assert.equal(proof.schema, 1, 'unknown Native SOURCEPREPARATION schema');
  assert.equal(proof.stage, nativePreparationStage, 'unknown Native SOURCEPREPARATION stage');
  exactKeys(proof.review, ['composition', 'dnsMisc', 'io', 'provider', 'scope'], 'Native operation review');
  exactKeys(proof.audits, ['actual', 'registryEquivalent', 'versionLock'], 'Native paired audits');
  assert.equal(sha256(canonical(proof)), expected, 'unreviewed Native SOURCEPREPARATION proof identity');
  const p = proof.preflight;
  exactKeys(p, ['schema', 'stage', 'capturedAt', 'qualification', 'sourceRoot', 'workspaceRoot', 'evidenceRoot',
    'identity', 'role', 'configuration', 'sharedCaches', 'producers', 'proofs'], 'Native source preflight');
  assert.equal(p.schema, 1, 'unknown source preflight schema');
  assert.equal(p.stage, 'accepted-composition-source-preflight', 'unknown source preflight stage');
  assert.deepEqual(p.role, { package: nativeRole.package, features: [nativeRole.features], target: nativeRole.target,
    edges: nativeRole.edges, toolchain: nativeRole.toolchain, profile: nativeRole.profile }, 'Native preparation role mismatch');
  assert.deepEqual(p.configuration, { home: nativeHome, cargoHome: join(nativeHome, '.cargo'),
    rustupHome: join(nativeHome, '.rustup'), configs: [[join(p.workspaceRoot, '.cargo/config.toml'), nativeCargoConfigSha256]] },
  'Native preparation configuration mismatch');
  assert.deepEqual(p.proofs.map(([path]) => path).sort(), [...nativePreparationProofFiles].sort(), 'Native preparation proof coverage');
  assert.ok(Array.isArray(proof.rows) && proof.rows.length > 0, 'Native preparation instances required');
  for (const row of proof.rows) {
    exactKeys(row, ['id', 'packageId', 'name', 'version', 'source', 'rawSources', 'decision'], 'Native preparation instance');
    assert.notEqual(row.id, 'RUSTSEC-2026-0387', 'Native0387 requires separate B approval');
    assert.ok(['selected-prerequisite', 'selected-backport', 'normal-absent'].includes(row.decision), 'unknown Native instance decision');
    assert.ok(Array.isArray(row.rawSources) && row.rawSources.length > 0 && new Set(row.rawSources).size === row.rawSources.length,
      'Native instance raw sources required exactly once');
  }
}
// Match the public acquisition inventory, including existing in-owner symlinks.
// Every regular file is sealed, not just Rust or a known caller list.
async function nativePreparationInventory(root, prefix = '') {
  assert.equal(await realpath(root), root, 'Native preparation inventory owner substitution');
  const files = [];
  for (const name of (await readdir(join(root, prefix))).sort()) {
    if (!prefix && name === '.git') continue;
    const path = prefix ? `${prefix}/${name}` : name;
    const absolute = join(root, path);
    const stat = await lstat(absolute);
    if (stat.isSymbolicLink()) {
      assert.ok((await realpath(absolute)).startsWith(`${root}/`), 'Native preparation external symlink');
      files.push([path, 'symlink', await readlink(absolute)]);
    } else if (stat.isDirectory()) files.push(...await nativePreparationInventory(root, path));
    else {
      assert.ok(stat.isFile(), 'Native preparation unsupported source type');
      files.push([path, sha256(await readFile(absolute)), stat.mode & 0o111]);
    }
  }
  return files;
}
async function checkNativePreparationSeal(seal, metadata, selected) {
  exactKeys(seal, ['sourceRoot', 'sourceInventories', 'consumers'], 'Native source seal');
  exactKeys(seal.sourceInventories, ['sdk', 'vendor'], 'Native source inventories');
  for (const owner of ['sdk', 'vendor']) assert.deepEqual(
    await nativePreparationInventory(join(seal.sourceRoot, owner)), seal.sourceInventories[owner],
    `Native preparation complete ${owner} source drift`);
  assert.deepEqual(seal.consumers.map(c => c.id).sort(), [...selected.keys()].sort(), 'Native complete consumer coverage drift');
  for (const consumer of seal.consumers) {
    exactKeys(consumer, ['id', 'root', 'owner', 'files'], 'Native consumer');
    const pkg = metadata.packages.find(p => p.id === consumer.id);
    assert.ok(pkg, 'Native consumer metadata identity missing');
    assert.equal(consumer.root, dirname(pkg.manifest_path), 'Native consumer manifest owner mismatch');
    assert.equal(await realpath(consumer.root), consumer.root, 'Native consumer owner substitution');
    const owner = ['sdk', 'vendor'].find(o => consumer.root.startsWith(`${join(seal.sourceRoot, o)}/`)) ?? null;
    assert.equal(consumer.owner, owner, 'Native consumer seal ownership mismatch');
    if (owner) assert.equal(consumer.files, null, 'duplicate Native consumer inventory');
    else assert.deepEqual(await nativePreparationInventory(consumer.root), consumer.files, 'Native complete external consumer source drift');
  }
}
function nativePreparationSelection(metadata, tree) {
  const selected = new Map();
  for (const line of tree.trimEnd().split('\n')) {
    const match = /^(\d+)([^\s]+) v(\S+)(?: \(proc-macro\))?(?: \(([^)]+)\))?\|([^|]*)$/.exec(line);
    assert.ok(match, 'unsupported Native preparation tree');
    const [, , name, version, source, features] = match;
    const packages = metadata.packages.filter(p => p.name === name && p.version === version &&
      (source?.startsWith('/') ? dirname(p.manifest_path) === source : p.source === registrySource));
    assert.equal(packages.length, 1, 'ambiguous Native preparation selected identity');
    const pkg = packages[0];
    const current = features ? features.split(',').sort() : [];
    // Host/build contexts may carry different features for the same package ID.
    selected.set(pkg.id, [...new Set([...(selected.get(pkg.id) ?? []), ...current])].sort());
    const forbidden = name.startsWith('hickory-') && version.startsWith('0.24.') ? /dnssec|text-parsing/i :
      name === 'keccak' && version === '0.1.4' ? /^asm$/ : name === 'rand' && ['0.8.5', '0.9.0'].includes(version) ? /^log$/ :
      (name === 'zstd' && version === '0.12.4') || (name === 'zstd-safe' && version === '6.0.6') || name === 'zstd-sys'
        ? /^(experimental|zstdmt|zdict_builder|legacy)$/ : null;
    if (forbidden) assert.ok(!current.some(f => forbidden.test(f)), `Native source prerequisite activated: ${name}`);
  }
  assert.ok(selected.size > 0, 'empty Native selection');
  return selected;
}
function assertNativePreparationInstances(entries, rows, findings, metadata, selected) {
  const ids = [...new Set(rows.map(r => r.id))].sort();
  assert.deepEqual(entries.map(e => e.id).sort(), ids, 'Native preparation exact advisory coverage');
  for (const entry of entries) {
    exactKeys(entry.sourceCallerNative, ['stage', 'proof'], 'Native preparation reference');
    assert.equal(entry.sourceCallerNative.stage, nativePreparationStage, 'unknown Native reference stage');
    assert.equal(entry.sourceCallerNative.proof, 'nativeSourcePreparationProof', 'unknown Native shared proof owner');
    assert.equal(entry.ecosystem, 'cargo', 'Native preparation requires Cargo');
    assert.equal(entry.sourceCallerRoot, undefined, 'Native/Root proof conflict');
    assert.equal(entry.reviewedSources, undefined, 'Native preparation uses shared source owner');
    assert.equal(entry.excludedFeatures, undefined, 'Native preparation uses exact fresh role features');
    const instances = rows.filter(r => r.id === entry.id);
    assert.equal(entry.reachability, instances.some(r => r.decision === 'selected-backport') ?
      'source-backport-reviewed' : 'source-prerequisite-excluded', 'Native instance classification mismatch');
    const actual = [...new Set(findings.filter(f => f.advisory.id === entry.id).map(f => canonical({
      name: f.package.name, version: f.package.version, source: f.package.source })))].sort();
    const covered = [];
    const seen = new Set();
    for (const row of instances) {
      assert.ok(!seen.has(row.packageId), 'duplicate or overlapping Native package instance');
      seen.add(row.packageId);
      const pkg = metadata.packages.find(p => p.id === row.packageId);
      assert.ok(pkg, 'Native actual role package ID missing');
      assert.deepEqual({ name: pkg.name, version: pkg.version, source: pkg.source },
        { name: row.name, version: row.version, source: row.source }, 'Native actual package/source mismatch');
      assert.equal(selected.has(pkg.id), row.decision !== 'normal-absent', 'Native selected/normal-absent instance mismatch');
      if (row.decision === 'selected-backport') assert.equal(pkg.source, null, 'Native backport requires actual local source');
      covered.push(...row.rawSources.map(source => canonical({ name: row.name, version: row.version, source })));
    }
    assert.deepEqual(covered.sort(), actual, 'Native disjoint actual/counterfactual instance coverage mismatch');
  }
}
function nativePreparationAuditIdentity(audit) {
  assert.ok(Array.isArray(audit?.vulnerabilities?.list), 'Native audit vulnerability list required');
  // Only finding order is representational; complete content and multiplicity remain bound.
  const list = audit.vulnerabilities.list.map(finding => canonical(finding)).sort().map(value => JSON.parse(value));
  return sha256(canonical({ ...audit, vulnerabilities: { ...audit.vulnerabilities, list } }));
}
function assertNativePreparationAudits(proof, audits) {
  assert.deepEqual(proof.audits, { actual: nativePreparationAuditIdentity(audits.cargo),
    registryEquivalent: nativePreparationAuditIdentity(audits.projectedAudit), versionLock: sha256(audits.projectedLock) },
  'Native reviewed paired audit/database/settings identity drift');
}
async function loadNativePreparationProof(reference, exceptionPath) {
  exactKeys(reference, ['path'], 'Native shared proof file owner');
  const proofRoot = dirname(await realpath(exceptionPath));
  const path = await regularOwnerFile(proofRoot, reference.path);
  const bytes = await readFile(path);
  return { proofRoot, proof: JSON.parse(bytes.toString('utf8')), fileSha256: sha256(bytes) };
}
async function enforceNativePreparation(entries, reference, findings, metadata, repoRoot, lockPath, lock, externalWorkspace, exceptionPath, audits) {
  const { proofRoot, proof, fileSha256 } = await loadNativePreparationProof(reference, exceptionPath);
  validateNativePreparationProof(proof, externalWorkspace);
  assertNativePreparationAudits(proof, audits);
  const p = proof.preflight;
  checkNativePreparationEnvironment();
  assert.equal(process.platform, 'linux', 'Native source preparation requires Linux');
  assert.equal(process.arch, 'x64', 'Native source preparation requires x86_64');
  const workspace = await realpath(p.workspaceRoot);
  assert.equal(workspace, p.workspaceRoot, 'Native preparation workspace substitution');
  assert.equal(workspace, join(p.sourceRoot, 'sdk'), 'Native preparation source owner mismatch');
  assert.equal(await realpath(lockPath), join(workspace, 'Cargo.lock'), 'Native preparation lock role mismatch');
  assert.equal(metadata.workspace_root, workspace, 'Native preparation metadata role mismatch');
  const repo = await realpath(repoRoot);
  assert.deepEqual(p.producers.map(([path]) => path).sort(), nativeProducers.map(path => join(repo, path)).sort(),
    'Native preparation public producer coverage mismatch');
  const checkInputs = async () => {
    await boundFile(proofRoot, { path: reference.path, sha256: fileSha256 });
    checkNativePreparationEnvironment();
    await checkNativeConfiguration(workspace);
    for (const [path, digest] of p.producers) await boundFile(repo, { path: relative(repo, path), sha256: digest });
    for (const [path, digest] of p.proofs) await boundFile(p.evidenceRoot, { path, sha256: digest });
    assert.equal(await realpath(p.sourceRoot), p.sourceRoot, 'Native source root substitution');
    assert.equal(await realpath(p.evidenceRoot), p.evidenceRoot, 'Native source evidence substitution');
    assert.equal(sha256(await readFile(join(workspace, 'Cargo.toml'))), p.identity.manifest, 'Native source manifest drift');
    assert.equal(sha256(await readFile(lockPath)), p.identity.lock, 'Native source lock drift');
    assert.equal(await readFile(lockPath, 'utf8'), lock, 'Native source lock changed during review');
  };
  // Reject stale producer pins before executing any acquisition code.
  await checkInputs();
  const recorded = await readJson(join(p.evidenceRoot, 'composed-metadata.json'));
  const seal = await readJson(join(p.evidenceRoot, 'source-consumers.json'));
  assert.equal(seal.sourceRoot, p.sourceRoot, 'Native source seal owner mismatch');
  for (const owner of ['sdk', 'vendor']) assert.equal(sha256(JSON.stringify(seal.sourceInventories[owner])), p.identity[owner],
    'Native accepted composition inventory identity mismatch');
  const toolBin = join(nativeHome, '.rustup/toolchains/1.94.1-x86_64-unknown-linux-gnu/bin');
  for (const tool of ['rustc', 'cargo']) assert.equal(await realpath(join(toolBin, tool)), join(toolBin, tool), 'Native fixed source tool substitution');
  const temporary = await mkdtemp(join(tmpdir(), 'deos-native-preparation.'));
  try {
    const run = (command, args) => {
      const result = spawnSync(command, args, { cwd: workspace, encoding: 'utf8', maxBuffer: 128 * 1024 * 1024,
        env: { ...process.env, RUSTC: join(toolBin, 'rustc'), CARGO_TARGET_DIR: temporary,
          CARGO_TERM_COLOR: 'never', CARGO_NET_OFFLINE: 'true', GIT_ALLOW_PROTOCOL: '', GIT_TERMINAL_PROMPT: '0' } });
      assert.equal(result.status, 0, `Native preparation acquisition failed: ${result.error ?? result.stderr}`);
      return result.stdout;
    };
    const acquire = async () => {
      const compiler = run(join(toolBin, 'rustc'), ['-vV']);
      assert.match(compiler, /^release: 1\.94\.1$/m);
      assert.match(compiler, /^commit-hash: e408947bfd200af42db322daf0fadfe7e26d3bd1$/m);
      assert.match(compiler, /^LLVM version: 21\.1\.8$/m);
      const tools = compiler + run(join(toolBin, 'cargo'), ['-V']) +
        run(join(toolBin, 'rustc'), ['--print', 'cfg', '--target', nativeRole.target]);
      assert.equal(tools, await readFile(join(p.evidenceRoot, 'source-tools.txt'), 'utf8'), 'Native source compiler/default cfg drift');
      const current = JSON.parse(run(join(toolBin, 'cargo'), ['metadata', '--locked', '--offline', '--features',
        'polkadot-omni-node/runtime-benchmarks', '--format-version', '1']));
      const graph = v => canonical({ packages: v.packages, resolve: v.resolve, workspace_root: v.workspace_root, workspace_members: v.workspace_members });
      assert.equal(graph(current), graph(recorded), 'Native current full metadata drift');
      assert.equal(graph(current), graph(metadata), 'Native supplied metadata drift');
      const tree = run(join(toolBin, 'cargo'), ['tree', '--locked', '--offline', '-p', nativeRole.package, '--features',
        nativeRole.features, '--target', nativeRole.target, '--edges', nativeRole.edges, '--prefix', 'depth', '--format', '{p}|{f}']);
      assert.equal(tree, await readFile(join(p.evidenceRoot, 'composed-tree.txt'), 'utf8'), 'Native fresh normal tree drift');
      assert.equal(run('git', ['-c', 'protocol.allow=never', '--no-optional-locks', 'show-ref']),
        await readFile(join(p.evidenceRoot, 'git-refs.txt'), 'utf8'), 'Native actual source refs drift');
      const selected = nativePreparationSelection(current, tree);
      await checkNativePreparationSeal(seal, current, selected);
      assertNativePreparationInstances(entries, proof.rows, findings, current, selected);
      await checkInputs();
    };
    // Both passes re-acquire the role AND every input. An unchanged dependency
    // graph cannot hide a new direct/transitive/macro/generator operation.
    await acquire();
    await acquire();
    console.log('[INFO] Native SOURCEPREPARATION source/operation prerequisites enforced; no ELF, B0387, Root, general CLI or build admission');
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

// A finite public-input/initialization source case, never package-wide crypto or
// Native artifact clearance. Updating this reviewed identity requires a new review;
// neither a caller-supplied digest nor an environment override can bless a snapshot.
const rootCallerProofSha256 = '59010df2e6c94ec9e55f8aabd26d30761671a56025ebab0ebc90131aee5daca8';
const rootCallerRole = { ...rootRole, purpose: 'finite-public-operand-source-case', toolchain: '1.94.1',
  profile: 'release: opt3, unwind; production: fat LTO, codegen-units 1',
  support: 'Root source roles; both Wasm targets; native runtime and offline PVF; macOS acquisition retained; no native binary clearance' };
const rootCallerRoots = ['crypto-bigint@0.5.5', 'ctutils@0.4.8', 'cmov@0.5.4',
  'elliptic-curve@0.13.8', 'k256@0.13.4', 'p256@0.13.2', 'ecdsa@0.16.9', 'primeorder@0.13.6',
  'sp-core@43.0.0', 'sp-runtime@49.0.0', 'sp-io@49.0.0', 'frame-support@49.1.0',
  'sc-network@0.60.0', 'sc-chain-spec@53.0.0', 'polkadot-node-metrics@34.0.0',
  'polkadot-node-core-pvf@36.0.0', 'polkadot-node-core-pvf-common@29.0.0', 'substrate-wasm-builder@35.0.0'];
const rootCallerInputs = ['template/Cargo.toml', 'template/Cargo.lock', 'template/rust-toolchain.toml',
  'scripts/03-build-runtime.sh', 'scripts/_common.sh', 'scripts/ci-local.sh'];
const rootCallerTrees = ['all', 'wasm-v1-normal', 'wasm-v1-extended', 'wasm-unknown-normal',
  'wasm-unknown-extended', 'native-runtime', 'pvf-host'];
function rootCallerTreeArgs(role) {
  assert.ok(rootCallerTrees.includes(role), 'unsupported Root tree role');
  if (role === 'all') return rootTreeArgs.slice(1);
  const base = ['tree', '--locked', '--offline', '-p', role === 'pvf-host' ? 'deos-pvf-validation' : 'deos-runtime'];
  if (role.startsWith('wasm-')) {
    base.push('--no-default-features', '--target', role.startsWith('wasm-v1-') ? 'wasm32v1-none' : 'wasm32-unknown-unknown');
    if (role.endsWith('-extended')) base.push('--features', 'runtime-benchmarks,try-runtime');
  } else base.push('--target', 'x86_64-unknown-linux-gnu');
  return [...base, '--edges', role.startsWith('wasm-') ? 'normal' : 'normal,build', '--prefix', 'depth', '--format', '{p}|{f}'];
}
function checkRootCallerEnvironment(env = process.env) {
  for (const key of Object.keys(env)) {
    assert.ok(!(key.startsWith('CARGO_') && !/^(CARGO_BUILD_JOBS|CARGO_TARGET_DIR|CARGO_NET_OFFLINE|CARGO_TERM_[A-Z_]+|CARGO_HOME)$/.test(key)) &&
      !(/^(RUST|WASM_|SKIP_WASM_BUILD|SUBSTRATE_)/.test(key) && !['RUSTUP_HOME', 'RUSTUP_TOOLCHAIN'].includes(key)),
    `unsupported Root compiler/configuration override: ${key}`);
  }
  assert.equal(env.HOME ?? nativeHome, nativeHome, 'unsupported Root HOME');
  assert.equal(env.CARGO_HOME ?? join(nativeHome, '.cargo'), join(nativeHome, '.cargo'), 'unsupported Root CARGO_HOME');
  assert.equal(env.RUSTUP_HOME ?? join(nativeHome, '.rustup'), join(nativeHome, '.rustup'), 'unsupported Root RUSTUP_HOME');
  if (env.RUSTUP_TOOLCHAIN !== undefined) assert.equal(env.RUSTUP_TOOLCHAIN, '1.94.1', 'unsupported Root RUSTUP_TOOLCHAIN');
}
async function checkRootCallerConfiguration(workspace, cargoHome = join(nativeHome, '.cargo')) {
  checkRootCallerEnvironment();
  const paths = new Set();
  for (let cwd = workspace; ; cwd = dirname(cwd)) {
    for (const name of ['config', 'config.toml']) paths.add(join(cwd, '.cargo', name));
    if (dirname(cwd) === cwd) break;
  }
  for (const name of ['config', 'config.toml']) paths.add(join(cargoHome, name));
  for (const path of paths) {
    const stat = await lstat(path).catch(error => { if (error.code === 'ENOENT') return null; throw error; });
    assert.equal(stat, null, `unreviewed Root Cargo configuration: ${path}`);
  }
}
// Workspace Rust files include unconventional target paths; source/test/example
// and fixture trees also bind non-Rust inputs. Package build outputs are excluded.
// Dependency packages are complete: build scripts/macros may consume non-Rust
// inputs outside src. No symbol scan or conventional-directory guess is a boundary.
async function rootCallerInventory(root, workspace = false, prefix = '') {
  const files = [];
  for (const name of (await readdir(join(root, prefix))).sort()) {
    if (['target', '.git'].includes(name) && (prefix === '' ||
      (workspace && (await readdir(join(root, prefix))).includes('Cargo.toml')))) continue;
    const path = prefix ? `${prefix}/${name}` : name;
    const stat = await lstat(join(root, path));
    assert.ok(!stat.isSymbolicLink(), `Root inventory symlink substitution: ${path}`);
    if (stat.isDirectory()) files.push(...await rootCallerInventory(root, workspace, path));
    else {
      assert.ok(stat.isFile(), 'unsupported Root inventory file type');
      if (!workspace || path.endsWith('.rs') || name === 'Cargo.toml' ||
        /(?:^|\/)(?:src|tests|examples|fixtures)\//.test(path)) files.push(path);
    }
  }
  return files.sort();
}
async function checkRootCallerFiles(root, files, paths) {
  assert.deepEqual(files.map(f => f.path).sort(), paths, 'Root complete source/manifest inventory drift');
  for (const file of files) await boundFile(root, file);
}
function validateRootCallerProof(proof, externalWorkspace) {
  assert.equal(externalWorkspace, false, 'Root source proof cannot transfer to external Native workspace');
  exactKeys(proof, [...Object.keys(rootCallerRole), 'workspaceRoot', 'compilerSha256', 'advisory',
    'inputs', 'packages', 'trees'], 'Root caller proof');
  for (const [key, value] of Object.entries(rootCallerRole)) assert.equal(proof[key], value, `unsupported Root caller ${key}`);
  assert.deepEqual(Object.keys(proof.trees).sort(), [...rootCallerTrees].sort(), 'Root exact separate feature trees required');
  assert.equal(sha256(canonical(proof)), rootCallerProofSha256, 'unreviewed Root caller proof identity');
}
function assertRootCallerFindings(entry, findings, metadata) {
  assert.equal(entry.ecosystem, 'cargo', 'Root source case requires Cargo');
  assert.equal(entry.id, 'RUSTSEC-2026-0387', 'unsupported Root source advisory');
  assert.equal(entry.reachability, 'source-prerequisite-excluded', 'Root source case is not package-wide backport or absence');
  assert.equal(entry.sourceCallerNative, undefined, 'Root/Native source case conflict');
  const affected = findings.filter(f => f.advisory.id === entry.id);
  assert.ok(affected.length > 0 && affected.every(f => f.package.name === 'crypto-bigint' && f.package.version === '0.5.5' &&
    (f.package.source === undefined || f.package.source === null || f.package.source === registrySource)), 'Root affected package/source mismatch');
  const packages = metadata.packages.filter(p => p.name === 'crypto-bigint' && p.version === '0.5.5');
  assert.equal(packages.length, 1, 'Root actual bigint instance mismatch');
  assert.equal(packages[0].source, null, 'Root requires actual local maintained bigint');
}
function assertRootCallerTree(role, tree, proof, root) {
  assert.equal(sha256(tree.replaceAll(root, '$REPO')), proof.trees[role], `Root ${role} source/parent/feature tree drift`);
  assert.ok(tree.trim().length > 0, 'empty Root caller tree');
  for (const line of tree.trimEnd().split('\n').filter(Boolean)) assert.match(line, /^\d+[^ |]+ v[^ |]+(?: .*?)?\|[^|]*$/, 'unsupported Root caller tree');
  // Do not join/drop all-tree rows against unified metadata: cxxbridge-cmd is
  // unrelated and absent there. The entire raw tree identity still binds it.
  if (role.startsWith('wasm-')) {
    assert.ok(!/^\d+(?:p256|sc-network) v/m.test(tree), 'Root guest selected secret network role');
    const core = tree.split('\n').filter(line => /^\d+sp-core v/.test(line));
    assert.ok(core.length > 0 && core.every(line => !/(?:^|[,|])(?:std|full_crypto)(?:,| |$)/.test(line)), 'Root guest std/full_crypto activated');
  }
}
function rootCallerSelection(metadata, trees) {
  const selected = new Set();
  for (const tree of Object.values(trees)) {
    for (const line of tree.trimEnd().split('\n').filter(Boolean)) {
      const match = /^\d+([^\s]+) v(\S+)(?: \(proc-macro\))?(?: \(([^)]+)\))?\|[^|]*$/.exec(line);
      assert.ok(match, 'unsupported Root selection tree');
      const [, name, version, source] = match;
      const matches = metadata.packages.filter(p => p.name === name && p.version === version &&
        (source?.startsWith('/') ? dirname(p.manifest_path) === source : p.source === registrySource));
      // This unrelated Cargo-internal tool is absent from metadata. It remains
      // whole-tree bound, never silently joined to a different package/source.
      if (name === 'cxxbridge-cmd' && version === '1.0.196' && source === undefined && matches.length === 0) continue;
      assert.equal(matches.length, 1, `Root selected package/source identity mismatch: ${name}@${version}`);
      selected.add(matches[0].id);
    }
  }
  return selected;
}
function rootCallerBoundary(metadata, selected, roots = rootCallerRoots) {
  const packages = new Map(metadata.packages.map(p => [p.id, p]));
  const nodes = new Map(metadata.resolve.nodes.map(n => [n.id, n]));
  const normalBuild = d => d.dep_kinds.some(k => k.kind === null || k.kind === 'build');
  const deps = id => {
    assert.ok(packages.has(id) && nodes.has(id), 'Root boundary package/node missing');
    return nodes.get(id).deps.filter(d => selected.has(d.pkg) && normalBuild(d));
  };
  const consumers = new Set(roots.map(key => {
    const matches = metadata.packages.filter(p => `${p.name}@${p.version}` === key && selected.has(p.id));
    assert.equal(matches.length, 1, `Root caller package identity mismatch: ${key}`);
    return matches[0].id;
  }));
  // Induced normal/build metadata edges deliberately overapproximate role/target
  // unification. Reexports and arbitrarily long public API routes need no names.
  for (let changed = true; changed;) {
    changed = false;
    for (const id of selected) if (!consumers.has(id) && deps(id).some(d => consumers.has(d.pkg))) {
      consumers.add(id); changed = true;
    }
  }
  const required = new Set([...consumers, ...metadata.workspace_members]);
  // Proc macros/build scripts AND ordinary reexported macro_rules providers can
  // emit caller tokens without importing crypto themselves. Bind the forward
  // implementation closure, not just packages labelled proc-macro. On this graph
  // the workspace umbrella reaches every metadata-joined selected package.
  for (const id of required) for (const d of deps(id)) required.add(d.pkg);
  return required;
}
function assertRootCallerCoverage(proof, required) {
  assert.deepEqual(proof.packages.map(p => p.id).sort(), [...required].sort(), 'Root complete caller package coverage required');
}
async function checkRootCallerPackages(proof, metadata, root, selected) {
  assertRootCallerCoverage(proof, rootCallerBoundary(metadata, selected));
  for (const review of proof.packages) {
    exactKeys(review, ['id', 'name', 'version', 'source', 'manifest', 'files'], 'Root caller package');
    const pkg = metadata.packages.find(p => p.id === review.id);
    assert.ok(pkg, 'Root package ID missing');
    assert.deepEqual([pkg.name, pkg.version, pkg.source, pkg.manifest_path],
      [review.name, review.version, review.source, review.manifest], 'Root caller source/manifest substitution');
    const owner = dirname(review.manifest);
    assert.equal(await realpath(owner), owner, 'Root caller owner substitution');
    if (metadata.workspace_members.includes(pkg.id)) assert.ok(owner.startsWith(`${root}/template/`), 'Root workspace caller outside owner');
    else if (pkg.name === 'crypto-bigint') assert.equal(owner, join(root, 'template/vendor/crypto-bigint'), 'Root maintained source substitution');
    else if (pkg.source === null) assert.ok(owner.startsWith(`${root}/template/vendor/`), 'Root local dependency outside vendor owner');
    else assert.equal(pkg.source, registrySource, 'Root registry caller replacement');
    await checkRootCallerFiles(owner, review.files, await rootCallerInventory(owner, metadata.workspace_members.includes(pkg.id)));
  }
}
async function enforceRootCaller(entry, findings, metadata, repoRoot, lockPath, lock, externalWorkspace) {
  const proof = entry.sourceCallerRoot;
  validateRootCallerProof(proof, externalWorkspace);
  assertRootCallerFindings(entry, findings, metadata);
  checkRootCallerEnvironment();
  const root = await realpath(repoRoot);
  assert.equal(root, resolve(repoRoot), 'Root repository path substitution');
  const workspace = join(root, 'template');
  assert.equal(await realpath(workspace), workspace, 'Root workspace symlink substitution');
  assert.equal(proof.workspaceRoot, workspace, 'Root caller workspace mismatch');
  assert.equal(metadata.workspace_root, workspace, 'Root caller metadata role mismatch');
  assert.equal(resolve(lockPath), join(workspace, 'Cargo.lock'), 'Root caller lock role mismatch');
  assert.equal(await realpath(lockPath), resolve(lockPath), 'Root caller lock substitution');
  const check = async (selected) => {
    await checkRootCallerConfiguration(workspace);
    await checkRootCallerFiles(root, proof.inputs, [...rootCallerInputs].sort());
    await boundFile(join(nativeHome, '.cargo/advisory-db'), proof.advisory);
    assert.equal(proof.advisory.path, 'crates/crypto-bigint/RUSTSEC-2026-0387.md', 'Root primary advisory owner mismatch');
    await checkRootCallerPackages(proof, metadata, root, selected);
    assert.equal(await readFile(lockPath, 'utf8'), lock, 'Root lock changed during caller review');
  };
  await checkRootCallerConfiguration(workspace);
  await checkRootCallerFiles(root, proof.inputs, [...rootCallerInputs].sort());
  const paths = {};
  for (const executable of ['cargo', 'rustc']) {
    const selected = spawnSync('rustup', ['which', '--toolchain', '1.94.1', executable], { encoding: 'utf8' });
    assert.equal(selected.status, 0, 'Root fixed toolchain unavailable');
    paths[executable] = selected.stdout.trim();
    assert.equal(await realpath(paths[executable]), paths[executable], 'Root fixed toolchain substitution');
    assert.ok(paths[executable].startsWith(join(nativeHome, '.rustup/toolchains/1.94.1-')), 'Root toolchain home substitution');
  }
  const compiler = spawnSync(paths.rustc, ['-vV'], { encoding: 'utf8' });
  assert.equal(compiler.status, 0, 'Root compiler unavailable');
  assert.match(compiler.stdout, /^release: 1\.94\.1$/m, 'Root compiler release drift');
  assert.match(compiler.stdout, /^LLVM version: 21\.1\.8$/m, 'Root compiler LLVM drift');
  assert.equal(sha256(compiler.stdout), proof.compilerSha256, 'Root compiler support identity drift');
  const temporary = await mkdtemp(join(tmpdir(), 'deos-root-caller.'));
  try {
    const run = args => {
      const result = spawnSync(paths.cargo, args, { cwd: workspace, encoding: 'utf8', maxBuffer: 128 * 1024 * 1024,
        env: { ...process.env, RUSTC: paths.rustc, CARGO_TARGET_DIR: temporary, CARGO_TERM_COLOR: 'never',
          CARGO_NET_OFFLINE: 'true', GIT_ALLOW_PROTOCOL: '', GIT_TERMINAL_PROMPT: '0' } });
      assert.equal(result.status, 0, `current Root caller Cargo proof failed: ${result.error ?? result.stderr}`);
      return result.stdout;
    };
    const beforeTree = run(rootCallerTreeArgs('all'));
    assertRootCallerTree('all', beforeTree, proof, root);
    await check(rootCallerSelection(metadata, { all: beforeTree }));
    const current = JSON.parse(run(['metadata', '--locked', '--offline', '--all-features', '--format-version', '1']));
    const graph = value => ({ packages: value.packages, workspace_members: value.workspace_members,
      resolve: value.resolve, workspace_root: value.workspace_root });
    assert.equal(canonical(graph(current)), canonical(graph(metadata)), 'Root stale full metadata/source graph');
    const trees = Object.fromEntries(rootCallerTrees.map(role => [role, run(rootCallerTreeArgs(role))]));
    for (const role of rootCallerTrees) assertRootCallerTree(role, trees[role], proof, root);
    const selected = rootCallerSelection(current, trees);
    await check(selected);
    console.log('[INFO] Current Root0387 finite caller/public-operand source case enforced; no Native, package-wide constant-time, binary or launch clearance');
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

function selfTestNativeInstances() {
  const selectedPackage = { name: 'quinn-udp', version: '0.6.1', source: registrySource };
  const absentPackage = { ...selectedPackage, version: '0.5.4' };
  const proof = { ...nativeRole, workspaceRoot: '/fixture/sdk', treeSha256: '0'.repeat(64),
    inputs: [], producers: [], advisories: [], packages: [], affectedPackages: [selectedPackage],
    absentAffectedPackages: [absentPackage] };
  const affected = [selectedPackage, absentPackage].map(package_ => ({ package: package_ }));
  const metadata = { packages: [selectedPackage, absentPackage].map((p, i) => ({ ...p, id: String(i) })) };
  const selected = new Set(['0']);
  const check = (p = proof, m = metadata, s = selected, f = affected) => {
    nativeInstanceBindings(p, f);
    assertNativeAbsentInstances(p.absentAffectedPackages ?? [], m, s);
  };
  check();
  check({ ...proof, absentAffectedPackages: [] }, metadata, selected, affected.slice(0, 1));
  const old = { ...proof }; delete old.absentAffectedPackages;
  check(old, metadata, selected, affected.slice(0, 1));
  let negatives = 0;
  for (const mutate of [
    p => { p.absentAffectedPackages = null; },
    p => { p.absentAffectedPackages = {}; },
    p => { p.absentAffectedPackages = []; },
    p => { p.absentAffectedPackages.push(p.absentAffectedPackages[0]); },
    p => { p.affectedPackages.push(p.affectedPackages[0]); },
    p => { p.absentAffectedPackages.push(p.affectedPackages[0]); },
    p => { p.affectedPackages = []; },
    p => { p.absentAffectedPackages[0].version = '0.5.5'; },
    p => { p.absentAffectedPackages[0].source = 'registry+other'; },
    p => { p.absentAffectedPackages[0].source = 1; },
    p => { p.absentAffectedPackages[0].extra = true; },
    p => { p.absentAffectedPackages.push({ ...absentPackage, version: '9' }); },
    p => { p.unknown = true; },
    p => { p.affectedPackages = [absentPackage]; p.absentAffectedPackages = [selectedPackage]; },
  ]) {
    const p = structuredClone(proof); mutate(p);
    assert.throws(() => check(p)); negatives++;
  }
  for (const packages of [metadata.packages.slice(0, 1),
    [...metadata.packages, { ...metadata.packages[1], source: 'registry+other', id: '2' }],
    [metadata.packages[0], { ...metadata.packages[1], source: 'registry+other' }]]) {
    assert.throws(() => check(proof, { packages })); negatives++;
  }
  assert.throws(() => check(proof, metadata, new Set(['0', '1']))); negatives++;
  assert.throws(() => check(proof, metadata, selected, [])); negatives++;
  assert.equal(canonical(nativeInstanceBindings(old, affected.slice(0, 1))),
    canonical(nativeInstanceBindings(proof, affected)));
  console.log(`[SUCCESS] Native per-instance self-test: 3 positive, ${negatives} negative; exact role absence, not source clearance`);
}

async function selfTestNativeConfiguration(directory) {
  const { mkdir } = await import('node:fs/promises');
  const sdk = join(directory, 'native', 'sdk');
  const cargoHome = join(directory, 'cargo-home');
  await mkdir(join(sdk, '.cargo'), { recursive: true });
  await mkdir(cargoHome);
  const config = "[build]\nrustdocflags = [\n\t\"-Dwarnings\",\n\t\"-Arustdoc::redundant_explicit_links\", # stylistic\n]\n\n[env]\n# Needed for musl builds so user doesn't have to install musl-tools.\nCC_x86_64_unknown_linux_musl = { value = \".cargo/musl-gcc\", force = true, relative = true }\nCXX_x86_64_unknown_linux_musl = { value = \".cargo/musl-g++\", force = true, relative = true }\nCARGO_WORKSPACE_ROOT_DIR = { value = \"\", relative = true }\nSQLX_OFFLINE = \"true\"\n\n[net]\nretry = 5\n# git-fetch-with-cli = true # commented because there is a risk that a runner can be banned by github\n";
  const path = join(sdk, '.cargo', 'config.toml');
  await writeFile(path, config);
  const baseline = { HOME: nativeHome };
  checkNativeEnvironment(baseline);
  checkNativeEnvironment({ ...baseline, CARGO_BUILD_JOBS: '2', CARGO_NET_OFFLINE: 'true',
    CARGO_TERM_COLOR: 'never', CARGO_TARGET_DIR: '/tmp/own-target', RUSTUP_TOOLCHAIN: '1.94.1',
    CARGO_HOME: join(nativeHome, '.cargo') });
  const overrides = {
    RUSTFLAGS: '--cfg feature="dimpl"', CARGO_ENCODED_RUSTFLAGS: '--cfg\x1ffeature="dimpl"',
    CARGO_PROFILE_PRODUCTION_LTO: 'off', CARGO_PROFILE_PRODUCTION_CODEGEN_UNITS: '16',
    RUSTC_WRAPPER: '/tmp/wrapper', RUSTC_WORKSPACE_WRAPPER: '/tmp/wrapper',
    RUSTC: '/tmp/rustc', RUSTDOC: '/tmp/rustdoc', RUSTDOCFLAGS: '--cfg other',
    RUSTC_BOOTSTRAP: '1', CARGO_BUILD_RUSTFLAGS: '--cfg other', CARGO_BUILD_TARGET: 'other',
    CARGO_BUILD_RUSTC: '/tmp/rustc', CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS: '--cfg other',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: '/tmp/linker',
    CARGO_UNSTABLE_CONFIG_INCLUDE: 'true', CARGO_ENCODED_RUSTDOCFLAGS: '--cfg other',
    CARGO_HOME: cargoHome, RUSTUP_HOME: '/tmp/rustup', RUSTUP_TOOLCHAIN: 'stable',
    CARGO_SOURCE_CRATES_IO_REPLACE_WITH: 'other', CARGO_REGISTRIES_CRATES_IO_INDEX: 'other',
  };
  for (const [key, value] of Object.entries(overrides)) {
    assert.throws(() => checkNativeEnvironment({ ...baseline, [key]: value }), /unsupported Native/);
  }
  for (const [key, home] of [['CARGO_HOME', '.cargo'], ['RUSTUP_HOME', '.rustup']]) {
    assert.throws(() => checkNativeEnvironment({ ...baseline,
      [key]: relative(process.cwd(), join(nativeHome, home)) || '.' }), /unsupported Native/);
  }
  assert.throws(() => checkNativeEnvironment({ ...baseline, CARGO_BUILD_JOBS: '2',
    RUSTFLAGS: '', CARGO_PROFILE_PRODUCTION_LTO: 'fat' }), /unsupported Native/);
  const check = () => checkNativeConfiguration(sdk, cargoHome);
  await check();
  for (const added of [join(sdk, '.cargo', 'config'), join(directory, '.cargo', 'config.toml'),
    join(directory, '.cargo', 'config'), join(cargoHome, 'config.toml'), join(cargoHome, 'config')]) {
    await mkdir(dirname(added), { recursive: true });
    await writeFile(added, '[build]\nrustflags=["--cfg", "other"]\n');
    await assert.rejects(check, /unreviewed Native Cargo configuration/);
    await rm(added);
  }
  for (const changed of [config + '\n[profile.production]\nlto=false\n',
    config + '\ninclude="other.toml"\n', config + '\n[source.crates-io]\nreplace-with="other"\n']) {
    await writeFile(path, changed);
    await assert.rejects(check, /reviewed Cargo config drift/);
  }
  await rm(path);
  await assert.rejects(check, /reviewed Cargo config missing/);
  await writeFile(path, config);
  await check();
  console.log('[SUCCESS] Native configuration self-test: 4 positive, ' +
    (Object.keys(overrides).length + 12) + ' unchanged-policy negatives; additions/deletions and repeated drift checks');
}

async function selfTestRootCaller(directory, proofPath) {
  const { mkdir, rename, symlink } = await import('node:fs/promises');
  const root = join(directory, 'root-caller');
  const workspace = join(root, 'template');
  await mkdir(join(workspace, 'runtime/src'), { recursive: true });
  await mkdir(join(workspace, 'pvf/src'), { recursive: true });
  await mkdir(join(workspace, 'core/src'), { recursive: true });
  const manifest = '[workspace]\nmembers=["runtime","pvf","core"]\nresolver="2"\n';
  await writeFile(join(workspace, 'Cargo.toml'), manifest);
  await writeFile(join(workspace, 'runtime/Cargo.toml'), '[package]\nname="deos-runtime"\nversion="0.0.0"\n[dependencies]\nsp-core={path="../core",default-features=false}\n[features]\ndefault=["sp-core/std"]\nruntime-benchmarks=[]\ntry-runtime=[]\n');
  await writeFile(join(workspace, 'pvf/Cargo.toml'), '[package]\nname="deos-pvf-validation"\nversion="0.0.0"\n');
  await writeFile(join(workspace, 'core/Cargo.toml'), '[package]\nname="sp-core"\nversion="43.0.0"\n[features]\nstd=[]\nfull_crypto=[]\n');
  const source = join(workspace, 'runtime/src/lib.rs');
  await writeFile(source, 'pub fn public_recover() {}\n');
  await writeFile(join(workspace, 'core/src/lib.rs'), '');
  await writeFile(join(workspace, 'pvf/src/main.rs'), 'fn main() {}\n');
  let positives = 0, negatives = 0;
  const rejected = async (fn, pattern) => { await assert.rejects(fn, pattern); negatives++; };
  const run = args => {
    const r = spawnSync('cargo', ['+1.94.1', ...args], { cwd: workspace, encoding: 'utf8',
      env: { ...process.env, CARGO_TARGET_DIR: join(root, 'target'), CARGO_TERM_COLOR: 'never' } });
    assert.equal(r.status, 0, r.stderr); return r.stdout;
  };
  run(['generate-lockfile', '--offline']);
  const metadataArgs = ['metadata', '--locked', '--offline', '--all-features', '--format-version', '1'];
  const graph = run(metadataArgs);
  const inventory = await rootCallerInventory(join(workspace, 'runtime'), true);
  const files = await Promise.all(inventory.map(async path => ({ path, sha256: sha256(await readFile(join(workspace, 'runtime', path))) })));
  const checkFiles = async () => checkRootCallerFiles(join(workspace, 'runtime'), files,
    await rootCallerInventory(join(workspace, 'runtime'), true));
  await checkFiles(); positives++;
  const trees = Object.fromEntries(rootCallerTrees.map(role => [role, run(rootCallerTreeArgs(role))]));
  const proof = { trees: Object.fromEntries(Object.entries(trees).map(([role, tree]) => [role, sha256(tree.replaceAll(root, '$REPO'))])) };
  for (const role of rootCallerTrees) { assertRootCallerTree(role, trees[role], proof, root); positives++; }
  // These change a real caller without changing the graph or no-full_crypto.
  for (const text of ['pub fn private_seed() { EcdsaPair::from_seed_slice(secret()); }',
    'pub fn network() { NetworkWorker::new(secret()); }',
    'pub fn network() { Litep2pNetworkBackend::new(secret()); }',
    'pub fn extensions() { extensions.register(KeystoreExt::new(keys)); }']) {
    await writeFile(source, text);
    assert.equal(run(metadataArgs), graph, 'source-only counterfactual must preserve Cargo graph');
    assert.equal(run(rootCallerTreeArgs('wasm-v1-normal')), trees['wasm-v1-normal']);
    await rejected(checkFiles, /source hash drift/);
  }
  await writeFile(source, 'pub fn public_recover() {}\n');
  await writeFile(join(workspace, 'runtime/src/new.rs'), 'fn secret() {}');
  await rejected(checkFiles, /inventory drift/);
  await rm(join(workspace, 'runtime/src/new.rs'));
  await rename(source, source + '.renamed.rs');
  await rejected(checkFiles, /inventory drift/);
  await rename(source + '.renamed.rs', source);
  const original = await readFile(source);
  await rm(source);
  await rejected(checkFiles, /inventory drift/);
  await writeFile(source, original);
  await writeFile(join(workspace, 'runtime/Cargo.toml'), '[package]\nname="substitution"\nversion="0.0.0"\n');
  await rejected(checkFiles, /source hash drift/);
  await writeFile(join(workspace, 'runtime/Cargo.toml'), '[package]\nname="deos-runtime"\nversion="0.0.0"\n[dependencies]\nsp-core={path="../core",default-features=false}\n[features]\ndefault=["sp-core/std"]\nruntime-benchmarks=[]\ntry-runtime=[]\n');
  await rename(source, source + '.saved');
  await symlink(source + '.saved', source);
  await rejected(checkFiles, /symlink substitution/);
  await rm(source);
  await rename(source + '.saved', source);
  for (const role of rootCallerTrees) {
    const stale = structuredClone(proof); stale.trees[role] = '0'.repeat(64);
    assert.throws(() => assertRootCallerTree(role, trees[role], stale, root), /tree drift/); negatives++;
  }
  // Even a matching metadata-derived hash cannot make a std guest a guest proof.
  const stdTree = trees['native-runtime'];
  await rejected(async () => assertRootCallerTree('wasm-v1-normal', stdTree,
    { trees: { 'wasm-v1-normal': sha256(stdTree.replaceAll(root, '$REPO')) } }, root), /guest std/);
  await rejected(async () => rootCallerTreeArgs('arbitrary-target'), /unsupported Root tree/);
  checkRootCallerEnvironment({ HOME: nativeHome }); positives++;
  const overrides = { RUSTFLAGS: '--cfg feature="dimpl"', CARGO_ENCODED_RUSTFLAGS: '--cfg\x1ffeature="dimpl"',
    CARGO_PROFILE_PRODUCTION_LTO: 'off', CARGO_PROFILE_PRODUCTION_CODEGEN_UNITS: '8',
    CARGO_BUILD_TARGET: 'other', CARGO_BUILD_RUSTFLAGS: '--cfg secret', RUSTC: '/tmp/rustc', RUSTC_WRAPPER: '/tmp/wrapper',
    RUSTC_BOOTSTRAP: '1', RUSTDOCFLAGS: '--cfg secret', WASM_BUILD_TYPE: 'debug', WASM_BUILD_RUSTFLAGS: '--cfg secret',
    SKIP_WASM_BUILD: '1', RUSTUP_TOOLCHAIN: 'stable', HOME: '/other', CARGO_HOME: '.cargo', RUSTUP_HOME: '.rustup' };
  for (const [key, value] of Object.entries(overrides)) {
    assert.throws(() => checkRootCallerEnvironment({ HOME: nativeHome, [key]: value }), /unsupported Root/); negatives++;
  }
  const cargoHome = join(root, 'cargo-home');
  await mkdir(cargoHome);
  const configCheck = () => checkRootCallerConfiguration(workspace, cargoHome);
  await configCheck(); positives++;
  for (const path of [join(workspace, '.cargo/config'), join(workspace, '.cargo/config.toml'),
    join(root, '.cargo/config'), join(root, '.cargo/config.toml'), join(cargoHome, 'config'), join(cargoHome, 'config.toml')]) {
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, '[build]\nrustflags=["--cfg", "secret"]\n');
    await rejected(configCheck, /unreviewed Root Cargo configuration/);
    await rm(path);
    await configCheck();
  }
  await checkFiles(); positives++;
  if (proofPath) {
    const actual = await readJson(proofPath);
    validateRootCallerProof(actual, false); positives++;
    for (const mutation of [p => { p.target = 'x86_64-unknown-linux-gnu'; }, p => { p.features = 'default'; },
      p => { delete p.trees['wasm-unknown-extended']; }, p => { p.trees.all = '0'.repeat(64); },
      p => { p.packages.pop(); }, p => { p.packages[0].source = registrySource; },
      p => { p.advisory.sha256 = '0'.repeat(64); }, p => { p.compilerSha256 = '0'.repeat(64); },
      p => { p.profile = 'debug'; }, p => { p.inputs.pop(); }, p => { p.packages[0].files.pop(); },
      p => { p.workspaceRoot = '/native/sdk'; }, p => { p.cargoArgs = ['--config', 'secret']; }]) {
      const changed = structuredClone(actual); mutation(changed);
      assert.throws(() => validateRootCallerProof(changed, false)); negatives++;
    }
    for (const invalid of [undefined, {}, { packages: actual.packages }]) {
      assert.throws(() => validateRootCallerProof(invalid, false)); negatives++;
    }
    assert.throws(() => validateRootCallerProof(actual, true), /cannot transfer/); negatives++;
    const actualRoot = dirname(actual.workspaceRoot);
    const supplied = spawnSync('cargo', ['+1.94.1', ...metadataArgs], { cwd: actual.workspaceRoot, encoding: 'utf8', maxBuffer: 128 * 1024 * 1024 });
    assert.equal(supplied.status, 0, supplied.stderr);
    const entry = { ecosystem: 'cargo', id: 'RUSTSEC-2026-0387', reachability: 'source-prerequisite-excluded', sourceCallerRoot: actual };
    const findings = [{ advisory: { id: entry.id }, package: { name: 'crypto-bigint', version: '0.5.5', source: registrySource } }];
    const metadata = JSON.parse(supplied.stdout);
    await enforceRootCaller(entry, findings, metadata, actualRoot,
      join(actual.workspaceRoot, 'Cargo.lock'), await readFile(join(actual.workspaceRoot, 'Cargo.lock'), 'utf8'), false);
    positives++;
    const actualTrees = Object.fromEntries(rootCallerTrees.map(role => {
      const result = spawnSync('cargo', ['+1.94.1', ...rootCallerTreeArgs(role)],
        { cwd: actual.workspaceRoot, encoding: 'utf8', maxBuffer: 128 * 1024 * 1024 });
      assert.equal(result.status, 0, result.stderr);
      return [role, result.stdout];
    }));
    const selected = rootCallerSelection(metadata, actualTrees);
    for (const name of ['cumulus-pallet-xcmp-queue', 'frame-system', 'sp-application-crypto']) {
      const pkg = metadata.packages.find(p => p.name === name);
      assert.ok(selected.has(pkg.id) && actual.packages.some(p => p.id === pkg.id), `Root dependency consumer missing: ${name}`);
      const missing = { packages: actual.packages.filter(p => p.id !== pkg.id) };
      await assert.rejects(() => checkRootCallerPackages(missing, metadata, actualRoot, selected), /coverage required/);
    }
    console.log('[SUCCESS] Actual Root consumer coverage: local XCMP, registry frame-system and sp-application-crypto included; each omitted binding rejected without source mutation');
    for (const change of [p => { p.source = 'registry+other'; }, p => { p.manifest_path += '.other'; },
      p => { p.id += '-other'; }]) {
      const changed = structuredClone(metadata);
      change(changed.packages.find(p => p.id === actual.packages[0].id));
      await rejected(() => checkRootCallerPackages(actual, changed, actualRoot, selected),
        /coverage required|substitution|package ID missing|package identity mismatch|package\/node missing/);
    }
    for (const change of [e => { e.reachability = 'source-backport-reviewed'; },
      e => { e.sourceCallerNative = {}; }, e => { e.id = 'RUSTSEC-OTHER'; }]) {
      const changed = structuredClone(entry); change(changed);
      assert.throws(() => assertRootCallerFindings(changed, findings, metadata)); negatives++;
    }
    for (const source of ['registry+other', 'git+other']) {
      const changed = structuredClone(findings); changed[0].package.source = source;
      assert.throws(() => assertRootCallerFindings(entry, changed, metadata), /package\/source mismatch/); negatives++;
    }
  }
  console.log(`[SUCCESS] Root caller self-test: ${positives} positive, ${negatives} negative; isolated functional Cargo fixtures${proofPath ? ' and pinned current Root baseline' : '; current Root baseline requires optional reviewed proof argument'}`);
}

async function selfTestRootConsumers(directory) {
  const { mkdir, rename } = await import('node:fs/promises');
  const root = join(directory, 'consumers');
  const workspace = join(root, 'template');
  const put = async (path, text) => { await mkdir(dirname(path), { recursive: true }); await writeFile(path, text); };
  await put(join(workspace, 'Cargo.toml'), '[workspace]\nmembers=["runtime","pvf"]\nexclude=["vendor/core","vendor/relay","vendor/direct","vendor/transitive","vendor/macro","vendor/proc","vendor/generator"]\nresolver="2"\n');
  const packages = [
    ['runtime', 'deos-runtime', '[dependencies]\ntransitive={path="../vendor/transitive"}\n[features]\ndefault=[]\nruntime-benchmarks=[]\ntry-runtime=[]\n', 'pub use transitive::*;'],
    ['pvf', 'deos-pvf-validation', '', 'pub fn offline() {}'],
    ['vendor/core', 'sp-core', '[workspace]\n[features]\nstd=[]\nfull_crypto=[]\n', '#![no_std]\npub mod ecdsa { pub struct Pair(pub [u8;32]); impl Pair { pub fn from_seed_slice(seed: &[u8]) -> Result<Self, ()> { let mut out=[0;32]; if seed.len()!=32 { return Err(()); } out.copy_from_slice(seed); Ok(Self(out)) } }'],
    ['vendor/relay', 'relay', '[workspace]\n[dependencies]\nsp-core={path="../core",default-features=false}\n', 'pub use sp_core;'],
    ['vendor/direct', 'direct', '[workspace]\n[dependencies]\nrelay={path="../relay"}\nmacro-provider={path="../macro"}\nproc-provider={path="../proc"}\n[build-dependencies]\ngenerator={path="../generator"}\n', 'pub use relay::sp_core;'],
    ['vendor/transitive', 'transitive', '[workspace]\n[dependencies]\ndirect={path="../direct"}\n', 'pub use direct::sp_core;'],
    ['vendor/macro', 'macro-provider', '[workspace]\n', '#[macro_export] macro_rules! seed { ($s:expr) => { sp_core::ecdsa::Pair::from_seed_slice($s) }; }'],
    ['vendor/proc', 'proc-provider', '[workspace]\n[lib]\nproc-macro=true\n', '#[proc_macro] pub fn seed(_: proc_macro::TokenStream) -> proc_macro::TokenStream { "pub fn public() {}".parse().unwrap() }'],
    ['vendor/generator', 'generator', '[workspace]\n', "pub fn tokens() -> &'static str { include_str!(\"../constructor.tokens\") }"],
  ];
  for (const [path, name, manifest, source] of packages) {
    await put(join(workspace, path, 'Cargo.toml'), `[package]\nname="${name}"\nversion="0.0.0"\nedition="2021"\n${manifest}`);
    await put(join(workspace, path, 'src/lib.rs'), source);
  }
  await put(join(workspace, 'vendor/direct/build.rs'), 'fn main() { let _ = generator::tokens(); }');
  await put(join(workspace, 'vendor/generator/constructor.tokens'), 'pub fn public() {}');
  const run = args => {
    const result = spawnSync('cargo', ['+1.94.1', ...args], { cwd: workspace, encoding: 'utf8',
      env: { ...process.env, CARGO_TARGET_DIR: join(root, 'target'), CARGO_TERM_COLOR: 'never' } });
    assert.equal(result.status, 0, result.stderr); return result.stdout;
  };
  run(['generate-lockfile', '--offline']);
  const metadataArgs = ['metadata', '--locked', '--offline', '--all-features', '--format-version', '1'];
  const raw = run(metadataArgs);
  const metadata = JSON.parse(raw);
  const trees = Object.fromEntries(rootCallerTrees.map(role => [role, run(rootCallerTreeArgs(role))]));
  const treeProof = { trees: Object.fromEntries(Object.entries(trees).map(([role, tree]) => [role, sha256(tree.replaceAll(root, '$REPO'))])) };
  for (const role of rootCallerTrees) assertRootCallerTree(role, trees[role], treeProof, root);
  const selected = rootCallerSelection(metadata, trees);
  const required = rootCallerBoundary(metadata, selected, ['sp-core@0.0.0']);
  assert.equal(required.size, packages.length);
  const proof = { packages: [] };
  for (const pkg of metadata.packages) {
    const owner = dirname(pkg.manifest_path);
    proof.packages.push({ id: pkg.id, files: await Promise.all((await rootCallerInventory(owner, false)).map(async path =>
      ({ path, sha256: sha256(await readFile(join(owner, path))) }))) });
  }
  const check = async (candidate = proof) => {
    assertRootCallerCoverage(candidate, rootCallerBoundary(metadata, selected, ['sp-core@0.0.0']));
    for (const review of candidate.packages) {
      const owner = dirname(metadata.packages.find(p => p.id === review.id).manifest_path);
      await checkRootCallerFiles(owner, review.files, await rootCallerInventory(owner, false));
    }
  };
  let positives = 0, negatives = 0;
  const unchanged = () => {
    assert.equal(run(metadataArgs), raw, 'nonworkspace mutation changed metadata/features');
    for (const role of rootCallerTrees) assert.equal(run(rootCallerTreeArgs(role)), trees[role], `nonworkspace mutation changed ${role}`);
  };
  await check(); positives++;
  for (const name of ['relay', 'direct', 'transitive', 'macro-provider', 'proc-provider', 'generator']) {
    const pkg = metadata.packages.find(p => p.name === name);
    assert.ok(!metadata.workspace_members.includes(pkg.id), 'consumer fixture must be nonworkspace');
    await assert.rejects(() => check({ packages: proof.packages.filter(p => p.id !== pkg.id) }), /coverage required/); negatives++;
  }
  for (const path of ['vendor/relay', 'vendor/direct', 'vendor/transitive']) {
    const source = join(workspace, path, 'src/lib.rs');
    const original = await readFile(source);
    await writeFile(source, Buffer.concat([original, Buffer.from('\npub fn newly_secret() { let _ = sp_core::ecdsa::Pair::from_seed_slice(&[7u8;32]); }\n')]));
    unchanged();
    await assert.rejects(check, /source hash drift/); negatives++;
    await writeFile(source, original);
  }
  for (const path of ['vendor/macro', 'vendor/proc']) {
    const source = join(workspace, path, 'src/lib.rs');
    const original = await readFile(source);
    const changed = original.toString().replace('pub fn public() {}',
      'pub fn secret() { let _ = sp_core::ecdsa::Pair::from_seed_slice(&[7;32]); }')
      .replace('from_seed_slice($s)', 'from_seed_slice(&[8;32])');
    assert.notEqual(changed, original.toString());
    await writeFile(source, changed);
    unchanged(); await assert.rejects(check, /source hash drift/); negatives++;
    await writeFile(source, original);
  }
  const extra = join(workspace, 'vendor/transitive/src/added.rs');
  await writeFile(extra, 'pub fn seed() { let _ = crate::sp_core::ecdsa::Pair::from_seed_slice(&[9;32]); }');
  unchanged(); await assert.rejects(check, /inventory drift/); negatives++;
  await rename(extra, extra + '.renamed');
  await assert.rejects(check, /inventory drift/); negatives++;
  await rm(extra + '.renamed');
  const nestedTarget = join(workspace, 'vendor/transitive/src/target/seed.rs');
  await put(nestedTarget, 'pub fn seed() {}');
  await assert.rejects(check, /inventory drift/); negatives++;
  await rm(dirname(nestedTarget), { recursive: true });
  const generatorInput = join(workspace, 'vendor/generator/constructor.tokens');
  await writeFile(generatorInput, 'pub fn newly_secret() { let _ = sp_core::ecdsa::Pair::from_seed_slice(&[7;32]); }');
  unchanged(); await assert.rejects(check, /source hash drift/); negatives++;
  await writeFile(generatorInput, 'pub fn public() {}');
  const removed = join(workspace, 'vendor/relay/src/lib.rs');
  const original = await readFile(removed);
  await rm(removed); await assert.rejects(check, /inventory drift/); negatives++;
  await writeFile(removed, original);
  // The same selector refuses source ambiguity, unknown orphans and source swaps.
  for (const mutate of [m => { m.packages.push({ ...m.packages[0] }); },
    m => { m.packages[0].manifest_path = join(dirname(m.packages[0].manifest_path), 'other/Cargo.toml'); }]) {
    const changed = structuredClone(metadata); mutate(changed);
    assert.throws(() => rootCallerSelection(changed, trees), /identity mismatch/); negatives++;
  }
  assert.throws(() => rootCallerSelection(metadata, { all: '0unknown v0.0.0|\n' }), /identity mismatch/); negatives++;
  await check(); positives++;
  console.log(`[SUCCESS] Root nonworkspace consumer self-test: ${positives} positive, ${negatives} negative; direct/transitive public constructors, reexports, ordinary macros, build generator inputs; unchanged metadata and all seven trees`);
}

async function selfTestNativePreparation(directory) {
  const { mkdir, symlink } = await import('node:fs/promises');
  let positives = 0;
  let negatives = 0;
  const reject = (fn, pattern) => { assert.throws(fn, pattern); negatives++; };
  const rejectAsync = async (fn, pattern) => { await assert.rejects(fn, pattern); negatives++; };
  const root = join(directory, 'source-preparation');
  const sdk = join(root, 'sdk');
  const vendor = join(root, 'vendor');
  const external = join(root, 'external');
  for (const owner of [sdk, vendor, external]) await mkdir(owner, { recursive: true });
  await writeFile(join(sdk, 'Cargo.toml'), 'profile and target inputs');
  await writeFile(join(vendor, 'backport.rs'), 'reviewed correction');
  for (const name of ['direct.rs', 'transitive.rs', 'macro.rs', 'generator.dat'])
    await writeFile(join(external, name), 'reviewed consumer input');
  const pkg = { id: 'registry#zstd-safe@6.0.6', name: 'zstd-safe', version: '6.0.6', source: registrySource,
    manifest_path: join(external, 'Cargo.toml') };
  const metadata = { packages: [pkg] };
  const selected = new Map([[pkg.id, ['std']]]);
  const seal = { sourceRoot: root, sourceInventories: { sdk: await nativePreparationInventory(sdk),
    vendor: await nativePreparationInventory(vendor) }, consumers: [{ id: pkg.id, root: external, owner: null,
      files: await nativePreparationInventory(external) }] };
  const row = { id: 'RUSTSEC-2026-0397', packageId: pkg.id, name: pkg.name, version: pkg.version,
    source: registrySource, rawSources: [registrySource], decision: 'selected-prerequisite' };
  const entry = { id: row.id, ecosystem: 'cargo', reachability: 'source-prerequisite-excluded',
    sourceCallerNative: { stage: nativePreparationStage, proof: 'nativeSourcePreparationProof' } };
  const findings = [{ advisory: { id: row.id }, package: { name: pkg.name, version: pkg.version, source: registrySource } }];
  const preflight = { schema: 1, stage: 'accepted-composition-source-preflight', capturedAt: 'fixture', qualification: 'UNQUALIFIED',
    sourceRoot: root, workspaceRoot: sdk, evidenceRoot: root, identity: {},
    role: { package: nativeRole.package, features: [nativeRole.features], target: nativeRole.target, edges: nativeRole.edges,
      toolchain: nativeRole.toolchain, profile: nativeRole.profile },
    configuration: { home: nativeHome, cargoHome: join(nativeHome, '.cargo'), rustupHome: join(nativeHome, '.rustup'),
      configs: [[join(sdk, '.cargo/config.toml'), nativeCargoConfigSha256]] }, sharedCaches: [], producers: [],
    proofs: nativePreparationProofFiles.map(path => [path, '0'.repeat(64)]) };
  const proof = { schema: 1, stage: nativePreparationStage, preflight,
    audits: { actual: 'fixture', registryEquivalent: 'fixture', versionLock: 'fixture' },
    review: { composition: 'fixture', dnsMisc: 'fixture', io: 'fixture', provider: 'fixture', scope: 'fixture' }, rows: [row] };
  validateNativePreparationProof(proof, true, sha256(canonical(proof))); positives++;
  const auditFixture = () => ({ database: { 'last-commit': 'fixed', 'last-updated': 'fixed', 'advisory-count': 2 },
    settings: { ignore: [], informational_warnings: ['unmaintained', 'unsound'] }, lockfile: { 'dependency-count': 2 },
    vulnerabilities: { count: 2, found: true, list: [
      { advisory: { id: 'RUSTSEC-2026-0001', title: 'first' },
        package: { name: 'first', version: '1.0.0', source: registrySource, checksum: 'first' },
        versions: { patched: ['>=1.1.0', '<2.0.0'], unaffected: [] }, parents: ['a', 'b'] },
      { advisory: { id: 'RUSTSEC-2026-0002', title: 'second' },
        package: { name: 'second', version: '2.0.0', source: registrySource, checksum: 'second' },
        versions: { patched: ['>=2.1.0'], unaffected: [] }, parents: ['b'] },
    ] }, warnings: { unsound: [{ advisory: { id: 'warning-a' } }, { advisory: { id: 'warning-b' } }] } });
  const audits = { cargo: auditFixture(), projectedAudit: auditFixture(), projectedLock: 'fixed' };
  audits.projectedAudit.vulnerabilities.list[0].package.source = null;
  const auditProof = { audits: { actual: nativePreparationAuditIdentity(audits.cargo),
    registryEquivalent: nativePreparationAuditIdentity(audits.projectedAudit), versionLock: sha256(audits.projectedLock) } };
  const unchangedAudits = structuredClone(audits);
  assertNativePreparationAudits(auditProof, audits); positives++;
  assert.deepEqual(audits, unchangedAudits);
  for (const role of ['cargo', 'projectedAudit']) {
    const reordered = structuredClone(audits);
    reordered[role].vulnerabilities.list.reverse();
    assertNativePreparationAudits(auditProof, reordered); positives++;
    for (const change of [a => { a.database['last-commit'] = 'new'; },
      a => { a.database['last-updated'] = 'new'; }, a => { a.database['advisory-count']++; },
      a => { a.settings.ignore.push('ignore'); }, a => { a.settings.informational_warnings.reverse(); },
      a => { a.lockfile['dependency-count']++; }, a => { a.vulnerabilities.count++; },
      a => { a.vulnerabilities.found = false; }, a => { a.extra = true; },
      a => { a.vulnerabilities.extra = true; }, a => { a.warnings.unsound.reverse(); },
      a => { a.vulnerabilities.list[0].package.name = 'changed'; },
      a => { a.vulnerabilities.list[0].package.version = 'changed'; },
      a => { a.vulnerabilities.list[0].package.source = 'changed'; },
      a => { a.vulnerabilities.list[0].package.checksum = 'changed'; },
      a => { a.vulnerabilities.list[0].advisory.id = 'changed'; },
      a => { a.vulnerabilities.list[0].advisory.title = 'changed'; },
      a => { a.vulnerabilities.list[0].versions.patched[0] = 'changed'; },
      a => { a.vulnerabilities.list[0].versions.patched.reverse(); },
      a => { a.vulnerabilities.list[0].parents.reverse(); },
      a => { a.vulnerabilities.list.pop(); },
      a => { a.vulnerabilities.list.push(structuredClone(a.vulnerabilities.list[0])); a.vulnerabilities.count++; },
      a => { a.vulnerabilities.list[1] = structuredClone(a.vulnerabilities.list[0]); },
      a => { a.vulnerabilities.list[0].extra = true; }]) {
      const a = structuredClone(audits); change(a[role]);
      reject(() => assertNativePreparationAudits(auditProof, a), /audit\/database\/settings identity drift/);
    }
    for (const list of [undefined, null, {}]) {
      const a = structuredClone(audits); a[role].vulnerabilities.list = list;
      reject(() => assertNativePreparationAudits(auditProof, a), /vulnerability list required/);
    }
  }
  const swapped = { ...audits, cargo: audits.projectedAudit, projectedAudit: audits.cargo };
  reject(() => assertNativePreparationAudits(auditProof, swapped), /audit\/database\/settings identity drift/);
  reject(() => assertNativePreparationAudits(auditProof, { ...audits, projectedLock: 'new' }), /audit\/database\/settings identity drift/);
  await rejectAsync(() => boundFile(root, { path: 'source-tools.txt', sha256: '0'.repeat(64) }), /ENOENT/);
  await writeFile(join(root, 'source-tools.txt'), 'compiler and default cfg');
  const toolProof = { path: 'source-tools.txt', sha256: sha256('compiler and default cfg') };
  await boundFile(root, toolProof); positives++;
  await writeFile(join(root, 'source-tools.txt'), 'injected cfg');
  await rejectAsync(() => boundFile(root, toolProof), /source hash drift/);
  await rm(join(root, 'source-tools.txt'));
  const policyPath = join(root, 'policy.json');
  const sharedPath = join(root, 'shared.json');
  await writeFile(policyPath, '{}');
  await writeFile(sharedPath, '{}');
  assert.deepEqual((await loadNativePreparationProof({ path: 'shared.json' }, policyPath)).proof, {}); positives++;
  for (const path of ['../shared.json', sharedPath, './shared.json'])
    await rejectAsync(() => loadNativePreparationProof({ path }, policyPath), /canonical owner-relative|outside owner/);
  await rejectAsync(() => loadNativePreparationProof({ path: 'sdk' }, policyPath), /nonregular file/);
  await symlink('shared.json', join(root, 'shared-link.json'));
  await rejectAsync(() => loadNativePreparationProof({ path: 'shared-link.json' }, policyPath), /symlink/);
  const fifo = join(root, 'shared.fifo');
  const created = spawnSync('mkfifo', [fifo], { encoding: 'utf8', timeout: 3000 });
  assert.equal(created.status, 0, `FIFO fixture unavailable: ${created.error ?? created.stderr}`);
  // Bound the regression in a child: a read-before-stat bug must fail, not hang the suite.
  const fifoCheck = spawnSync(process.execPath, ['--input-type=module', '-e', `
import assert from 'node:assert/strict';
import { readFile, realpath, lstat } from 'node:fs/promises';
import { dirname, resolve, relative } from 'node:path';
import { createHash } from 'node:crypto';
const sha256 = value => createHash('sha256').update(value).digest('hex');
${exactKeys.toString()}
${regularOwnerFile.toString()}
${boundFile.toString()}
${loadNativePreparationProof.toString()}
await assert.rejects(() => loadNativePreparationProof({ path: 'shared.fifo' }, ${JSON.stringify(policyPath)}), /nonregular file/);
`], { encoding: 'utf8', timeout: 3000 });
  assert.equal(fifoCheck.error, undefined, `FIFO proof refusal hung: ${fifoCheck.error}`);
  assert.equal(fifoCheck.status, 0, `FIFO proof refusal failed: ${fifoCheck.stderr}`); negatives++;
  // No fixture or caller hash can become production authority.
  reject(() => validateNativePreparationProof(proof, true), /unreviewed Native/);
  reject(() => validateNativePreparationProof(proof, false, sha256(canonical(proof))), /cannot transfer to Root/);
  for (const change of [p => { delete p.preflight; }, p => { p.unknown = true; }, p => { p.stage = 'BUILD'; },
    p => { p.schema = 2; }]) {
    const p = structuredClone(proof); change(p);
    reject(() => validateNativePreparationProof(p, true, sha256(canonical(p))), /unknown or missing|unknown Native/);
  }
  for (const change of [p => { p.preflight.stage = 'prepared'; }, p => { p.preflight.proofs.pop(); },
    p => { p.preflight.proofs = p.preflight.proofs.filter(([name]) => name !== 'source-tools.txt'); },
    p => { p.preflight.unknown = true; }, p => { p.audits.unknown = true; }, p => { p.preflight.role.target = 'all'; },
    p => { p.preflight.role.profile = 'release'; }, p => { p.preflight.configuration.home = '.'; },
    p => { p.rows[0].id = 'RUSTSEC-2026-0387'; }, p => { p.rows[0].decision = 'safe'; }]) {
    const p = structuredClone(proof); change(p);
    reject(() => validateNativePreparationProof(p, true, sha256(canonical(p))), /unknown|mismatch|B approval|proof coverage/);
  }
  assertNativePreparationInstances([entry], [row], findings, metadata, selected); positives++;
  const absent = { ...row, decision: 'normal-absent' };
  assertNativePreparationInstances([entry], [absent], findings, metadata, new Map()); positives++;
  for (const [rows, data, selection, pattern] of [
    [[row, row], metadata, selected, /duplicate or overlapping/],
    [[row, absent], metadata, selected, /duplicate or overlapping/],
    [[absent], metadata, selected, /selected\/normal-absent/],
    [[row], { packages: [] }, selected, /actual role package ID/],
    [[row], { packages: [{ ...pkg, source: null }] }, selected, /package\/source mismatch/],
    [[{ ...row, rawSources: [registrySource, registrySource] }], metadata, selected, /coverage mismatch/],
    [[{ ...row, id: 'stale' }], metadata, selected, /exact advisory coverage/],
  ]) reject(() => assertNativePreparationInstances([entry], rows, findings, data, selection), pattern);
  for (const change of [e => { e.sourceCallerNative.stage = 'UNKNOWN'; }, e => { e.sourceCallerNative.proof = 'arbitrary'; },
    e => { e.sourceCallerNative.sha256 = '0'.repeat(64); }, e => { e.reachability = 'lockfile-only'; }]) {
    const e = structuredClone(entry); change(e);
    reject(() => assertNativePreparationInstances([e], [row], findings, metadata, selected), /unknown|classification/);
  }
  await checkNativePreparationSeal(seal, metadata, selected); positives++;
  for (const name of ['direct.rs', 'transitive.rs', 'macro.rs', 'generator.dat']) {
    const path = join(external, name);
    await writeFile(path, 'new unsafe operation; metadata/features/normal tree unchanged');
    await rejectAsync(() => checkNativePreparationSeal(seal, metadata, selected), /external consumer source drift/);
    await writeFile(path, 'reviewed consumer input');
  }
  for (const [owner, name] of [[sdk, 'new.rs'], [vendor, 'new.rs'], [external, 'new.rs']]) {
    const path = join(owner, name);
    await writeFile(path, 'new input');
    await rejectAsync(() => checkNativePreparationSeal(seal, metadata, selected), /source drift/);
    await rm(path);
  }
  await rm(join(external, 'direct.rs'));
  await rejectAsync(() => checkNativePreparationSeal(seal, metadata, selected), /external consumer source drift/);
  await symlink('transitive.rs', join(external, 'direct.rs'));
  await rejectAsync(() => checkNativePreparationSeal(seal, metadata, selected), /external consumer source drift/);
  await rm(join(external, 'direct.rs'));
  await writeFile(join(external, 'direct.rs'), 'reviewed consumer input');
  await checkNativePreparationSeal(seal, metadata, selected); positives++;
  await writeFile(join(vendor, 'backport.rs'), 'lost source correction');
  await rejectAsync(() => checkNativePreparationSeal(seal, metadata, selected), /vendor source drift/);
  await writeFile(join(vendor, 'backport.rs'), 'reviewed correction');
  await rejectAsync(() => checkNativePreparationSeal({ ...seal, consumers: [] }, metadata, selected), /consumer coverage/);
  await rejectAsync(() => checkNativePreparationSeal({ ...seal, consumers: [...seal.consumers, ...seal.consumers] }, metadata, selected), /consumer coverage/);
  const environment = { HOME: nativeHome, CARGO_HOME: join(nativeHome, '.cargo'), RUSTUP_HOME: join(nativeHome, '.rustup') };
  checkNativePreparationEnvironment(environment); positives++;
  for (const key of ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WRAPPER', 'CARGO_PROFILE_PRODUCTION_LTO',
    'CARGO_PROFILE_PRODUCTION_CODEGEN_UNITS', 'CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER', 'WASM_BUILD_TOOLCHAIN',
    'SKIP_WASM_BUILD', 'RUST_TARGET_PATH', 'SUBSTRATE_RUNTIME_TARGET', 'HOME', 'CARGO_HOME', 'RUSTUP_HOME'])
    reject(() => checkNativePreparationEnvironment({ ...environment, [key]: '.' }), /unsupported Native/);
  assert.deepEqual(nativePreparationSelection(metadata, '0zstd-safe v6.0.6|std\n'), selected); positives++;
  assert.deepEqual(nativePreparationSelection(metadata, '0zstd-safe v6.0.6|std\n1zstd-safe v6.0.6|alloc,std\n'),
    new Map([[pkg.id, ['alloc', 'std']]])); positives++;
  for (const [name, version, feature] of [['zstd-safe', '6.0.6', 'experimental'], ['zstd-safe', '6.0.6', 'zstdmt'],
    ['zstd', '0.12.4', 'zdict_builder'], ['zstd-sys', '2.1.2', 'legacy'], ['hickory-resolver', '0.24.2', 'dnssec'],
    ['hickory-proto', '0.24.4', 'text-parsing'], ['keccak', '0.1.4', 'asm'], ['rand', '0.8.5', 'log'], ['rand', '0.9.0', 'log']])
    reject(() => nativePreparationSelection({ packages: [{ ...pkg, name, version }] }, `0${name} v${version}|${feature}\n`), /prerequisite activated/);
  console.log(`[SUCCESS] Native SOURCEPREPARATION self-test: ${positives} positive, ${negatives} negative; source-only shared proof, full input seals, no binary receipts`);
}

if (process.argv[2] === '--self-test') {
  assert.ok([3, 4].includes(process.argv.length), 'Usage: --self-test [REVIEWED_ROOT_PROOF_JSON]');
  const directory = await mkdtemp(join(tmpdir(), 'deos-dependency-self-test.'));
  await selfTestNativePreparation(directory);
  await selfTestNativeConfiguration(directory);
  selfTestNativeInstances();
  await selfTestRootCaller(directory, process.argv[3]);
  await selfTestRootConsumers(directory);
  const date = new Date().toISOString().slice(0, 10);
  const advisory = 'RUSTSEC-SELFTEST-0001';
  const exception = {
    ecosystem: 'cargo', id: advisory, expires: date,
    reachability: 'source-prerequisite-excluded',
    reason: 'Fixture-only independently reviewed exclusion; no package safety assertion.',
  };
  const finding = { advisory: { id: advisory }, package: { name: 'fixture', version: '1.0.0' } };
  const audit = list => ({
    vulnerabilities: { list, count: list.length, found: list.length > 0 },
    database: { 'last-commit': 'fixture', 'last-updated': date },
    settings: { ignore: [], target_arch: [], target_os: [], severity: null,
      informational_warnings: ['unsound'] },
    lockfile: { 'dependency-count': 2 },
  });
  const actualLock = join(directory, 'Cargo.lock');
  const metadataPath = join(directory, 'metadata.json');
  const projectionPath = join(directory, 'projected.lock');
  const manifest = join(directory, 'Cargo.toml');
  const patched = join(directory, 'patched.rs');
  const lock = 'version = 4\n\n[[package]]\nname = "fixture"\nversion = "1.0.0"\ndependencies = [\n "workspace",\n]\n\n[[package]]\nname = "workspace"\nversion = "1.0.0"\n';
  const metadata = {
    packages: [
      { id: 'fixture', name: 'fixture', version: '1.0.0', source: null, manifest_path: manifest },
      { id: 'workspace', name: 'workspace', version: '1.0.0', source: null },
    ], workspace_members: ['workspace'], resolve: { nodes: [{ id: 'fixture', features: [] }, { id: 'workspace', features: [] }] },
  };
  const base = {
    cargo: audit([]), projected: audit([finding]),
    npm: { metadata: { vulnerabilities: { critical: 0 } } },
    ledger: { formatVersion: 1, reviewedAt: date, exceptions: [exception], licenseExceptions: [] },
    lock: { packages: {} },
  };
  const cli = args => spawnSync(process.execPath, [process.argv[1], ...args], { encoding: 'utf8' });
  const invoke = async data => {
    const names = ['cargo', 'projected', 'npm', 'ledger', 'lock'];
    const paths = names.map(name => join(directory, name + '.json'));
    await Promise.all(paths.map((path, i) => writeFile(path, JSON.stringify(data[names[i]]))));
    return cli([...paths, actualLock, metadataPath, projectionPath, directory]);
  };
  let positives = 0;
  let negatives = 0;
  const expect = (result, status, pattern) => {
    assert.equal(result.status, status, result.stderr);
    if (pattern) assert.match(result.stderr, pattern);
    if (status === 0) positives++; else negatives++;
  };
  const reject = async (change, pattern) => {
    const data = structuredClone(base);
    change(data);
    expect(await invoke(data), 1, pattern);
  };
  try {
    await Promise.all([
      writeFile(actualLock, lock), writeFile(metadataPath, JSON.stringify(metadata)),
      writeFile(manifest, '[package]\nname = "fixture"\nversion = "1.0.0"\n'),
      writeFile(patched, '// fixture backport\n'),
    ]);
    expect(cli(['--project', actualLock, metadataPath, projectionPath]), 0);
    const projection = await readFile(projectionPath, 'utf8');
    assert.equal(projection.replace('source = "registry+https://github.com/rust-lang/crates.io-index"\n', ''), lock);
    expect(await invoke(base), 0); // A hidden local-path finding is retained by the union.
    await reject(d => { d.ledger.exceptions = []; }, /unreviewed material dependency finding/);
    await reject(d => { d.ledger.exceptions[0].reachability = 'native-only'; }, /unknown reachability/);
    await reject(d => { d.projected.database['last-commit'] = 'other'; }, /database identity mismatch/);
    await reject(d => { delete d.projected.database; }, /database identity is missing/);
    await reject(d => { d.projected.vulnerabilities.count = 0; }, /summary mismatch/);
    await reject(d => { d.projected.settings.ignore = [advisory]; }, /ignored Cargo/);
    await reject(d => { d.projected.settings.informational_warnings = []; }, /unsound Cargo advisories are required/);
    await reject(d => { d.projected.lockfile['dependency-count'] = 1; }, /lock graph count mismatch/);
    await reject(d => { d.projected = audit([]); }, /stale dependency exception/);
    await reject(d => {
      d.npm.vulnerabilities = { fixture: { severity: 'critical' } };
      d.ledger.exceptions.push({ ...exception, ecosystem: 'npm', id: 'fixture' });
    }, /npm critical advisory remains present even if listed/);
    await reject(d => { d.npm.metadata.vulnerabilities.critical = 1; }, /npm critical/);
    await reject(d => { d.ledger.exceptions.push(exception); }, /duplicate exception/);
    await reject(d => { d.ledger.exceptions[0].expires = '2000-01-01'; }, /expired/);
    const prerequisite = structuredClone(base);
    prerequisite.ledger.exceptions[0].excludedFeatures = [{ name: 'fixture', version: '1.0.0', source: null, features: ['dnssec'] }];
    expect(await invoke(prerequisite), 0);
    const substitutedSource = structuredClone(prerequisite);
    substitutedSource.ledger.exceptions[0].excludedFeatures[0].source = 'registry+https://github.com/rust-lang/crates.io-index';
    expect(await invoke(substitutedSource), 1, /feature exclusion source identity mismatch/);
    const enabled = structuredClone(metadata);
    enabled.resolve.nodes[0].features = ['dnssec'];
    await writeFile(metadataPath, JSON.stringify(enabled));
    expect(await invoke(prerequisite), 1, /excluded feature activated/);
    await writeFile(metadataPath, JSON.stringify(metadata));
    const backport = structuredClone(base);
    backport.ledger.exceptions[0].reachability = 'source-backport-reviewed';
    backport.ledger.exceptions[0].reviewedSources = [{
      name: 'fixture', version: '1.0.0',
      files: await Promise.all(['Cargo.toml', 'patched.rs'].map(async path => ({
        path, sha256: createHash('sha256').update(await readFile(join(directory, path))).digest('hex'),
      }))),
    }];
    expect(await invoke(backport), 0);
    backport.ledger.exceptions[0].reviewedSources[0].files[1].sha256 = '0'.repeat(64);
    expect(await invoke(backport), 1, /reviewed source hash mismatch/);
    await reject(d => { d.ledger.exceptions[0].reachability = 'source-backport-reviewed'; }, /requires reviewedSources/);
    const primaryOnly = structuredClone(base);
    primaryOnly.cargo = audit([finding]);
    primaryOnly.projected = audit([]);
    expect(await invoke(primaryOnly), 0);
    const noManifest = structuredClone(backport);
    noManifest.ledger.exceptions[0].reviewedSources[0].files.shift();
    expect(await invoke(noManifest), 1, /requires Cargo.toml and patched files/);
    for (const altered of [projection.replace('version = "1.0.0"', 'version = "1.0.1"'),
      projection.replace(' "workspace",', ' "other",')]) {
      await writeFile(projectionPath, altered);
      expect(await invoke(base), 1, /projection graph\/version proof failed/);
    }
    await writeFile(metadataPath, JSON.stringify({ ...metadata, resolve: null }));
    expect(await invoke(base), 1, /full locked Cargo metadata/);
    await writeFile(metadataPath, JSON.stringify(metadata));
    await rm(projectionPath);
    expect(await invoke(base), 1, /ENOENT/);
    expect(cli(['--project', actualLock, metadataPath, actualLock]), 1, /cannot overwrite/);
    const externalProjection = join(directory, 'external.lock');
    expect(cli(['--project-external-workspace', actualLock, metadataPath, externalProjection]), 0);
    assert.equal((await readFile(externalProjection, 'utf8')).match(/^source = /gm).length, 2);
    assert.equal((await readFile(externalProjection, 'utf8')).replace(/^source = "[^"]+"\n/gm, ''), lock);
    await writeFile(projectionPath, await readFile(externalProjection, 'utf8'));
    const files = ['cargo', 'projected', 'npm', 'ledger', 'lock'].map(name => join(directory, name + '.json'));
    await invoke(base); // Restore genuine fixture reports, retaining the broader view.
    expect(cli([...files, actualLock, metadataPath, projectionPath, directory]), 1, /projection graph\/version proof failed/);
    expect(cli([...files, actualLock, metadataPath, projectionPath, directory, '--external-workspace']), 0);
    await writeFile(files[1], JSON.stringify(audit([finding, {
      advisory: { id: 'RUSTSEC-SELFTEST-EXTERNAL' }, package: { name: 'workspace', version: '1.0.0' },
    }])));
    expect(cli([...files, actualLock, metadataPath, projectionPath, directory, '--external-workspace']), 1,
      /unreviewed material dependency finding cargo:RUSTSEC-SELFTEST-EXTERNAL/);
    expect(cli(['one', 'two', 'three', 'four']), 2, /Usage/);
    assert.equal(await readFile(actualLock, 'utf8'), lock);
    // A real offline Cargo fixture distinguishes dev-unified metadata from role
    // selection. No test hook or supplied tree can replace current Cargo output.
    const root = join(directory, 'role');
    const { mkdir } = await import('node:fs/promises');
    await mkdir(join(root, 'template', 'src'), { recursive: true });
    await mkdir(join(root, 'fixture', 'src'), { recursive: true });
    const rootManifest = '[workspace]\nmembers = ["."]\nresolver = "2"\n[package]\nname = "root-role"\nversion = "1.0.0"\n[dev-dependencies]\nfixture = { path = "../fixture" }\n';
    await Promise.all([
      writeFile(join(root, 'template', 'Cargo.toml'), rootManifest),
      writeFile(join(root, 'template', 'rust-toolchain.toml'), '[toolchain]\nchannel = "1.94.1"\n'),
      writeFile(join(root, 'template', 'src', 'lib.rs'), ''),
      writeFile(join(root, 'fixture', 'Cargo.toml'), '[workspace]\n[package]\nname = "fixture"\nversion = "1.0.0"\n'),
      writeFile(join(root, 'fixture', 'src', 'lib.rs'), ''),
    ]);
    const cargoRun = args => {
      const result = spawnSync('cargo', ['+1.94.1', ...args], { cwd: join(root, 'template'), encoding: 'utf8',
        env: { ...process.env, CARGO_TARGET_DIR: join(root, 'target'), CARGO_TERM_COLOR: 'never' } });
      assert.equal(result.status, 0, result.stderr);
      return result.stdout;
    };
    cargoRun(['generate-lockfile', '--offline']);
    const full = JSON.parse(cargoRun(['metadata', '--locked', '--offline', '--all-features', '--format-version', '1']));
    assert.equal(full.packages.length, 2);
    const tree = cargoRun(rootTreeArgs.slice(1));
    assert(!tree.includes('fixture'));
    const proof = { ...rootRole, treeSha256: sha256(tree.replaceAll(root, '$REPO')),
      inputs: await Promise.all(['template/Cargo.lock', 'template/Cargo.toml', 'template/rust-toolchain.toml']
        .map(async path => ({ path, sha256: sha256(await readFile(join(root, path))) }))) };
    const roleEntry = { ...exception, reachability: 'root-normal-build-absent',
      absentFromRootNormalBuild: { name: 'fixture', version: '1.0.0', source: null } };
    const roleFindings = [{ ...finding, package: { ...finding.package, source: null } }];
    const roleLock = join(root, 'template', 'Cargo.lock');
    const enforce = (entries = [roleEntry], p = proof, m = full, external = false, path = roleLock) =>
      enforceRootAbsence(entries, p, roleFindings, m, root, path,
        cargoRoleLock, external);
    const cargoRoleLock = await readFile(roleLock, 'utf8');
    await enforce(); positives++;
    const rejectedRole = async (act, pattern) => { await assert.rejects(act, pattern); negatives++; };
    for (const [key, value] of [['role', 'sdk-omni'], ['target', 'x86_64-unknown-linux-gnu'],
      ['features', 'default'], ['edges', 'normal,build,dev'], ['artifact', 'unbound'], ['unknown', true]]) {
      await rejectedRole(() => enforce([roleEntry], { ...proof, [key]: value }), /unsupported root role|unknown or missing/);
    }
    await rejectedRole(() => enforce([roleEntry], { ...proof, inputs: [...proof.inputs, proof.inputs[0]] }), /exact unique source inputs/);
    await rejectedRole(() => enforce([roleEntry], null), /requires rootNormalBuildProof/);
    await rejectedRole(() => enforce([roleEntry], proof, full, true), /cannot transfer/);
    await rejectedRole(() => enforce([roleEntry], proof, { ...full, workspace_root: directory }), /metadata role mismatch/);
    await rejectedRole(() => enforce([roleEntry], proof, full, false, actualLock), /lock role mismatch/);
    const stale = structuredClone(full);
    stale.resolve.nodes[0].features.push('unselected');
    await rejectedRole(() => enforce([roleEntry], proof, stale), /stale full metadata/);
    const wrongSource = { ...roleEntry, absentFromRootNormalBuild: { ...roleEntry.absentFromRootNormalBuild, source: registrySource } };
    await rejectedRole(() => enforce([wrongSource]), /finding source identity mismatch/);
    await rejectedRole(() => enforce([{ ...roleEntry, absentFromRootNormalBuild: undefined }]), /requires package identity/);
    await rejectedRole(() => enforce([{ ...roleEntry, reachability: 'lockfile-only' }]), /conflicting root absence/);
    await writeFile(join(root, 'template', 'Cargo.toml'), rootManifest + '\n[features]\nnew-parent-feature = []\n');
    await rejectedRole(() => enforce(), /root proof input drift/);
    const changedFeatures = { ...proof, inputs: await Promise.all(proof.inputs.map(async file =>
      ({ ...file, sha256: sha256(await readFile(join(root, file.path))) }))) };
    const newFull = JSON.parse(cargoRun(['metadata', '--locked', '--offline', '--all-features', '--format-version', '1']));
    await rejectedRole(() => enforce([roleEntry], changedFeatures, newFull), /parent feature identity drift/);
    await writeFile(join(root, 'template', 'Cargo.toml'), rootManifest.replace('[dev-dependencies]', '[dependencies]'));
    const activated = JSON.parse(cargoRun(['metadata', '--locked', '--offline', '--all-features', '--format-version', '1']));
    const activatedTree = cargoRun(rootTreeArgs.slice(1));
    const activatedProof = { ...proof, treeSha256: sha256(activatedTree.replaceAll(root, '$REPO')),
      inputs: await Promise.all(proof.inputs.map(async file => ({ ...file, sha256: sha256(await readFile(join(root, file.path))) }))) };
    await rejectedRole(() => enforce([roleEntry], activatedProof, activated), /excluded root package activated/);
    await writeFile(projectionPath, projection);
    await invoke(base);
    for (const target of ['all', 'x86_64-unknown-linux-gnu']) {
      const raw = JSON.stringify({ ...base.ledger, rootNormalBuildProof: proof })
        .replace('"target":"all"', `"target":"all","target":"${target}"`);
      await writeFile(join(directory, 'ledger.json'), raw);
      expect(cli([...files, actualLock, metadataPath, projectionPath, directory]), 1, /duplicate JSON object key: target/);
    }
    await reject(d => { d.ledger.exceptions[0].reachability = 'root-normal-build-absent'; }, /requires rootNormalBuildProof/);
    await reject(d => { d.ledger.rootNormalBuildProof = proof; }, /stale root role proof/);
    console.log('[SUCCESS] Dependency CLI self-test: ' + positives + ' positive, ' + negatives +
      ' negative cases; isolated projection, union, DB, source bindings and fresh offline Cargo role enforcement');
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
  process.exit(0);
}

function fail(message) {
  console.error(`[ERROR] ${message}`);
  process.exitCode = 1;
}

function parseDate(value, label) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value ?? '')) {
    throw new Error(`${label} must be YYYY-MM-DD`);
  }
  return new Date(`${value}T00:00:00Z`);
}

async function readJson(path) {
  const raw = await readFile(path, 'utf8');
  const value = JSON.parse(raw);
  // JSON.parse silently keeps the last duplicate key, hiding conflicting proof
  // settings. Tokenize only after syntax validation; strings stay indivisible.
  const tokens = raw.match(/"(?:\\.|[^"\\])*"|[{}\[\]:,]|[^\s{}\[\]:,]+/g) ?? [];
  const objects = [];
  for (let index = 0; index < tokens.length; index++) {
    const token = tokens[index];
    if (token === '{' || token === '[') objects.push(token === '{' ? new Set() : null);
    else if (token === '}' || token === ']') objects.pop();
    else if (token.startsWith('"') && tokens[index + 1] === ':') {
      const key = JSON.parse(token);
      const keys = objects.at(-1);
      assert.ok(keys && !keys.has(key), `duplicate JSON object key: ${key}`);
      keys.add(key);
    }
  }
  return value;
}

// Only Cargo-generated lock syntax is supported. Reconstructing exactly the same
// bytes except inserted source lines proves names, versions and edges unchanged.
function projectLock(lock, metadata, externalWorkspace = false) {
  assert.ok(Array.isArray(metadata.packages) && Array.isArray(metadata.workspace_members) &&
    Array.isArray(metadata.resolve?.nodes), 'full locked Cargo metadata is required');
  const members = new Set(metadata.workspace_members);
  const resolved = new Set(metadata.resolve.nodes.map(node => node.id));
  assert.ok(metadata.packages.every(pkg => resolved.has(pkg.id)), 'incomplete resolved Cargo metadata');
  const sourceNull = new Set(metadata.packages.filter(pkg => pkg.source === null)
    .map(pkg => `${pkg.name}@${pkg.version}`));
  // An isolated upstream SDK workspace is third-party source too. This explicit
  // mode only broadens the registry-equivalent view; it never excludes findings.
  const locals = metadata.packages.filter(pkg => pkg.source === null &&
    (externalWorkspace || !members.has(pkg.id)));
  const keys = new Set(locals.map(pkg => `${pkg.name}@${pkg.version}`));
  assert.equal(keys.size, locals.length, 'ambiguous local package identity');
  const seen = new Set();
  const projected = lock.replace(/^\[\[package\]\]\n[\s\S]*?(?=^\[\[|$(?![\s\S]))/gm, block => {
    const name = /^name = "([^"\n]+)"$/m.exec(block)?.[1];
    const version = /^version = "([^"\n]+)"$/m.exec(block)?.[1];
    assert.ok(name && version, 'unsupported Cargo lock package syntax');
    const key = `${name}@${version}`;
    if (!/^source = /m.test(block)) {
      assert.ok(sourceNull.has(key), `source-null lock package missing from metadata: ${key}`);
    }
    if (!keys.has(key) || /^source = /m.test(block)) return block;
    assert.ok(!seen.has(key), 'duplicate local lock identity');
    assert.ok(!/^checksum = /m.test(block), 'local package unexpectedly has checksum');
    seen.add(key);
    return block.replace(`version = "${version}"\n`,
      `version = "${version}"\nsource = "${registrySource}"\n`);
  });
  assert.equal(seen.size, keys.size, 'metadata local packages missing from actual lock');
  return { projected, locals };
}

if (['--project', '--project-external-workspace'].includes(process.argv[2])) {
  assert.equal(process.argv.length, 6, 'Usage: --project or --project-external-workspace ACTUAL_LOCK FULL_METADATA_JSON TEMP_LOCK');
  const [, , , lockPath, metadataPath, output] = process.argv;
  assert.notEqual(resolve(lockPath), resolve(output), 'projection cannot overwrite actual lock');
  const { projected, locals } = projectLock(await readFile(lockPath, 'utf8'), await readJson(metadataPath),
    process.argv[2] === '--project-external-workspace');
  await writeFile(output, projected, { flag: 'wx' });
  console.log(`[INFO] Counterfactual version view: ${locals.length} local third-party packages; not source provenance`);
  process.exit(0);
}

const arguments_ = process.argv.slice(2);
const externalWorkspace = arguments_.at(-1) === '--external-workspace';
if (externalWorkspace) arguments_.pop();
const [cargoPath, projectedAuditPath, npmPath, exceptionPath, packageLockPath,
  lockPath, metadataPath, projectedLockPath, repoRoot] = arguments_;
if (arguments_.length !== 9) {
  console.error('Usage: dependency-provenance.mjs ACTUAL_AUDIT PROJECTED_AUDIT NPM_AUDIT EXCEPTIONS PACKAGE_LOCK ACTUAL_LOCK FULL_METADATA PROJECTED_LOCK REPO_ROOT [--external-workspace] | --project ACTUAL_LOCK FULL_METADATA TEMP_LOCK | --project-external-workspace ACTUAL_LOCK FULL_METADATA TEMP_LOCK | --self-test [REVIEWED_ROOT_PROOF_JSON]');
  process.exit(2);
}
const [cargo, projectedAudit, npm, ledger, packageLock, lock, metadata, projectedLock] = await Promise.all([
  readJson(cargoPath), readJson(projectedAuditPath), readJson(npmPath), readJson(exceptionPath),
  readJson(packageLockPath), readFile(lockPath, 'utf8'), readJson(metadataPath), readFile(projectedLockPath, 'utf8'),
]);
assert.notEqual(await realpath(cargoPath), await realpath(projectedAuditPath), 'both distinct audit files are required');
const { projected, locals } = projectLock(lock, metadata, externalWorkspace);
assert.equal(projectedLock, projected, 'projection graph/version proof failed');
for (const audit of [cargo, projectedAudit]) {
  assert.ok(audit.database?.['last-commit'] && audit.database?.['last-updated'], 'cargo advisory database identity is missing');
  assert.ok(Array.isArray(audit.vulnerabilities?.list), 'missing Cargo audit vulnerabilities');
  assert.equal(audit.vulnerabilities.count, audit.vulnerabilities.list.length, 'Cargo audit summary mismatch');
  assert.equal(audit.vulnerabilities.found, audit.vulnerabilities.list.length > 0, 'Cargo audit summary mismatch');
  assert.deepEqual(audit.settings?.ignore, [], 'ignored Cargo advisories are not accepted');
  assert.deepEqual(audit.settings?.target_arch, [], 'filtered Cargo advisories are not accepted');
  assert.deepEqual(audit.settings?.target_os, [], 'filtered Cargo advisories are not accepted');
  assert.equal(audit.settings?.severity, null, 'filtered Cargo advisories are not accepted');
  assert.ok(audit.settings?.informational_warnings?.includes('unsound'), 'unsound Cargo advisories are required');
  assert.equal(audit.lockfile?.['dependency-count'], [...lock.matchAll(/^\[\[package\]\]$/gm)].length,
    'Cargo audit lock graph count mismatch');
}
assert.deepEqual(cargo.database, projectedAudit.database, 'Cargo advisory database identity mismatch');
assert.deepEqual(cargo.settings, projectedAudit.settings, 'Cargo audit settings mismatch');
assert.deepEqual(cargo.lockfile, projectedAudit.lockfile, 'Cargo audit lock graph count mismatch');
console.log('[INFO] Unioning untouched actual audit with counterfactual version-view audit; projected identities are not source provenance');

if (
  ledger.formatVersion !== 1 ||
  !Array.isArray(ledger.exceptions) ||
  !Array.isArray(ledger.licenseExceptions)
) {
  throw new Error('unsupported dependency exception ledger');
}

const today = new Date();
today.setUTCHours(0, 0, 0, 0);
const reviewedAt = parseDate(ledger.reviewedAt, 'reviewedAt');
if (reviewedAt > today) fail('dependency review date is in the future');

function validateExpiry(entry, key) {
  if (typeof entry.reason !== 'string' || entry.reason.length < 40) {
    fail(`${key} lacks a material reachability rationale`);
  }
  const expiry = parseDate(entry.expires, `${key} expiry`);
  if (expiry < today) fail(`${key} expired on ${entry.expires}`);
  if (expiry.getTime() - reviewedAt.getTime() > 90 * 24 * 60 * 60 * 1000) {
    fail(`${key} exceeds the 90-day exception horizon`);
  }
}

const exceptions = new Map();
for (const entry of ledger.exceptions) {
  const key = `${entry.ecosystem}:${entry.id}`;
  if (exceptions.has(key)) fail(`duplicate exception ${key}`);
  if (!reachabilityClasses.has(entry.reachability)) {
    fail(`${key} has unknown reachability classification`);
  }
  validateExpiry(entry, key);
  exceptions.set(key, entry);
}

const findings = new Set();
const cargoFindings = [cargo, projectedAudit].flatMap(audit => [
  ...audit.vulnerabilities.list, ...(audit.warnings?.unsound ?? []),
]);
for (const finding of cargoFindings) {
  assert.ok(finding.advisory?.id && finding.package?.name && finding.package?.version,
    'malformed Cargo advisory finding');
  findings.add(`cargo:${finding.advisory.id}`);
}

// An affected local Root bigint cannot be waived with package files alone.
const rootSources = ledger.exceptions.filter(e => e.sourceCallerRoot !== undefined ||
  (!externalWorkspace && e.ecosystem === 'cargo' && e.id === 'RUSTSEC-2026-0387'));
for (const entry of rootSources) {
  assert.ok(!process.exitCode, 'invalid exception settings before Root caller proof');
  await enforceRootCaller(entry, cargoFindings, metadata, repoRoot, lockPath, lock, externalWorkspace);
}

const rootAbsences = ledger.exceptions.filter(e => e.reachability === 'root-normal-build-absent' ||
  e.absentFromRootNormalBuild !== undefined);
if (rootAbsences.length) {
  assert.ok(!process.exitCode, 'invalid exception settings before root proof');
  await enforceRootAbsence(rootAbsences, ledger.rootNormalBuildProof, cargoFindings, metadata,
    repoRoot, lockPath, lock, externalWorkspace);
} else {
  assert.equal(ledger.rootNormalBuildProof, undefined, 'stale root role proof');
}

const nativeSources = ledger.exceptions.filter(e => e.sourceCallerNative !== undefined);
const nativePreparation = nativeSources.filter(e => e.sourceCallerNative?.stage === nativePreparationStage);
if (nativePreparation.length) {
  assert.equal(nativeSources.length, nativePreparation.length, 'mixed Native proof stages');
  assert.equal(ledger.exceptions.filter(e => e.ecosystem === 'cargo').length, nativePreparation.length,
    'Native preparation cannot mix unbound Cargo decisions or approve Native0387 B');
  assert.ok(!process.exitCode, 'invalid exception settings before Native preparation proof');
  await enforceNativePreparation(nativePreparation, ledger.nativeSourcePreparationProof, cargoFindings, metadata,
    repoRoot, lockPath, lock, externalWorkspace, exceptionPath, { cargo, projectedAudit, projectedLock });
} else {
  assert.equal(ledger.nativeSourcePreparationProof, undefined, 'stale Native preparation proof');
  if (nativeSources.length) {
    assert.ok(!process.exitCode, 'invalid exception settings before Native source proof');
    await enforceNativeSources(nativeSources, cargoFindings, metadata, repoRoot, lockPath, lock, externalWorkspace);
  }
}

// A future independent review may bind a backport decision to these exact files.
// Hash agreement proves identity, not that the source fixes the advisory.
for (const entry of ledger.exceptions.filter(e => e.excludedFeatures !== undefined)) {
  assert.equal(entry.ecosystem, 'cargo', 'feature exclusion requires Cargo finding');
  assert.ok(Array.isArray(entry.excludedFeatures) && entry.excludedFeatures.length > 0,
    'feature exclusion requires package prerequisites');
  for (const exclusion of entry.excludedFeatures) {
    const packages = metadata.packages.filter(p => p.name === exclusion.name && p.version === exclusion.version);
    assert.equal(packages.length, 1, 'feature exclusion package identity mismatch');
    assert.ok(Object.hasOwn(exclusion, 'source'), 'feature exclusion source identity is required');
    assert.equal(packages[0].source, exclusion.source, 'feature exclusion source identity mismatch');
    const node = metadata.resolve.nodes.find(n => n.id === packages[0].id);
    assert.ok(Array.isArray(node?.features), 'resolved package features are required');
    assert.ok(Array.isArray(exclusion.features) && exclusion.features.length > 0,
      'excluded feature names are required');
    for (const feature of exclusion.features) {
      assert.ok(typeof feature === 'string' && feature.length > 0, 'invalid excluded feature');
      assert.ok(!node.features.includes(feature), `excluded feature activated: ${exclusion.name}/${feature}`);
    }
  }
}
for (const entry of ledger.exceptions.filter(e => !nativePreparation.includes(e) &&
  (e.reachability === 'source-backport-reviewed' || e.reviewedSources !== undefined))) {
  assert.equal(entry.ecosystem, 'cargo', 'source review requires Cargo finding');
  const affected = cargoFindings.filter(f => f.advisory.id === entry.id);
  const packages = new Map(affected.map(f => [`${f.package.name}@${f.package.version}`, f.package]));
  assert.ok(packages.size > 0, 'stale backport decision');
  assert.ok(Array.isArray(entry.reviewedSources), 'backport decision requires reviewedSources');
  assert.equal(entry.reviewedSources.length, packages.size, 'backport source coverage mismatch');
  const seen = new Set();
  const root = await realpath(repoRoot);
  for (const review of entry.reviewedSources) {
    const key = `${review.name}@${review.version}`;
    assert.ok(packages.has(key) && !seen.has(key), 'backport source package mismatch');
    seen.add(key);
    const pkg = locals.find(p => p.name === review.name && p.version === review.version);
    assert.ok(pkg, 'backport review requires actual local third-party source');
    const manifest = await realpath(pkg.manifest_path);
    const packageRoot = dirname(manifest);
    assert.ok(Array.isArray(review.files) && review.files.length >= 2,
      'backport review requires Cargo.toml and patched files');
    const files = new Set();
    for (const file of review.files) {
      assert.ok(typeof file.path === 'string' && /^[a-f0-9]{64}$/.test(file.sha256), 'invalid reviewed source binding');
      const path = await realpath(resolve(root, file.path));
      assert.equal(file.path, relative(root, path), 'reviewed source must be canonical repository-relative path');
      assert.ok(!file.path.startsWith('../'), 'reviewed source outside repository');
      assert.ok(path.startsWith(`${packageRoot}/`), 'reviewed source outside package');
      assert.ok(!files.has(path), 'duplicate reviewed source binding');
      files.add(path);
      assert.equal(createHash('sha256').update(await readFile(path)).digest('hex'), file.sha256,
        `reviewed source hash mismatch: ${file.path}`);
    }
    assert.ok(files.has(manifest), 'backport review requires Cargo.toml binding');
  }
}
for (const [name, finding] of Object.entries(npm.vulnerabilities ?? {})) {
  if (finding.severity === 'critical') fail('npm critical advisory remains present even if listed');
  if (finding.severity === 'high' || finding.severity === 'critical') {
    findings.add(`npm:${name}`);
  }
}

for (const finding of findings) {
  if (!exceptions.has(finding)) fail(`unreviewed material dependency finding ${finding}`);
}
for (const exception of exceptions.keys()) {
  if (!findings.has(exception)) fail(`stale dependency exception ${exception}`);
}

const allowedNpmLicenses = new Set([
  '(MIT OR CC0-1.0)',
  '0BSD',
  'Apache-2.0',
  'BSD-2-Clause',
  'BSD-3-Clause',
  'BlueOak-1.0.0',
  'CC-BY-3.0',
  'CC0-1.0',
  'GPL-3.0-or-later WITH Classpath-exception-2.0',
  'ISC',
  'MIT',
  'MPL-2.0',
  'Unlicense',
]);
const licenseExceptions = new Map();
for (const entry of ledger.licenseExceptions) {
  const key = `${entry.ecosystem}:${entry.id}`;
  validateExpiry(entry, key);
  licenseExceptions.set(key, entry);
}
const seenLicenseExceptions = new Set();
for (const [path, pkg] of Object.entries(packageLock.packages ?? {})) {
  if (!path) continue;
  const isLocalDescriptor =
    path === '.papi/descriptors' || path === 'node_modules/@polkadot-api/descriptors';
  if (isLocalDescriptor) continue;
  if (path.startsWith('node_modules/') && pkg.resolved?.startsWith('https://registry.npmjs.org/')) {
    if (!pkg.integrity) fail(`${path} lacks registry integrity in package-lock.json`);
  }
  if (allowedNpmLicenses.has(pkg.license)) continue;
  const key = `npm:${path.replace(/^node_modules\//, '')}@${pkg.version}`;
  if (licenseExceptions.has(key)) seenLicenseExceptions.add(key);
  else fail(`${path} has unreviewed npm license ${pkg.license ?? '<missing>'}`);
}
for (const key of licenseExceptions.keys()) {
  if (!seenLicenseExceptions.has(key)) fail(`stale license exception ${key}`);
}

if ((npm.metadata?.vulnerabilities?.critical ?? 0) > 0) {
  fail('npm critical advisory remains present even if listed');
}

if (!process.exitCode) {
  console.log(
    `[SUCCESS] Reviewed ${findings.size} material findings and npm license/integrity state; ` +
      `Cargo DB ${cargo.database['last-commit']}; no critical npm findings`,
  );
}
