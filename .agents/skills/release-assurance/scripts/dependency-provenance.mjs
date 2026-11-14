#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { lstat, mkdtemp, readdir, readFile, realpath, rm, writeFile } from 'node:fs/promises';
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
async function boundFile(root, file) {
  exactKeys(file, ['path', 'sha256'], 'Native file');
  assert.ok(typeof file.path === 'string' && file.path.length > 0, 'Native file path required');
  assert.match(file.sha256, /^[a-f0-9]{64}$/, 'invalid Native source digest');
  const path = resolve(root, file.path);
  assert.equal(relative(root, path), file.path, 'Native file must be canonical owner-relative');
  assert.ok(!file.path.startsWith('../'), 'Native file outside owner');
  assert.equal(await realpath(path), path, 'Native source symlink/path substitution');
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

if (process.argv[2] === '--self-test') {
  assert.equal(process.argv.length, 3);
  const directory = await mkdtemp(join(tmpdir(), 'deos-dependency-self-test.'));
  await selfTestNativeConfiguration(directory);
  selfTestNativeInstances();
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
  console.error('Usage: dependency-provenance.mjs ACTUAL_AUDIT PROJECTED_AUDIT NPM_AUDIT EXCEPTIONS PACKAGE_LOCK ACTUAL_LOCK FULL_METADATA PROJECTED_LOCK REPO_ROOT [--external-workspace] | --project ACTUAL_LOCK FULL_METADATA TEMP_LOCK | --project-external-workspace ACTUAL_LOCK FULL_METADATA TEMP_LOCK | --self-test');
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
if (nativeSources.length) {
  assert.ok(!process.exitCode, 'invalid exception settings before Native source proof');
  await enforceNativeSources(nativeSources, cargoFindings, metadata, repoRoot, lockPath, lock, externalWorkspace);
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
for (const entry of ledger.exceptions.filter(e => e.reachability === 'source-backport-reviewed' || e.reviewedSources !== undefined)) {
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
