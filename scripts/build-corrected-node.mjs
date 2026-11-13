import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import os from 'node:os';

const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const digest = file => hash(fs.readFileSync(file));
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const save = (file, value) => fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
const macro = 'substrate/primitives/api/proc-macro/src/impl_runtime_apis.rs';
const original = '5466016552eef8393ba2b15a51c5f23fadd6c5c5f874f2e5439f05bebd9758a8';
const corrected = 'c2d2f02bd4372b402fb15c2bc43046131d2e9a8f6b84ac91789e6507e557be3d';
const patches = ['crypto-bigint', 'hickory-proto', 'webpki-roots'];

function inventory(root, prefix = '') {
  return fs.readdirSync(path.join(root, prefix)).sort().flatMap(name => {
    if (!prefix && name === '.git') return [];
    const relative = path.join(prefix, name);
    const stat = fs.lstatSync(path.join(root, relative));
    if (stat.isSymbolicLink()) {
      const target = fs.readlinkSync(path.join(root, relative));
      assert(fs.realpathSync(path.join(root, relative)).startsWith(fs.realpathSync(root) + path.sep), `External frozen symlink: ${relative}`);
      return [[relative, 'symlink', target]];
    }
    if (stat.isDirectory()) return inventory(root, relative);
    assert(stat.isFile(), `Not a regular input: ${relative}`);
    return [[relative, digest(path.join(root, relative)), stat.mode & 0o111]];
  });
}

function patch(sdk, vendor) {
  const input = path.join(sdk, macro);
  const replacement = path.join(vendor, 'sp-api-proc-macro/src/impl_runtime_apis.rs');
  assert.equal(digest(input), original, 'Unexpected SDK macro');
  assert.equal(digest(replacement), corrected, 'Unexpected reviewed correction');
  const before = fs.readFileSync(input, 'utf8');
  const after = fs.readFileSync(replacement, 'utf8');
  const oldBlock = '\t\t\t\t\tstd::cell::RefCell::borrow_mut(&self.transaction_depth)\n\t\t\t\t\t\t.checked_sub(1)\n\t\t\t\t\t\t.expect("Transactions are opened and closed together; qed");';
  const newBlock = '\t\t\t\t\t{\n\t\t\t\t\t\tlet mut depth = std::cell::RefCell::borrow_mut(&self.transaction_depth);\n\t\t\t\t\t\t*depth = depth.checked_sub(1)\n\t\t\t\t\t\t\t.expect("Transactions are opened and closed together; qed");\n\t\t\t\t\t}';
  assert(before.includes(oldBlock));
  assert.equal(before.replace(oldBlock, newBlock), after,
    'Correction must only assign the checked decrement result');
  fs.copyFileSync(replacement, input);
  const manifest = path.join(sdk, 'Cargo.toml');
  const text = fs.readFileSync(manifest, 'utf8');
  assert(!/^\[patch\.crates-io\]/m.test(text), 'Unexpected existing patch table');
  fs.appendFileSync(manifest, '\n[patch.crates-io]\n' + patches.map(name =>
    `${name} = { path = "../vendor/${name}" }\n`).join(''));
}

function closure(metadata) {
  const nodes = new Map(metadata.resolve.nodes.map(node => [node.id, node]));
  const root = metadata.packages.find(pkg => pkg.name === 'polkadot-omni-node');
  assert(root, 'Missing Omni package');
  const reached = new Set();
  const visit = id => {
    if (reached.has(id)) return;
    reached.add(id);
    for (const dep of nodes.get(id).deps) {
      if (dep.dep_kinds.some(kind => kind.kind === null || kind.kind === 'build')) visit(dep.pkg);
    }
  };
  visit(root.id);
  return { nodes, reached, packages: metadata.packages.filter(pkg => reached.has(pkg.id)) };
}

function treeClosure(metadata, text) {
  const nodes = new Map();
  const stack = [];
  for (const line of text.split('\n')) {
    const match = /^(\d+)([^\d\s]\S*) v(\S+)(?: \(proc-macro\))?(?: \(([^)]+)\))?\|([^\n]*?)(?: \(\*\))?$/.exec(line);
    if (/^\d/.test(line)) assert(match, `Unparsed Cargo tree entry: ${line}`);
    if (!match) continue;
    const [, level, name, version, source, features] = match;
    const choices = metadata.packages.filter(pkg => pkg.name === name && pkg.version === version &&
      (source?.startsWith('/') ? path.dirname(pkg.manifest_path) === source : pkg.source !== null));
    assert.equal(choices.length, 1, `Ambiguous tree package: ${line}`);
    const { id } = choices[0];
    if (!nodes.has(id)) nodes.set(id, { features: [], deps: [] });
    const node = nodes.get(id);
    node.features = [...new Set([...node.features, ...features.split(',').filter(Boolean)])].sort();
    if (+level > 0) {
      assert(stack[+level - 1], `Missing tree parent: ${line}`);
      nodes.get(stack[+level - 1]).deps.push({ pkg: id, dep_kinds: [{ kind: null }] });
    }
    stack[+level] = id;
    stack.length = +level + 1;
  }
  assert(nodes.size > 0, 'Empty Cargo tree');
  const reached = new Set(nodes.keys());
  return { nodes, reached, packages: metadata.packages.filter(pkg => reached.has(pkg.id)) };
}

function graph(work, label) {
  const metadata = read(path.join(work, `${label}-metadata.json`));
  const { nodes, reached, packages } = treeClosure(metadata, fs.readFileSync(path.join(work, `${label}-tree.txt`), 'utf8'));
  const selected = packages.map(pkg => ({
    id: pkg.id, name: pkg.name, version: pkg.version, source: pkg.source,
    manifest: pkg.manifest_path, features: nodes.get(pkg.id).features,
    parents: packages.filter(parent => nodes.get(parent.id).deps.some(dep => dep.pkg === pkg.id &&
      dep.dep_kinds.some(kind => kind.kind === null || kind.kind === 'build'))).map(parent => parent.id),
  })).sort((a, b) => a.id.localeCompare(b.id));
  if (label === 'candidate') {
    const cryptos = selected.filter(pkg => pkg.name === 'crypto-bigint');
    assert(cryptos.length > 0, 'Missing Crypto in Omni closure');
    for (const pkg of cryptos) {
      assert.equal(pkg.version, '0.5.5');
      assert.equal(pkg.source, null, 'Registry Crypto remains reachable');
      assert.equal(pkg.manifest, path.join(work, 'vendor/crypto-bigint/Cargo.toml'));
    }
    for (const name of patches.slice(1)) {
      const version = name === 'hickory-proto' ? '0.24.4' : '0.25.4';
      for (const pkg of selected.filter(pkg => pkg.name === name && pkg.version === version))
        assert.equal(pkg.manifest, path.join(work, `vendor/${name}/Cargo.toml`));
    }
    assert.equal(digest(path.join(work, 'sdk', macro)), corrected);
    const macros = selected.filter(pkg => pkg.name === 'sp-api-proc-macro');
    assert.equal(macros.length, 1, 'Ambiguous client macro in node closure');
    assert.equal(macros[0].version, '15.0.0');
    assert.equal(macros[0].source, null);
    assert.equal(macros[0].manifest, path.join(work, 'sdk/substrate/primitives/api/proc-macro/Cargo.toml'));
    const ct = selected.find(pkg => pkg.name === 'ctutils');
    assert.equal(ct?.version, '0.4.8');
    assert.equal(ct.source, 'registry+https://github.com/rust-lang/crates.io-index');
    assert(!ct.features.includes('default'), 'ctutils default features unexpectedly enabled');
    const cmov = selected.filter(pkg => pkg.name === 'cmov');
    assert.equal(cmov.length, 1);
    assert.equal(cmov[0].version, '0.5.4');
    assert.equal(cmov[0].source, 'registry+https://github.com/rust-lang/crates.io-index');
    for (const name of patches.slice(1)) {
      assert(selected.some(pkg => pkg.name === name && pkg.source === null), `Reviewed ${name} patch unused`);
    }
  }
  save(path.join(work, `${label}-closure.json`), {
    scope: 'Actual host-target Omni Cargo tree normal/build closure and features, joined to full workspace metadata; not executed calls or whole SDK ownership',
    libp2pIdentity: selected.filter(pkg => pkg.name === 'libp2p-identity').map(pkg => ({ id: pkg.id, features: pkg.features, secp256k1Selected: pkg.features.includes('secp256k1') })),
    p256Callers: selected.filter(pkg => pkg.name === 'p256'),
    count: reached.size, packages: selected,
  });
}

function freeze(work, script, common) {
  const candidate = read(path.join(work, 'candidate-closure.json')).packages;
  const baseline = read(path.join(work, 'baseline-closure.json')).packages;
  const securityNames = ['wasmtime', 'cranelift-codegen', 'rustls', 'rustls-webpki', 'tokio', 'rustix', 'zstd', 'zstd-safe', 'zstd-sys', 'hickory-proto'];
  save(path.join(work, 'prepared.json'), {
    schema: 1, sdkCommit: '9448fe4f252936d241b8d2663506813b884f94f3',
    sdkTree: '97a1f73479286fdbd94e4bed0c5062999fab3073', toolchain: '1.94.1',
    profile: 'production: opt3, unwind, fat LTO, codegen-units 1', features: ['runtime-benchmarks'],
    qualification: 'Diagnostic production-like source build, not published binary equivalence or accepted node',
    expectedBinary: path.join(work, 'target/production/polkadot-omni-node'),
    build: ['cargo', '+1.94.1', 'build', '--verbose', '--locked', '--profile', 'production', '-p', 'polkadot-omni-node', '--bin', 'polkadot-omni-node', '--features', 'runtime-benchmarks', '-j', '2'],
    securityReviewRequired: candidate.filter(pkg => securityNames.includes(pkg.name)),
    libp2pIdentity: read(path.join(work, 'candidate-closure.json')).libp2pIdentity,
    p256Callers: read(path.join(work, 'candidate-closure.json')).p256Callers,
    originalLockSha256: digest(path.join(work, 'baseline.lock')),
    candidateLockSha256: digest(path.join(work, 'sdk/Cargo.lock')),
    changes: { removed: baseline.filter(a => !candidate.some(b => a.id === b.id)), added: candidate.filter(a => !baseline.some(b => a.id === b.id)) },
    scripts: [script, common, process.argv[1]].map(file => [fs.realpathSync(file), digest(file)]),
    inputs: ['sdk', 'vendor'].map(dir => [dir, inventory(path.join(work, dir))]),
    proofs: ['git-refs.txt', 'lock-delta.patch', 'baseline.lock', 'baseline-metadata.json', 'candidate-metadata.json', 'baseline-closure.json', 'candidate-closure.json', 'candidate-tree.txt', 'baseline-tree.txt'].map(file => [file, digest(path.join(work, file))]),
  });
}

function verify(work) {
  const manifest = read(path.join(work, 'prepared.json'));
  assert.equal(manifest.schema, 1);
  for (const [file, expected] of manifest.scripts) assert.equal(digest(file), expected, `Changed script: ${file}`);
  for (const [dir, expected] of manifest.inputs) assert.deepEqual(inventory(path.join(work, dir)), expected, `Changed prepared ${dir}`);
  for (const [file, expected] of manifest.proofs) assert.equal(digest(path.join(work, file)), expected, `Changed proof: ${file}`);
}

function environment(work) {
  const names = ['PATH', 'PROTOC', 'PERL5LIB', 'LD_LIBRARY_PATH', 'LIBCLANG_PATH',
    'CC', 'CXX', 'CFLAGS', 'CXXFLAGS', 'LDFLAGS', 'CARGO_TARGET_DIR',
    'WASM_BUILD_WORKSPACE_HINT', 'WASM_BUILD_TOOLCHAIN', 'CARGO_BUILD_JOBS'];
  const cargoHome = process.env.CARGO_HOME || path.join(os.homedir(), '.cargo');
  save(path.join(work, 'build-environment.json'), {
    target: 'Linux x86_64 host; exact Rust host and versions in build-tools.txt',
    environment: Object.fromEntries(names.map(name => [name, process.env[name] ?? null])),
    globalCargoConfigs: ['config', 'config.toml'].map(name => path.join(cargoHome, name))
      .filter(file => fs.existsSync(file)).map(file => [file, digest(file)]),
    toolsReceiptSha256: digest(path.join(work, 'build-tools.txt')),
    skipWasmBuildPresent: Object.hasOwn(process.env, 'SKIP_WASM_BUILD'),
  });
}

function selfTest() {
  const meta = { packages: [{ id: 'root', name: 'polkadot-omni-node' }, { id: 'a' }, { id: 'dev' }], resolve: { nodes: [
    { id: 'root', deps: [{ pkg: 'a', dep_kinds: [{ kind: 'build' }] }, { pkg: 'dev', dep_kinds: [{ kind: 'dev' }] }] },
    { id: 'a', deps: [] }, { id: 'dev', deps: [] },
  ] } };
  assert.deepEqual([...closure(meta).reached], ['root', 'a']);
  assert.notEqual(hash('before'), hash('after'));
  const fixture = fs.mkdtempSync(path.join(os.tmpdir(), 'deos-corrected-node-test-'));
  try {
    const sdk = path.join(fixture, 'sdk');
    const vendor = path.join(fixture, 'vendor');
    fs.mkdirSync(sdk); fs.mkdirSync(vendor);
    fs.writeFileSync(path.join(sdk, 'Cargo.toml'), '[workspace]\n');
    fs.writeFileSync(path.join(sdk, 'Cargo.lock'), 'locked');
    const script = path.join(fixture, 'builder.sh');
    fs.writeFileSync(script, 'original');
    fs.mkdirSync(path.join(sdk, path.dirname(macro)), { recursive: true });
    fs.writeFileSync(path.join(sdk, macro), 'wrong source');
    assert.throws(() => patch(sdk, vendor), /Unexpected SDK macro/);
    for (const file of ['git-refs.txt', 'lock-delta.patch', 'baseline-metadata.json', 'candidate-metadata.json', 'baseline-tree.txt', 'candidate-tree.txt', 'baseline.lock']) fs.writeFileSync(path.join(fixture, file), '{}');
    for (const label of ['baseline', 'candidate']) save(path.join(fixture, `${label}-closure.json`), { packages: [] });
    freeze(fixture, script, script);
    verify(fixture);
    fs.appendFileSync(path.join(sdk, 'Cargo.toml'), '# changed');
    assert.throws(() => verify(fixture), /Changed prepared sdk/);
    fs.writeFileSync(path.join(sdk, 'Cargo.toml'), '[workspace]\n');
    verify(fixture);
    fs.appendFileSync(script, 'changed');
    assert.throws(() => verify(fixture), /Changed script/);
    fs.writeFileSync(script, 'original');
    fs.appendFileSync(path.join(fixture, 'candidate-metadata.json'), 'changed');
    assert.throws(() => verify(fixture), /Changed proof/);
    const treeMeta = { packages: [{ id: 'root', name: 'polkadot-omni-node', version: '1', manifest_path: '/sdk/Cargo.toml' },
      { id: 'a', name: 'a', version: '1', source: 'registry+fixture', manifest_path: '/registry/a/Cargo.toml' },
      { id: 'unused', name: 'unused', version: '1', manifest_path: '/sdk/unused/Cargo.toml' }] };
    const actual = treeClosure(treeMeta, '0polkadot-omni-node v1 (/sdk)|default\n1a v1 (proc-macro) (/registry/a)|x\n1a v1|y (*)\n');
    assert.deepEqual([...actual.reached], ['root', 'a']);
    assert.deepEqual(actual.nodes.get('a').features, ['x', 'y']);
    assert.equal(actual.nodes.get('root').deps[0].pkg, 'a');
  } finally { fs.rmSync(fixture, { recursive: true, force: true }); }
  console.log('Source precondition, tree/features, freeze and script/source/graph invalidation self-tests passed');
}

const [mode, ...args] = process.argv.slice(2);
if (mode === 'patch') patch(...args);
else if (mode === 'graph') graph(...args);
else if (mode === 'freeze') freeze(...args);
else if (mode === 'verify') verify(...args);
else if (mode === 'environment') environment(...args);
else if (mode === 'self-test') selfTest();
else throw Error('Internal support mode required');
