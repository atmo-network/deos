import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import os from 'node:os';
import { spawnSync } from 'node:child_process';

const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const digest = file => hash(fs.readFileSync(file));
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const save = (file, value) => fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
const macro = 'substrate/primitives/api/proc-macro/src/impl_runtime_apis.rs';
const original = '5466016552eef8393ba2b15a51c5f23fadd6c5c5f874f2e5439f05bebd9758a8';
const corrected = 'c2d2f02bd4372b402fb15c2bc43046131d2e9a8f6b84ac91789e6507e557be3d';
const patches = ['crypto-bigint', 'hickory-proto', 'webpki-roots'];
const registry = 'registry+https://github.com/rust-lang/crates.io-index';
const provider = 'str0m-openssl';
const providerSource = 'f583a3c550be2b12979dff2fc2459b52f861ba0bb2c31b8007721dee7cba7e5d';
const providerArchive = 'ae33ce33dcd4dd2d9b6c01e7e425b604be9b811f69511b4387e76d5a36fe606b';
// Exact SDK parent blocks bound both the original selection and the retained edge.
// These multi-range requirements admit both choices; a package-wide Cargo update
// cannot select a dependency version for just one parent.
const parentSelections = [
  ['errno', '0.3.10', 'windows-sys', '0.52.0', '0.59.0', '>=0.52, <=0.59', '56b1f6f858154916dfb5c69bac8f628aac7de0522ba0102e509b84a3a0170481', '9772464afdbbc19798d5184d8552db3bced0f22a12909dd2f9253e0c8aa7e640'],
  ['quinn-udp', '0.6.1', 'windows-sys', '0.52.0', '0.60.2', '>=0.52, <=0.61', '0086ad9916a21cd8bdb06ccae168be5724878e97785d2cef59a06d7f264e6c3d', 'ea049c3379391db3cba4a11b33829d76ac14b04d36492ad5c6cadde19048f0d4'],
  ['rustix', '0.38.42', 'windows-sys', '0.52.0', '0.59.0', '>=0.52, <=0.59', '4fd94bf407cce113db412b9e0eee8e6e33d400c39f7d5370cc23c7324e61b5bd', '55dc6f221e30640706981f5aaf91c254b1c9d230825e8a2de54b963de55b1b7a'],
  ['rustix', '1.0.8', 'windows-sys', '0.52.0', '0.60.2', '>=0.52, <0.61', 'f1afe8469831ac2e1cca59a8cf40c68873868c7441ae126bf45989bc1e374714', 'b74c9ee20fd040a2ac1aa11c58941ccc4f8210202da828cc524a312195448dd2'],
  ['rustls-platform-verifier', '0.5.3', 'windows-sys', '0.52.0', '0.59.0', '>=0.52.0, <0.60.0', 'cde5be71a878b885ac25917c4c010e11a07e75a307599707cb98bccc8b097ecc', '845b1455c4c2fb5022fe3ecd2af84cceeebfc47c0b57c40515ed053c379f2616'],
  ['rustls-platform-verifier', '0.6.2', 'windows-sys', '0.52.0', '0.60.2', '>=0.52.0, <0.62.0', '14fc2dee142520406e9bbfe3209e7a357c6fea211002bde6ed21cec60e761a66', '45c19c702a306af711931d9faa03b1eda7af777b94f26fd43be1b37c4b0ffab1'],
  ['tempfile', '3.14.0', 'windows-sys', '0.52.0', '0.59.0', '>=0.52, <=0.59', 'ab327a3bd811085edb00240774fbf3a31bba683859700848ee26b6a946a1792e', '1dfa443ee037b59ec5366c12dd5c37d3cb77281fff54ba8834a392c7ccc8d0c2'],
  ['prost-build', '0.13.2', 'heck', '0.4.1', '0.5.0', '>=0.4, <=0.5', '0e41d16cc6b22e63af0675cc265942423a436f1a338c85e86afdb082fdcfbb08', 'fac096ef2f539b034e7eed299b67d0d6ab71fff36bb01ca10dd117bb312f30de'],
  ['prost-build', '0.14.1', 'heck', '0.4.1', '0.5.0', '>=0.4, <=0.5', '71988fc9a129a04f2741f26bc60445493f9cd0f589ad9d11daa9750fec1aa3b1', '7bcf6221bb72c40165a85791d9e820c073b47726f4c4aa80d25f7741be9478db'],
];

function retainedParentLock(text, metadata, selections = parentSelections) {
  const blocks = text.split('[[package]]');
  const field = (block, name) => new RegExp(`^${name} = "([^"]+)"`, 'm').exec(block)?.[1];
  const find = (name, version) => blocks.map((block, index) => ({ block, index })).filter(({ block }) =>
    field(block, 'name') === name && field(block, 'version') === version);
  for (const [name, version, dependency, from, to, requirement, beforeHash, afterHash] of selections) {
    const parents = find(name, version);
    assert.equal(parents.length, 1, `Missing or ambiguous retained parent: ${name}@${version}`);
    const { block, index } = parents[0];
    assert.equal(field(block, 'source'), registry, `Changed parent source: ${name}`);
    assert.equal(hash(block), beforeHash, `Changed parent block: ${name}@${version}`);
    const packages = metadata.packages.filter(pkg => pkg.name === name && pkg.version === version && pkg.source === registry);
    assert.equal(packages.length, 1, `Missing retained parent manifest: ${name}@${version}`);
    const deps = packages[0].dependencies.filter(dep => dep.name === dependency);
    assert.equal(deps.length, 1, `Changed parent requirement: ${name}`);
    assert.equal(deps[0].source, registry);
    assert.equal(deps[0].req, requirement, `Changed parent requirement: ${name}`);
    assert.equal(deps[0].kind, null);
    assert.equal(deps[0].target, dependency === 'windows-sys' ? 'cfg(windows)' : null);
    for (const selected of [from, to]) {
      const targets = find(dependency, selected);
      assert.equal(targets.length, 1, `Missing or ambiguous dependency: ${dependency}@${selected}`);
      assert.equal(field(targets[0].block, 'source'), registry, `Changed dependency source: ${dependency}`);
    }
    const edge = `\n "${dependency} ${from}",\n`;
    assert.equal(block.split(edge).length, 2, `Missing or duplicate retained edge: ${name}`);
    const retained = block.replace(edge, `\n "${dependency} ${to}",\n`);
    assert.equal(hash(retained), afterHash, `Changed retained block: ${name}@${version}`);
    blocks[index] = retained;
  }
  return blocks.join('[[package]]');
}

function retainParents(work) {
  const file = path.join(work, 'sdk/Cargo.lock');
  const before = fs.readFileSync(file, 'utf8');
  const after = retainedParentLock(before, read(path.join(work, 'baseline-metadata.json')));
  // Complete lock identity binds every compiler checksum and necessary dependency
  // edge as well as the unchanged remainder of this exact SDK construction.
  assert.equal(hash(after), '14c26cf10b721dfe0c4f78add53e3815b38daccdbb76fe425e647336301f8e03', 'Unexpected coupled compiler lock delta');
  fs.writeFileSync(file + '.retained', after, { flag: 'wx' });
  assert.equal(fs.readFileSync(file, 'utf8'), before, 'Lock changed during retention');
  fs.renameSync(file + '.retained', file);
  save(path.join(work, 'retained-parents.json'), parentSelections.map(([name, version, dependency, from, to, requirement]) =>
    ({ name, version, dependency, from, to, requirement })));
}

function assertRetainedGraph(work, metadata, selected) {
  // Project only the separately checked provider source substitution back onto
  // the old graph authority; never bless unrelated graph changes with new counts.
  const baseline = read(path.join(work, 'baseline-metadata.json')).packages.filter(pkg => pkg.name === provider);
  assert.equal(baseline.length, 1, 'Ambiguous baseline provider');
  assert.equal(baseline[0].version, '0.6.0');
  assert.equal(baseline[0].source, registry);
  const current = metadata.packages.filter(pkg => pkg.name === provider);
  assert.equal(current.length, 1, 'Ambiguous selected provider');
  assert.equal(current[0].source, null, 'Unpatched registry provider');
  assert.equal(current[0].manifest_path, path.join(work, `vendor/${provider}/Cargo.toml`));
  const projected = selected.map(pkg => pkg.name === provider ? { ...pkg,
    source: registry, manifest: baseline[0].manifest_path } : pkg);
  const normalize = value => JSON.parse(JSON.stringify(value).replaceAll(current[0].id, baseline[0].id).replaceAll(work, '$WORK')
    .replaceAll('36.0.7', '36.0.20').replaceAll('0.123.7', '0.123.20'));
  const packages = normalize(projected).map(pkg => ({ ...pkg, parents: [...pkg.parents].sort() }))
    .sort((a, b) => a.id.localeCompare(b.id));
  const nodes = normalize(metadata.resolve.nodes).map(node => ({ ...node,
    deps: [...node.deps].sort((a, b) => a.name.localeCompare(b.name)),
    dependencies: [...node.dependencies].sort(), features: [...node.features].sort(),
  })).sort((a, b) => a.id.localeCompare(b.id));
  // Fingerprints of the accepted graph, normalizing only the coupled patch IDs.
  // The full host closure includes all 49 crypto callers; nodes include Windows.
  assert.equal(hash(JSON.stringify(packages)), '8a899936f6b8fc5d814483a01a4b3ec498b009ba00ac701a383ac4ab162b8c13', 'Unrelated host graph delta');
  assert.equal(hash(JSON.stringify(nodes)), 'eef045cd637936faa23490361b7d86aa7ec07b1ab37762d417d9ca27b6a7177f', 'Unrelated workspace graph delta');
}

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

function patchProvider(sdk, vendor) {
  assertProviderSource(vendor);
  const manifest = path.join(sdk, 'Cargo.toml');
  const text = fs.readFileSync(manifest, 'utf8');
  assert.equal((text.match(/^\[patch\.crates-io\]$/gm) || []).length, 1, 'Missing or duplicate SDK patch table');
  assert(!/^str0m-openssl\s*=/m.test(text), 'Duplicate provider patch');
  fs.writeFileSync(manifest, text.replace('[patch.crates-io]\n',
    `[patch.crates-io]\n${provider} = { path = "../vendor/${provider}" }\n`));
}

function assertProviderSource(vendor) {
  assert.equal(digest(path.join(vendor, provider, 'Cargo.toml')), '873875f893878c43727c1e91fce4c0d6879336b15310e3256b50e83bc37d83fd', 'Changed provider manifest');
  assert.equal(digest(path.join(vendor, provider, 'src/dtls_ossl.rs')), providerSource, 'Changed reviewed provider source');
  assert.equal(digest(path.join(vendor, provider, 'src/lib.rs')), '79a84a365b2bf2cbaa4b5c2650feabb12d5a33efc210b65288d96eb25cf2eff9', 'Changed provider backend dispatch');
}

function assertProviderGraph(work, selected) {
  const one = name => {
    const pkgs = selected.filter(pkg => pkg.name === name);
    assert.equal(pkgs.length, 1, `Missing or duplicate selected ${name}`);
    return pkgs[0];
  };
  const ssl = one(provider);
  assert.equal(ssl.version, '0.6.0');
  assert.equal(ssl.source, null, 'Unpatched registry provider');
  assert.equal(ssl.manifest, path.join(work, `vendor/${provider}/Cargo.toml`));
  assert.deepEqual(ssl.features, ['vendored'], 'Provider must select Ossl, not dimpl');
  const str0m = one('str0m');
  assert.equal(str0m.version, '0.23.1');
  assert.deepEqual(str0m.features, ['openssl', 'vendored']);
  assert.deepEqual(ssl.parents, [str0m.id]);
  const lite = selected.filter(pkg => pkg.name === 'litep2p' && pkg.version === '0.15.3');
  assert.equal(lite.length, 1);
  assert(lite[0].features.includes('webrtc'), 'WebRTC not selected');
  assert.deepEqual(str0m.parents, [lite[0].id]);
  assert(selected.some(pkg => pkg.name === 'sc-network' && pkg.version === '0.60.0' && lite[0].parents.includes(pkg.id)), 'Missing SDK network provider path');
}

function providerLock(work) {
  const file = path.join(work, 'sdk/Cargo.lock');
  const text = fs.readFileSync(file, 'utf8');
  const header = `[[package]]\nname = "${provider}"\nversion = "0.6.0"\n`;
  assert.equal(text.split(header).length, 2, 'Missing or duplicate provider lock');
  const restored = text.replace(header, header + `source = "${registry}"\nchecksum = "${providerArchive}"\n`);
  assert.equal(hash(restored), '14c26cf10b721dfe0c4f78add53e3815b38daccdbb76fe425e647336301f8e03', 'Unrelated provider lock delta');
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

function assertCompilerProfile(selected) {
  for (const [family, version] of [[/^wasmtime(?:-|$)/, '36.0.20'], [/^cranelift-/, '0.123.20']]) {
    const members = selected.filter(pkg => family.test(pkg.name));
    assert(members.length > 0, `Missing compiler family: ${family}`);
    for (const pkg of members) assert.equal(pkg.version, version, `Uncorrected compiler: ${pkg.name}`);
  }
  const root = selected.filter(pkg => pkg.name === 'polkadot-omni-node');
  assert.equal(root.length, 1, 'Ambiguous Omni root');
  assert(root[0].features.includes('runtime-benchmarks'), 'Missing Omni runtime-benchmarks');
}

function graphSelection(metadata, text) {
  const { nodes, reached, packages } = treeClosure(metadata, text);
  const selected = packages.map(pkg => ({
    id: pkg.id, name: pkg.name, version: pkg.version, source: pkg.source,
    manifest: pkg.manifest_path, features: nodes.get(pkg.id).features,
    parents: packages.filter(parent => nodes.get(parent.id).deps.some(dep => dep.pkg === pkg.id &&
      dep.dep_kinds.some(kind => kind.kind === null || kind.kind === 'build'))).map(parent => parent.id),
  })).sort((a, b) => a.id.localeCompare(b.id));
  return { selected, reached };
}

function graph(work, label) {
  const metadata = read(path.join(work, `${label}-metadata.json`));
  const { selected, reached } = graphSelection(metadata, fs.readFileSync(path.join(work, `${label}-tree.txt`), 'utf8'));
  if (label === 'candidate') {
    assertCompilerProfile(selected);
    assertProviderSource(path.join(work, 'vendor'));
    assertProviderGraph(work, selected);
    providerLock(work);
    assertRetainedGraph(work, metadata, selected);
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

// This is an explicit exploratory composition, not a caller-extensible policy.
const compositionPatches = [
  ['hickory-proto-026', 'hickory-proto', '8d96022d321d8e20f7c3d846a424d0f6a0890295129b37a52044219ba3c80ecd'],
  ['lru-0.7.8', 'lru', '581dc40b8231a11b4082a1a5503e6b6a904dc713049a63e69f27d5080dbfc873'],
  ['lru-0.12.3', 'lru', 'ba866e50566f1dd7561507e5f531ba355cd6140f09050477cff209fa2cee4d3e'],
  ['memmap2-0.5.10', 'memmap2', 'cbf8d835df0ae240cd6ce717346d698eca65f15319d84053c5a40372415d2c9d'],
];

const compositionSources = [
  ['hickory-proto-026', 'adc588665b22255b084b031abc3ea2682a358855d6222dec544aca5081250ba9'],
  ['lru-0.7.8', 'e8ac2462a97ceb9f27f7332a08ac44ddec47d658187c968453e30099dbd17429'],
  ['lru-0.12.3', 'a55deff7910bc57f656e2ae57d92dd44c17bafd5b95e69f804645ef121a763d8'],
  ['memmap2-0.5.10', 'c31931c8dfc90c2b685763834d8150a472293fe4846ce94c66ded547f71bba62'],
  ['hickory-proto', '0eca822c518c4f58e2be88169123b20f05742cca67e5f74bd47bb451b71b9f76'],
];

function assertCompositionSources(work) {
  for (const [dir, expected] of compositionSources)
    assert.equal(hash(JSON.stringify(inventory(path.join(work, 'vendor', dir)))), expected, `Changed composed source: ${dir}`);
}

function composePatches(work) {
  providerLock(work);
  assertCompositionSources(work);
  const file = path.join(work, 'sdk/Cargo.toml');
  const before = fs.readFileSync(file, 'utf8');
  assert.equal((before.match(/^\[patch\.crates-io\]$/gm) || []).length, 1);
  for (const [dir, , expected] of compositionPatches)
    assert.equal(digest(path.join(work, 'vendor', dir, 'Cargo.toml')), expected, `Changed imported manifest: ${dir}`);
  fs.writeFileSync(file, before + compositionPatches.map(([dir, name]) =>
    `${dir.replaceAll('.', '-')} = { package = "${name}", path = "../vendor/${dir}" }\n`).join(''));
}

function composeMinimums(work) {
  const file = path.join(work, 'sdk/Cargo.toml');
  let text = fs.readFileSync(file, 'utf8');
  for (const [name, from, to] of [
    ['anyhow', '1.0.81', '1.0.103'], ['memmap2', '0.9.3', '0.9.11'],
    ['rustix', '1.0.3', '1.1.6'], ['tokio', '1.45.0', '1.53.3'],
    ['tracing-subscriber', '=0.3.19', '=0.3.20'],
  ]) {
    const old = `${name} = { version = "${from}"`;
    assert.equal(text.split(old).length, 2, `Changed manifest minimum: ${name}`);
    text = text.replace(old, `${name} = { version = "${to}"`);
  }
  fs.writeFileSync(file, text);
}

function composeRetain(work) {
  const file = path.join(work, 'sdk/Cargo.lock');
  const before = fs.readFileSync(file, 'utf8');
  const blocks = before.split('[[package]]');
  const field = (block, key) => new RegExp(`^${key} = "([^"]+)"`, 'm').exec(block)?.[1];
  // Retain all eight still-existing parent choices; replaced rustix1 is obsolete.
  // Retain every unrelated multi-range edge found by the complete workspace
  // comparison, not only the nine historical named choices.
  const choices = parentSelections.filter(([name, version]) => name !== 'rustix' || version !== '1.0.8')
    .map(([name, version, dep, , to]) => [name, version, dep, to]);
  choices.push(
    ['prost-derive', '0.12.6', 'itertools', '0.11.0'],
    ['alloy-rpc-types-eth', '1.8.3', 'itertools', '0.13.0'],
    ['alloy-transport-http', '1.8.3', 'itertools', '0.13.0'],
    ['prost-build', '0.14.1', 'itertools', '0.13.0'],
    ['prost-derive', '0.13.5', 'itertools', '0.13.0'],
    ['prost-derive', '0.14.1', 'itertools', '0.13.0'],
    ['num_enum_derive', '0.7.6', 'proc-macro-crate', '1.3.1'],
    ['hyper-util', '0.1.17', 'socket2', '0.5.9'],
    ['quinn', '0.11.9', 'socket2', '0.5.9'],
    ['quinn-udp', '0.6.1', 'socket2', '0.5.9'],
  );
  for (const [name, version, dep, to] of choices) {
    const matches = blocks.map((block, index) => ({ block, index })).filter(({ block }) =>
      field(block, 'name') === name && field(block, 'version') === version);
    assert.equal(matches.length, 1, `Missing retained composition parent: ${name}@${version}`);
    const { block, index } = matches[0];
    assert.equal(field(block, 'source'), registry);
    assert(blocks.some(b => field(b, 'name') === dep && field(b, 'version') === to), `Missing retained target: ${dep}@${to}`);
    const edge = new RegExp(`\n "${dep} [^"]+",\n`, 'g');
    assert.equal([...block.matchAll(edge)].length, 1, `Missing or ambiguous retained edge: ${name}`);
    blocks[index] = block.replace(edge, `\n "${dep} ${to}",\n`);
  }
  fs.writeFileSync(file, blocks.join('[[package]]'));
  save(path.join(work, 'composed-retained-parents.json'), { choices, obsolete: 'rustix@1.0.8 replaced by reviewed1.1.6' });
}

// Measured exploratory identity only. Independent acceptance and the parent's
// final Native prerequisite guard are still required; these pins confer neither.
const composedIdentity = {
  manifest: 'e4fca58f4fb0e1bc1edc87bfdc4c55e43087ff377940403d7ad18c7568d3f81a',
  lock: '4930b498b00c8b6820da65a1ed08c5ce3c1c200dc5a4a1b019a51b1dd0efb8b0',
  sdk: '96e724553e457385400ca68ff1c2f12a6bda9970d9c072aaadd2bcae21f384a7',
  vendor: '048b9ec875e3eb84213bddb7d33ab8cb1d69e9106433f11bf3ffd6cefa49fa54',
  normal: 'f24c7440b1e01033f81435f3fd871ddeaff566a8f5496c7f580b7d3f743c7d65',
  workspace: '893cb743c3bf5a75fac73095f205b99b9104020990ad2d75bf5dcfffff885bc1',
};

function assertComposedIdentity(actual) {
  assert.deepEqual(actual, composedIdentity, 'Unexpected exploratory composition delta (not source acceptance)');
}

function composeReport(work) {
  const before = read(path.join(work, 'candidate-metadata.json'));
  const after = read(path.join(work, 'composed-metadata.json'));
  const normalize = value => JSON.parse(JSON.stringify(value).replaceAll(work, '$WORK'));
  const key = pkg => `${pkg.name}@${pkg.version}:${pkg.source || '$LOCAL'}`;
  const oldPackages = new Map(before.packages.map(pkg => [key(pkg), pkg]));
  const newPackages = new Map(after.packages.map(pkg => [key(pkg), pkg]));
  const oldNodes = new Map(before.resolve.nodes.map(node => [node.id, node]));
  const newNodes = new Map(after.resolve.nodes.map(node => [node.id, node]));
  const project = (pkg, nodes, metadata) => {
    const names = new Map(metadata.packages.map(p => [p.id, key(p)]));
    const node = nodes.get(pkg.id);
    return { features: [...node.features].sort(), deps: node.deps.map(dep => ({
      name: dep.name, pkg: names.get(dep.pkg), kinds: dep.dep_kinds,
    })).sort((a, b) => a.name.localeCompare(b.name) || a.pkg.localeCompare(b.pkg)) };
  };
  const changed = [];
  for (const [id, pkg] of oldPackages) if (newPackages.has(id)) {
    const left = project(pkg, oldNodes, before);
    const right = project(newPackages.get(id), newNodes, after);
    if (JSON.stringify(left) !== JSON.stringify(right)) changed.push({ package: id, before: left, after: right });
  }
  const selected = read(path.join(work, 'composed-closure.json')).packages;
  assertCompilerProfile(selected);
  assertProviderSource(path.join(work, 'vendor'));
  assertProviderGraph(work, selected);
  assertCompositionSources(work);
  const required = [
    ['hickory-net', '0.26.2'], ['hickory-resolver', '0.26.2'], ['rustls', '0.23.46'],
    ['rustls-webpki', '0.103.16'], ['h2', '0.4.16'], ['rustls-native-certs', '0.8.1'],
    ['tokio', '1.53.3'], ['rustix', '1.1.6'], ['crossbeam-epoch', '0.9.20'],
    ['anyhow', '1.0.103'], ['cxx', '1.0.196'], ['cxx-build', '1.0.196'],
    ['memmap2', '0.9.11'], ['zstd', '0.13.4'], ['zstd-safe', '7.4.0'],
    ['zstd-sys', '2.1.2+zstd.1.5.7'], ['tracing-subscriber', '0.3.20'],
    ['ctutils', '0.4.8'], ['cmov', '0.5.4'],
  ];
  for (const [name, version] of required) assert(selected.some(p => p.name === name && p.version === version), `Missing selected remedy: ${name}@${version}`);
  for (const [dir, name] of compositionPatches) assert(selected.some(p => p.name === name && p.source === null &&
    p.manifest === path.join(work, 'vendor', dir, 'Cargo.toml')), `Unused local remedy: ${dir}`);
  assert.equal(digest(path.join(work, 'sdk', macro)), corrected);
  assert(!selected.some(p => p.name === 'hickory-proto' && p.version === '0.24.4' && p.features.includes('text-parsing')));
  for (const [name, version, feature] of [['keccak', '0.1.4', 'asm'], ['rand', '0.8.5', 'log'], ['rand', '0.9.0', 'log']]) {
    const packages = selected.filter(p => p.name === name && p.version === version);
    assert.equal(packages.length, 1);
    assert(!packages[0].features.includes(feature), `Unreviewed feature: ${name}/${feature}`);
  }
  const identity = {
    manifest: digest(path.join(work, 'sdk/Cargo.toml')), lock: digest(path.join(work, 'sdk/Cargo.lock')),
    sdk: hash(JSON.stringify(inventory(path.join(work, 'sdk')))),
    vendor: hash(JSON.stringify(inventory(path.join(work, 'vendor')))),
    normal: hash(JSON.stringify(normalize(selected))), workspace: hash(JSON.stringify(normalize(after.resolve))),
  };
  assertComposedIdentity(identity);
  const files = ['sdk/Cargo.toml', 'sdk/Cargo.lock', 'composed-metadata.json', 'composed-tree.txt', 'composed-closure.json'];
  save(path.join(work, 'composition-report.json'), normalize({
    qualification: 'Exploratory full SDK composition; independent complete source/graph review required before freeze or build',
    identity, files: files.map(file => [file, digest(path.join(work, file))]),
    sourceInventories: ['sdk', 'vendor'].map(dir => [dir, hash(JSON.stringify(inventory(path.join(work, dir))))]),
    required, added: [...newPackages].filter(([id]) => !oldPackages.has(id)).map(([, pkg]) => pkg),
    removed: [...oldPackages].filter(([id]) => !newPackages.has(id)).map(([, pkg]) => pkg),
    unchangedParentDeltas: changed,
    normalParentDeltas: (() => {
      const old = read(path.join(work, 'candidate-closure.json')).packages;
      const identity = p => `${p.name}@${p.version}:${p.source || '$LOCAL'}`;
      const delta = [];
      for (const pkg of old) {
        const current = selected.find(p => identity(p) === identity(pkg));
        if (!current) continue;
        if (JSON.stringify(pkg.features) !== JSON.stringify(current.features) ||
            JSON.stringify([...pkg.parents].sort()) !== JSON.stringify([...current.parents].sort()))
          delta.push({ package: identity(pkg), before: pkg, after: current });
      }
      return delta;
    })(),
    normalGraphSha256: hash(JSON.stringify(normalize(selected))),
    workspaceGraphSha256: hash(JSON.stringify(normalize(after.resolve))),
    vendor: inventory(path.join(work, 'vendor')),
  }));
}

// Source preflight deliberately has no build/admission authority. These fixed
// accepted composition pins cannot be replaced by a caller-provided digest.
const sourceConfigSha256 = 'b19bf3b1b8fe09e7af28738a31dac7584490c7e97457b6372363351fc87f49f5';
function sourceConfiguration(sdk, env = process.env, home = os.userInfo().homedir) {
  for (const [name, value] of Object.entries(env)) {
    assert(!(name.startsWith('CARGO_') && !/^(CARGO_BUILD_JOBS|CARGO_TARGET_DIR|CARGO_NET_OFFLINE|CARGO_TERM_[A-Z_]+|CARGO_HOME)$/.test(name)) &&
      !/^(RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|RUSTDOCFLAGS|CARGO_ENCODED_RUSTDOCFLAGS|RUSTC|RUSTDOC|RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|RUSTC_BOOTSTRAP)$/.test(name),
    `Unsupported source compiler/configuration override: ${name}`);
    if (name.startsWith('WASM_')) assert(
      (name === 'WASM_BUILD_TOOLCHAIN' && value === '1.94.1') ||
      (name === 'WASM_BUILD_WORKSPACE_HINT' && value === sdk), `Unsupported source Wasm override: ${name}`);
  }
  assert.equal(env.HOME ?? home, home, 'Unsupported source HOME');
  assert.equal(env.CARGO_HOME ?? path.join(home, '.cargo'), path.join(home, '.cargo'), 'Unsupported source CARGO_HOME');
  assert.equal(env.RUSTUP_HOME ?? path.join(home, '.rustup'), path.join(home, '.rustup'), 'Unsupported source RUSTUP_HOME');
  assert.equal(env.RUSTUP_TOOLCHAIN ?? '1.94.1', '1.94.1', 'Unsupported source RUSTUP_TOOLCHAIN');
  assert(!Object.hasOwn(env, 'SKIP_WASM_BUILD'), 'Unsupported source SKIP_WASM_BUILD');
  const reviewed = path.join(sdk, '.cargo/config.toml');
  const configs = new Set(['config', 'config.toml'].map(name => path.join(home, '.cargo', name)));
  for (let dir = sdk; ; dir = path.dirname(dir)) {
    for (const name of ['config', 'config.toml']) configs.add(path.join(dir, '.cargo', name));
    if (dir === path.dirname(dir)) break;
  }
  for (const file of configs) {
    if (!fs.existsSync(file)) { assert.notEqual(file, reviewed, 'Missing source Cargo config'); continue; }
    assert.equal(file, reviewed, `Unreviewed source Cargo config: ${file}`);
    assert(fs.lstatSync(file).isFile() && !fs.lstatSync(file).isSymbolicLink(), 'Source Cargo config substitution');
    assert.equal(fs.realpathSync(file), file, 'Source Cargo config path substitution');
    assert.equal(digest(file), sourceConfigSha256, 'Changed source Cargo config');
  }
  return { home, cargoHome: path.join(home, '.cargo'), rustupHome: path.join(home, '.rustup'),
    configs: [[reviewed, sourceConfigSha256]] };
}

function composedSourceIdentity(sourceWork, metadata, selected) {
  const normalize = value => JSON.parse(JSON.stringify(value).replaceAll(sourceWork, '$WORK'));
  return {
    manifest: digest(path.join(sourceWork, 'sdk/Cargo.toml')), lock: digest(path.join(sourceWork, 'sdk/Cargo.lock')),
    sdk: hash(JSON.stringify(inventory(path.join(sourceWork, 'sdk')))),
    vendor: hash(JSON.stringify(inventory(path.join(sourceWork, 'vendor')))),
    normal: hash(JSON.stringify(normalize(selected))), workspace: hash(JSON.stringify(normalize(metadata.resolve))),
  };
}

const sourceRole = { package: 'polkadot-omni-node', features: ['runtime-benchmarks'], target: 'x86_64-unknown-linux-gnu',
  edges: 'normal,build', toolchain: '1.94.1', profile: 'production: opt3, unwind, fat LTO, codegen-units 1' };
const sourceProofs = ['git-refs.txt', 'source-tools.txt', 'composed-metadata.json', 'composed-tree.txt', 'composed-closure.json', 'source-consumers.json'];
const currentProducers = () => ['build-corrected-node.sh', '_common.sh', 'build-corrected-node.mjs']
  .map(name => { const file = path.join(path.dirname(fs.realpathSync(process.argv[1])), name); return [file, digest(file)]; });

function sourcePreflight(sdk, work, script, common) {
  assert.equal(fs.realpathSync(sdk), sdk, 'Source SDK must be canonical');
  const sourceWork = path.dirname(sdk);
  assert.equal(path.basename(sdk), 'sdk', 'Source requires an actual sdk/vendor composition');
  assert.equal(fs.realpathSync(path.dirname(work)), path.dirname(work), 'Evidence parent must be canonical');
  assert(!fs.existsSync(work), 'Refusing existing source preflight');
  const configuration = sourceConfiguration(sdk);
  const producers = currentProducers();
  assert.deepEqual([script, common, process.argv[1]].map(file => [fs.realpathSync(file), digest(file)]), producers);
  assert.equal(process.platform, 'linux', 'Source role requires Linux');
  assert.equal(process.arch, 'x64', 'Source role requires x86_64');
  const toolBin = path.join(configuration.rustupHome, 'toolchains/1.94.1-x86_64-unknown-linux-gnu/bin');
  for (const tool of ['cargo', 'rustc']) assert.equal(fs.realpathSync(path.join(toolBin, tool)), path.join(toolBin, tool), 'Fixed source toolchain substitution');
  fs.mkdirSync(work);
  const run = (command, args) => {
    const result = spawnSync(command, args, { cwd: sdk, encoding: 'utf8', maxBuffer: 128 * 1024 * 1024,
      env: { ...process.env, RUSTC: path.join(toolBin, 'rustc'), CARGO_TARGET_DIR: path.join(work, 'target'),
        CARGO_TERM_COLOR: 'never', CARGO_NET_OFFLINE: 'true', GIT_ALLOW_PROTOCOL: '', GIT_TERMINAL_PROMPT: '0' } });
    assert.equal(result.status, 0, `Source preflight command failed: ${command} ${args.join(' ')}: ${result.error ?? result.stderr}`);
    return result.stdout;
  };
  const git = args => run('git', ['-c', 'protocol.allow=never', '--no-optional-locks', ...args]);
  assert.equal(git(['rev-parse', 'HEAD']).trim(), '9448fe4f252936d241b8d2663506813b884f94f3', 'Source SDK commit drift');
  assert.equal(git(['rev-parse', 'HEAD^{tree}']).trim(), '97a1f73479286fdbd94e4bed0c5062999fab3073', 'Source SDK release tree drift');
  const refs = git(['show-ref']);
  fs.writeFileSync(path.join(work, 'git-refs.txt'), refs, { flag: 'wx' });
  const rustc = run(path.join(toolBin, 'rustc'), ['-vV']);
  assert.match(rustc, /^release: 1\.94\.1$/m);
  assert.match(rustc, /^commit-hash: e408947bfd200af42db322daf0fadfe7e26d3bd1$/m);
  assert.match(rustc, /^host: x86_64-unknown-linux-gnu$/m);
  assert.match(rustc, /^LLVM version: 21\.1\.8$/m);
  const cargo = run(path.join(toolBin, 'cargo'), ['-V']);
  assert.equal(cargo.trim(), 'cargo 1.94.1 (29ea6fb6a 2026-03-24)');
  const cfg = run(path.join(toolBin, 'rustc'), ['--print', 'cfg', '--target', 'x86_64-unknown-linux-gnu']);
  fs.writeFileSync(path.join(work, 'source-tools.txt'), rustc + cargo + cfg, { flag: 'wx' });
  const sourceBefore = { sdk: inventory(sdk), vendor: inventory(path.join(sourceWork, 'vendor')) };
  assert.equal(hash(JSON.stringify(sourceBefore.sdk)), composedIdentity.sdk, 'Unaccepted source SDK');
  assert.equal(hash(JSON.stringify(sourceBefore.vendor)), composedIdentity.vendor, 'Unaccepted source vendor');
  const metadata = run(path.join(toolBin, 'cargo'), ['metadata', '--offline', '--locked', '--format-version', '1',
    '--features', 'polkadot-omni-node/runtime-benchmarks']);
  fs.writeFileSync(path.join(work, 'composed-metadata.json'), metadata, { flag: 'wx' });
  const tree = run(path.join(toolBin, 'cargo'), ['tree', '--offline', '--locked', '-p', 'polkadot-omni-node',
    '--features', 'runtime-benchmarks', '--target', 'x86_64-unknown-linux-gnu', '--edges', 'normal,build', '--prefix', 'depth', '--format', '{p}|{f}']);
  fs.writeFileSync(path.join(work, 'composed-tree.txt'), tree, { flag: 'wx' });
  graph(work, 'composed');
  const current = JSON.parse(metadata);
  assert.equal(current.workspace_root, sdk, 'Source metadata owner drift');
  assertComposedGraphEvidence(sourceWork, work);
  const selected = read(path.join(work, 'composed-closure.json')).packages;
  assertComposedIdentity(composedSourceIdentity(sourceWork, current, selected));
  assertCompilerProfile(selected);
  assertProviderSource(path.join(sourceWork, 'vendor'));
  assertProviderGraph(sourceWork, selected);
  assertCompositionSources(sourceWork);
  // Seal every selected external consumer, including manifests, build scripts,
  // macros and non-Rust generator inputs. This is not an operation review.
  const consumers = selected.map(pkg => {
    const root = path.dirname(pkg.manifest);
    assert.equal(fs.realpathSync(root), root, 'Source consumer path substitution');
    const owner = root.startsWith(sdk + path.sep) ? 'sdk' : root.startsWith(path.join(sourceWork, 'vendor') + path.sep) ? 'vendor' : null;
    return { id: pkg.id, root, owner, files: owner ? null : inventory(root) };
  });
  save(path.join(work, 'source-consumers.json'), { sourceRoot: sourceWork, sourceInventories: sourceBefore, consumers });
  assert.deepEqual(sourceConfiguration(sdk), configuration, 'Source configuration changed during preflight');
  assert.deepEqual(currentProducers(), producers, 'Source producers changed during preflight');
  assert.equal(run(path.join(toolBin, 'rustc'), ['-vV']), rustc, 'Source compiler changed during preflight');
  assert.equal(run(path.join(toolBin, 'cargo'), ['-V']), cargo, 'Source Cargo changed during preflight');
  assert.equal(run(path.join(toolBin, 'rustc'), ['--print', 'cfg', '--target', sourceRole.target]), cfg, 'Source cfg changed during preflight');
  assert.equal(git(['rev-parse', 'HEAD']).trim(), '9448fe4f252936d241b8d2663506813b884f94f3', 'Source HEAD changed during preflight');
  assert.equal(git(['rev-parse', 'HEAD^{tree}']).trim(), '97a1f73479286fdbd94e4bed0c5062999fab3073', 'Source tree changed during preflight');
  assert.deepEqual(inventory(sdk), sourceBefore.sdk, 'Source SDK changed during preflight');
  assert.deepEqual(inventory(path.join(sourceWork, 'vendor')), sourceBefore.vendor, 'Source vendor changed during preflight');
  for (const consumer of consumers) if (consumer.files) assert.deepEqual(inventory(consumer.root), consumer.files, 'Source consumer changed during preflight');
  assert.equal(git(['show-ref']), refs, 'Source refs changed during preflight');
  save(path.join(work, 'source-preflight.json'), {
    schema: 1, stage: 'accepted-composition-source-preflight', capturedAt: new Date().toISOString(),
    qualification: 'UNQUALIFIED source evidence only; not a reviewed Native operation proof, finding clearance, freeze or build admission',
    sourceRoot: sourceWork, workspaceRoot: sdk, evidenceRoot: work, identity: composedIdentity,
    role: sourceRole,
    configuration, sharedCaches: [configuration.cargoHome, configuration.rustupHome],
    producers,
    proofs: sourceProofs.map(file => [file, digest(path.join(work, file))]),
  });
  console.log(`Unqualified current-source preflight only; no freeze, finding clearance or build: ${work}`);
}

// Technical source immutability only. Independent operation/security approval
// remains a caller workflow gate; --build is a separate explicit admission.
const composedKind = 'composed-source';
const composedStage = 'technically-frozen-source-only';
const composedQualification = 'Technical source freeze only; no Native finding clearance, security acceptance, ELF qualification or implicit build admission';
function regularCanonical(file) {
  assert.equal(fs.realpathSync(file), file, `Noncanonical proof path: ${file}`);
  assert(fs.lstatSync(file).isFile() && !fs.lstatSync(file).isSymbolicLink(), `Nonregular proof: ${file}`);
}

function sourceEvidence(work, evidence) {
  assert.equal(fs.realpathSync(work), work, 'Noncanonical composed workspace');
  assert.equal(fs.realpathSync(evidence), evidence, 'Noncanonical evidence root');
  assert(evidence !== work && !evidence.startsWith(work + path.sep), 'Evidence must be outside source workspace');
  const file = path.join(evidence, 'source-preflight.json');
  regularCanonical(file);
  const proof = read(file);
  assert.deepEqual(Object.keys(proof).sort(), ['schema', 'stage', 'capturedAt', 'qualification', 'sourceRoot', 'workspaceRoot',
    'evidenceRoot', 'identity', 'role', 'configuration', 'sharedCaches', 'producers', 'proofs'].sort(), 'Unknown/missing source evidence fields');
  assert.equal(proof.schema, 1);
  assert.equal(proof.stage, 'accepted-composition-source-preflight');
  assert.equal(proof.qualification, 'UNQUALIFIED source evidence only; not a reviewed Native operation proof, finding clearance, freeze or build admission');
  assert.equal(new Date(proof.capturedAt).toISOString(), proof.capturedAt);
  assert.equal(proof.sourceRoot, work, 'Source evidence relocation forbidden');
  assert.equal(proof.workspaceRoot, path.join(work, 'sdk'));
  assert.equal(proof.evidenceRoot, evidence);
  assertComposedIdentity(proof.identity);
  assert.deepEqual(proof.role, sourceRole, 'Source role drift');
  assert.deepEqual(proof.configuration, sourceConfiguration(path.join(work, 'sdk')), 'Source configuration drift');
  assert.deepEqual(proof.sharedCaches, [proof.configuration.cargoHome, proof.configuration.rustupHome]);
  assert.deepEqual(proof.producers, currentProducers(), 'Source producer drift');
  assert.deepEqual(proof.proofs.map(([name]) => name), sourceProofs, 'Unknown/missing source proofs');
  for (const [name, expected] of proof.proofs) {
    regularCanonical(path.join(evidence, name));
    assert.equal(digest(path.join(evidence, name)), expected, `Source evidence drift: ${name}`);
  }
  return proof;
}

function composedManifest(work, evidence, proof) {
  const files = ['.corrected-node-owner', 'git-refs.txt', 'composed-metadata.json', 'composed-tree.txt', 'composed-closure.json'];
  return {
    schema: 1, kind: composedKind, stage: composedStage, scope: 'source-only', qualification: composedQualification,
    sourceRoot: work, evidenceRoot: evidence, identity: proof.identity, role: proof.role,
    configuration: proof.configuration, producers: proof.producers,
    sourceEvidenceSha256: digest(path.join(evidence, 'source-preflight.json')),
    consumerSealSha256: digest(path.join(evidence, 'source-consumers.json')),
    proofs: files.map(name => { regularCanonical(path.join(work, name)); return [name, digest(path.join(work, name))]; }),
    expectedBinary: path.join(work, 'target/production/polkadot-omni-node'),
  };
}

function compareSourceSnapshots(evidence, fresh) {
  const before = read(path.join(evidence, 'source-preflight.json'));
  const after = read(path.join(fresh, 'source-preflight.json'));
  for (const key of ['sourceRoot', 'workspaceRoot', 'identity', 'role', 'configuration', 'sharedCaches', 'producers'])
    assert.deepEqual(after[key], before[key], `Source snapshot drift: ${key}`);
  for (const name of sourceProofs) {
    if (name === 'composed-metadata.json') {
      // Only Cargo's evidence-local output directories differ. Package IDs,
      // manifests, resolve/features and workspace ownership remain exact.
      const semantic = root => {
        const metadata = read(path.join(root, name));
        assert.equal(metadata.target_directory, path.join(root, 'target'));
        if (metadata.build_directory !== undefined) assert.equal(metadata.build_directory, path.join(root, 'target'));
        delete metadata.target_directory;
        delete metadata.build_directory;
        return metadata;
      };
      assert.deepEqual(semantic(fresh), semantic(evidence), 'Fresh source metadata drift');
    } else assert.equal(digest(path.join(fresh, name)), digest(path.join(evidence, name)), `Fresh source proof drift: ${name}`);
  }
}

function assertComposedState(manifest, expected) {
  assert.deepEqual(manifest, expected, 'Unknown/missing/drifted composed frozen state');
}

function assertComposedGraphEvidence(work, evidence) {
  const metadata = read(path.join(work, 'composed-metadata.json'));
  assert.equal(metadata.workspace_root, path.join(work, 'sdk'), 'Composed metadata relocation');
  const captured = read(path.join(evidence, 'composed-metadata.json'));
  assert.deepEqual(metadata.resolve, captured.resolve, 'Composed resolve mismatch');
  assert.deepEqual(metadata.packages, captured.packages, 'Composed package mismatch');
  for (const name of ['git-refs.txt', 'composed-closure.json'])
    assert.equal(digest(path.join(work, name)), digest(path.join(evidence, name)), `Composed proof mismatch: ${name}`);
  // Cargo's explicit --target separates host/target deduplication in raw tree
  // rendering. Require the exact same package/features/parents projection, not
  // identical (*) placement; both raw trees are independently sealed.
  const { selected } = graphSelection(metadata, fs.readFileSync(path.join(work, 'composed-tree.txt'), 'utf8'));
  assert.deepEqual(selected, read(path.join(evidence, 'composed-closure.json')).packages, 'Composed tree projection mismatch');
}

function verifyComposed(work, manifest) {
  const evidence = manifest.evidenceRoot;
  assert.equal(typeof evidence, 'string', 'Missing composed evidence root');
  const proof = sourceEvidence(work, evidence);
  assertComposedState(manifest, composedManifest(work, evidence, proof));
  assert.equal(fs.readFileSync(path.join(work, '.corrected-node-owner'), 'utf8'), 'deos-corrected-node-v1\n');
  // The constructor's original graph is bound independently of the fresh
  // preflight. Never reinterpret candidate-era metadata as the composed role.
  assertComposedGraphEvidence(work, evidence);
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'deos-composed-verifier-'));
  try {
    const fresh = path.join(temporary, 'preflight');
    const producers = currentProducers();
    sourcePreflight(path.join(work, 'sdk'), fresh, producers[0][0], producers[1][0]);
    compareSourceSnapshots(evidence, fresh);
    // Re-read the originals after the complete live graph/consumer capture.
    sourceEvidence(work, evidence);
    assert.deepEqual(manifest, composedManifest(work, evidence, proof), 'Frozen inputs changed during verification');
  } finally { fs.rmSync(temporary, { recursive: true, force: true }); }
}

function publishSourceFreeze(work, prepareManifest) {
  assert.equal(fs.realpathSync(work), work, 'Noncanonical composed workspace');
  for (const name of ['prepared.json', 'attempts', 'build-admission'])
    assert(!fs.lstatSync(path.join(work, name), { throwIfNoEntry: false }), 'Refusing prepared or attempted workspace');
  const admission = path.join(work, 'source-freeze-admission');
  fs.mkdirSync(admission);
  try {
    const manifest = prepareManifest();
    // Exclusive publish only after all before/after checks; no legacy retrofit.
    save(path.join(work, 'prepared.json'), manifest);
    console.log(`Technical composed source freeze only; separate review and explicit build admission required: ${work}`);
  } finally { fs.rmdirSync(admission); }
}

function freezeComposed(work, evidence) {
  publishSourceFreeze(work, () => {
    const proof = sourceEvidence(work, evidence);
    const manifest = composedManifest(work, evidence, proof);
    verifyComposed(work, manifest);
    return manifest;
  });
}

function freeze(work, script, common) {
  assert(!fs.existsSync(path.join(work, 'composed-closure.json')) && !fs.existsSync(path.join(work, 'source-preflight.json')),
    'Composed source requires a distinct approved Native source-stage freeze; candidate-era freeze forbidden');
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
    proofs: ['git-refs.txt', 'retained-parents.json', 'lock-delta.patch', 'baseline.lock', 'baseline-metadata.json', 'candidate-metadata.json', 'baseline-closure.json', 'candidate-closure.json', 'candidate-tree.txt', 'baseline-tree.txt'].map(file => [file, digest(path.join(work, file))]),
  });
}

function verify(work) {
  const manifest = read(path.join(work, 'prepared.json'));
  if (manifest.kind === 'composed-source') return verifyComposed(work, manifest);
  assert(!Object.hasOwn(manifest, 'kind'), 'Unknown prepared kind');
  assert(!fs.existsSync(path.join(work, 'composed-closure.json')) &&
    digest(path.join(work, 'sdk/Cargo.toml')) !== composedIdentity.manifest, 'Composed workspace requires composed-source prepared kind');
  assert.equal(manifest.schema, 1);
  for (const [file, expected] of manifest.scripts) assert.equal(digest(file), expected, `Changed script: ${file}`);
  for (const [dir, expected] of manifest.inputs) assert.deepEqual(inventory(path.join(work, dir)), expected, `Changed prepared ${dir}`);
  for (const [file, expected] of manifest.proofs) assert.equal(digest(path.join(work, file)), expected, `Changed proof: ${file}`);
}

function environment(work, record = work) {
  const names = ['PATH', 'PROTOC', 'PERL5LIB', 'LD_LIBRARY_PATH', 'LIBCLANG_PATH',
    'CC', 'CXX', 'CFLAGS', 'CXXFLAGS', 'LDFLAGS', 'CARGO_TARGET_DIR',
    'WASM_BUILD_WORKSPACE_HINT', 'WASM_BUILD_TOOLCHAIN', 'CARGO_BUILD_JOBS',
    'BINDGEN_EXTRA_CLANG_ARGS'];
  const cargoHome = process.env.CARGO_HOME || path.join(os.homedir(), '.cargo');
  save(path.join(record, 'build-environment.json'), {
    target: 'Linux x86_64 host; exact Rust host and versions in build-tools.txt',
    environment: Object.fromEntries(names.map(name => [name, process.env[name] ?? null])),
    globalCargoConfigs: ['config', 'config.toml'].map(name => path.join(cargoHome, name))
      .filter(file => fs.existsSync(file)).map(file => [file, digest(file)]),
    toolsReceiptSha256: digest(path.join(record, 'build-tools.txt')),
    skipWasmBuildPresent: Object.hasOwn(process.env, 'SKIP_WASM_BUILD'),
  });
}

function composedSelfTest() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'deos-composed-test-'));
  try {
    const work = path.join(root, 'source');
    const evidence = path.join(root, 'evidence');
    const fresh = path.join(root, 'fresh');
    for (const dir of [work, evidence, fresh]) fs.mkdirSync(dir);
    const proof = { sourceRoot: work, workspaceRoot: path.join(work, 'sdk'), identity: composedIdentity,
      role: sourceRole, configuration: { fixture: true }, sharedCaches: [], producers: [['fixture', hash('producer')]] };
    for (const dir of [evidence, fresh]) {
      save(path.join(dir, 'source-preflight.json'), proof);
      for (const name of sourceProofs) fs.writeFileSync(path.join(dir, name), 'fixture');
      fs.writeFileSync(path.join(dir, 'composed-metadata.json'), JSON.stringify({
        workspace_root: path.join(work, 'sdk'), target_directory: path.join(dir, 'target'),
        build_directory: path.join(dir, 'target'), packages: [{ id: 'fixture', name: 'polkadot-omni-node', version: '1',
          source: null, manifest_path: path.join(work, 'sdk/Cargo.toml') }], resolve: { nodes: [] },
      }));
      fs.writeFileSync(path.join(dir, 'composed-tree.txt'), `0polkadot-omni-node v1 (${work}/sdk)|\n`);
      fs.writeFileSync(path.join(dir, 'composed-closure.json'), JSON.stringify({ packages:
        graphSelection(read(path.join(dir, 'composed-metadata.json')), fs.readFileSync(path.join(dir, 'composed-tree.txt'), 'utf8')).selected }));
    }
    fs.writeFileSync(path.join(work, '.corrected-node-owner'), 'fixture');
    for (const name of ['git-refs.txt', 'composed-metadata.json', 'composed-tree.txt', 'composed-closure.json'])
      fs.copyFileSync(path.join(evidence, name), path.join(work, name));
    const manifest = composedManifest(work, evidence, proof);
    assertComposedState(structuredClone(manifest), manifest);
    for (const key of Object.keys(manifest)) {
      const missing = structuredClone(manifest); delete missing[key];
      assert.throws(() => assertComposedState(missing, manifest));
      assert.throws(() => assertComposedState({ ...manifest, [key]: 'drift' }, manifest));
    }
    assert.throws(() => assertComposedState({ ...manifest, unknown: true }, manifest));
    for (const key of Object.keys(composedIdentity)) {
      const changed = structuredClone(manifest); changed.identity[key] = hash('drift');
      assert.throws(() => assertComposedState(changed, manifest));
    }
    // No preparation or graph evidence alone publishes frozen state.
    assert(!fs.existsSync(path.join(work, 'prepared.json')));
    compareSourceSnapshots(evidence, fresh);
    for (const name of sourceProofs) {
      const file = path.join(fresh, name), bytes = fs.readFileSync(file);
      fs.appendFileSync(file, 'drift');
      assert.throws(() => compareSourceSnapshots(evidence, fresh));
      fs.writeFileSync(file, bytes);
    }
    for (const key of Object.keys(proof)) {
      const changed = { ...proof, [key]: 'drift' };
      fs.writeFileSync(path.join(fresh, 'source-preflight.json'), JSON.stringify(changed));
      assert.throws(() => compareSourceSnapshots(evidence, fresh));
    }
    fs.writeFileSync(path.join(fresh, 'source-preflight.json'), JSON.stringify(proof));
    const metaFile = path.join(fresh, 'composed-metadata.json');
    const metadata = read(metaFile);
    for (const key of ['workspace_root', 'target_directory', 'build_directory', 'packages', 'resolve']) {
      fs.writeFileSync(metaFile, JSON.stringify({ ...metadata, [key]: 'drift' }));
      assert.throws(() => compareSourceSnapshots(evidence, fresh));
    }
    fs.writeFileSync(metaFile, JSON.stringify(metadata));
    const sourceMetadata = path.join(work, 'composed-metadata.json');
    const originalMetadata = fs.readFileSync(sourceMetadata);
    fs.writeFileSync(sourceMetadata, JSON.stringify({ ...metadata, target_directory: path.join(work, 'target') }));
    assertComposedGraphEvidence(work, fresh);
    for (const key of ['workspace_root', 'packages', 'resolve']) {
      fs.writeFileSync(sourceMetadata, JSON.stringify({ ...metadata, [key]: 'drift' }));
      assert.throws(() => assertComposedGraphEvidence(work, fresh));
    }
    fs.writeFileSync(sourceMetadata, originalMetadata);
    const seal = path.join(evidence, 'source-consumers.json');
    fs.appendFileSync(seal, 'changed generator input');
    assert.throws(() => assertComposedState(manifest, composedManifest(work, evidence, proof)));
    fs.writeFileSync(seal, 'fixture');
    for (const name of ['.corrected-node-owner', 'git-refs.txt', 'composed-metadata.json', 'composed-tree.txt', 'composed-closure.json']) {
      const file = path.join(work, name), bytes = fs.readFileSync(file);
      fs.appendFileSync(file, 'drift');
      assert.throws(() => assertComposedState(manifest, composedManifest(work, evidence, proof)));
      fs.writeFileSync(file, bytes);
    }
    fs.unlinkSync(path.join(work, 'git-refs.txt'));
    fs.symlinkSync(path.join(evidence, 'git-refs.txt'), path.join(work, 'git-refs.txt'));
    assert.throws(() => composedManifest(work, evidence, proof), /Noncanonical/);
    fs.unlinkSync(path.join(work, 'git-refs.txt'));
    fs.writeFileSync(path.join(work, 'git-refs.txt'), 'fixture');
    // Exercise publication ordering with a fixture-only verifier. Production
    // always supplies verifyComposed and its fixed accepted source pins.
    assert.throws(() => publishSourceFreeze(work, () => { throw Error('before/after verification refused'); }));
    assert(!fs.existsSync(path.join(work, 'prepared.json')));
    assert(!fs.existsSync(path.join(work, 'source-freeze-admission')));
    fs.mkdirSync(path.join(work, 'source-freeze-admission'));
    assert.throws(() => publishSourceFreeze(work, () => manifest));
    assert(fs.existsSync(path.join(work, 'source-freeze-admission')));
    fs.rmdirSync(path.join(work, 'source-freeze-admission'));
    publishSourceFreeze(work, () => { compareSourceSnapshots(evidence, fresh); return manifest; });
    assert.deepEqual(read(path.join(work, 'prepared.json')), manifest);
    assert.throws(() => publishSourceFreeze(work, () => manifest), /Refusing prepared/);
    // Build verification cannot route composed/missing/unknown kinds to legacy.
    fs.writeFileSync(path.join(work, 'prepared.json'), JSON.stringify({ schema: 1, kind: 'unknown' }));
    assert.throws(() => verify(work), /Unknown prepared kind/);
    fs.writeFileSync(path.join(work, 'prepared.json'), JSON.stringify({ schema: 1 }));
    assert.throws(() => verify(work), /Composed workspace requires/);
    fs.writeFileSync(path.join(work, 'prepared.json'), JSON.stringify({ schema: 1, kind: composedKind }));
    assert.throws(() => verify(work), /Missing composed evidence root/);
    // Source evidence schema fails closed before it can acquire authority.
    assert.throws(() => sourceEvidence(work, evidence), /Unknown\/missing source evidence fields/);
    assert.throws(() => sourceEvidence(work, work), /Evidence must be outside/);
    // Neither a graph directory nor a missing proof may be presented as a
    // prepared composed source, and a path target cannot replace its seal.
    fs.unlinkSync(path.join(work, 'prepared.json'));
    assert.throws(() => verify(work));
    fs.symlinkSync(path.join(root, 'missing'), path.join(work, 'prepared.json'));
    assert.throws(() => publishSourceFreeze(work, () => manifest), /Refusing prepared/);
    fs.unlinkSync(path.join(work, 'prepared.json'));
    fs.mkdirSync(path.join(work, 'attempts'));
    assert.throws(() => publishSourceFreeze(work, () => manifest), /Refusing prepared/);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
  console.log('Composed manifest/snapshot/publish/routing fixture self-tests passed (no accepted source freeze)');
}

function selfTest() {
  composedSelfTest();
  const meta = { packages: [{ id: 'root', name: 'polkadot-omni-node' }, { id: 'a' }, { id: 'dev' }], resolve: { nodes: [
    { id: 'root', deps: [{ pkg: 'a', dep_kinds: [{ kind: 'build' }] }, { pkg: 'dev', dep_kinds: [{ kind: 'dev' }] }] },
    { id: 'a', deps: [] }, { id: 'dev', deps: [] },
  ] } };
  assert.deepEqual([...closure(meta).reached], ['root', 'a']);
  assert.notEqual(hash('before'), hash('after'));
  const compiler = [
    { name: 'polkadot-omni-node', features: ['runtime-benchmarks'] },
    { name: 'wasmtime', version: '36.0.20' },
    { name: 'wasmtime-internal-cranelift', version: '36.0.20' },
    { name: 'cranelift-codegen', version: '0.123.20' },
  ];
  assertCompilerProfile(compiler);
  for (const [index, version] of [[1, '36.0.7'], [2, '36.0.7'], [3, '0.123.7'], [3, '0.123.19']]) {
    assert.throws(() => assertCompilerProfile(compiler.map((pkg, i) => i === index ? { ...pkg, version } : pkg)), /Uncorrected compiler/);
  }
  assert.throws(() => assertCompilerProfile(compiler.slice(0, 3)), /Missing compiler family/);
  assert.throws(() => assertCompilerProfile(compiler.map((pkg, i) => i ? pkg : { ...pkg, features: [] })), /Missing Omni runtime-benchmarks/);
  assert.equal(parentSelections.length, 9);
  assert.equal(compositionSources.length, 5);
  assert.equal(compositionPatches.length, 4);
  assertComposedIdentity({ ...composedIdentity });
  for (const key of Object.keys(composedIdentity))
    assert.throws(() => assertComposedIdentity({ ...composedIdentity, [key]: hash('unreviewed') }), /Unexpected exploratory composition delta/);
  const parent = `\nname = "prost-build"\nversion = "0.13.2"\nsource = "${registry}"\nchecksum = "fixture"\ndependencies = [\n "heck 0.4.1",\n]\n\n`;
  const retained = parent.replace('heck 0.4.1', 'heck 0.5.0');
  const target = version => `[[package]]\nname = "heck"\nversion = "${version}"\nsource = "${registry}"\n\n`;
  const lock = 'version = 4\n\n[[package]]' + parent + target('0.4.1') + target('0.5.0');
  const manifest = { packages: [{ name: 'prost-build', version: '0.13.2', source: registry,
    dependencies: [{ name: 'heck', source: registry, req: '>=0.4, <=0.5', kind: null, target: null }] }] };
  const selection = [['prost-build', '0.13.2', 'heck', '0.4.1', '0.5.0', '>=0.4, <=0.5', hash(parent), hash(retained)]];
  const retain = (text, meta = manifest, rules = selection) => retainedParentLock(text, meta, rules);
  assert.equal(retain(lock), lock.replace(parent, retained));
  for (const bad of [lock.replace('prost-build', 'changed'), lock.replace('0.13.2', '0.13.3'),
    lock.replace(registry, 'registry+unreviewed'), lock.replace('checksum = "fixture"', 'checksum = "changed"'),
    lock.replace(' "heck 0.4.1",', ''), lock.replace(parent, parent + '[[package]]' + parent),
    lock.replace(target('0.5.0'), ''), lock.replace(target('0.5.0'), target('0.5.0').replace(registry, 'registry+unreviewed'))]) {
    assert.throws(() => retain(bad));
  }
  // Even a self-consistent old block fingerprint cannot authorize a missing edge.
  const missing = parent.replace(' "heck 0.4.1",', '');
  assert.throws(() => retain(lock.replace(parent, missing), manifest,
    [[...selection[0].slice(0, 6), hash(missing), hash(retained)]]), /Missing or duplicate retained edge/);
  assert.throws(() => retain(lock, { packages: [] }), /Missing retained parent manifest/);
  for (const field of ['req', 'source', 'kind', 'target']) {
    const changed = structuredClone(manifest);
    changed.packages[0].dependencies[0][field] = 'changed';
    assert.throws(() => retain(lock, changed));
  }
  assert.throws(() => retain(lock, manifest, [[...selection[0].slice(0, 7), hash('changed')]]), /Changed retained block/);
  assert.throws(() => assertProviderGraph('/fixture', []), /Missing or duplicate selected/);
  const fixture = fs.mkdtempSync(path.join(os.tmpdir(), 'deos-corrected-node-test-'));
  try {
    const sdk = path.join(fixture, 'sdk');
    const vendor = path.join(fixture, 'vendor');
    fs.mkdirSync(sdk); fs.mkdirSync(vendor);
    fs.writeFileSync(path.join(sdk, 'Cargo.toml'), '[workspace]\n[patch.crates-io]\n');
    fs.writeFileSync(path.join(sdk, 'Cargo.lock'), 'locked');
    const script = path.join(fixture, 'builder.sh');
    fs.writeFileSync(script, 'original');
    fs.mkdirSync(path.join(sdk, path.dirname(macro)), { recursive: true });
    fs.writeFileSync(path.join(sdk, macro), 'wrong source');
    assert.throws(() => patch(sdk, vendor), /Unexpected SDK macro/);
    for (const file of ['git-refs.txt', 'retained-parents.json', 'lock-delta.patch', 'baseline-metadata.json', 'candidate-metadata.json', 'baseline-tree.txt', 'candidate-tree.txt', 'baseline.lock']) fs.writeFileSync(path.join(fixture, file), '{}');
    for (const label of ['baseline', 'candidate']) save(path.join(fixture, `${label}-closure.json`), { packages: [] });
    freeze(fixture, script, script);
    verify(fixture);
    save(path.join(fixture, 'composed-closure.json'), { packages: [] });
    assert.throws(() => freeze(fixture, script, script), /candidate-era freeze forbidden/);
    fs.unlinkSync(path.join(fixture, 'composed-closure.json'));
    save(path.join(fixture, 'source-preflight.json'), {});
    assert.throws(() => freeze(fixture, script, script), /candidate-era freeze forbidden/);
    fs.unlinkSync(path.join(fixture, 'source-preflight.json'));
    const home = path.join(fixture, 'home');
    fs.mkdirSync(home);
    for (const env of [{ HOME: home + '/.' }, { CARGO_HOME: '.' }, { RUSTUP_HOME: 'alternate' }, { RUSTUP_TOOLCHAIN: 'stable' },
      { SKIP_WASM_BUILD: '' }, { RUSTFLAGS: '' }, { CARGO_ENCODED_RUSTFLAGS: '--cfg=feature=log' },
      { RUSTC: 'wrapper' }, { RUSTC_WRAPPER: '' }, { CARGO_PROFILE_PRODUCTION_LTO: 'off' },
      { CARGO_BUILD_RUSTFLAGS: '--cfg=feature=asm' }, { CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS: '--cfg=dimpl' },
      { CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: 'wrapper' }, { WASM_BUILD_TOOLCHAIN: 'stable' }]) {
      assert.throws(() => sourceConfiguration(sdk, env, home), /Unsupported source/);
    }
    assert.throws(() => sourceConfiguration(sdk, {}, home), /Missing source Cargo config/);
    fs.mkdirSync(path.join(fixture, '.cargo'));
    fs.writeFileSync(path.join(fixture, '.cargo/config'), '[build]\nrustflags=[]');
    // The reviewed child config is absent; neither an ancestor nor a new home
    // may silently supply it. Both must fail rather than recording new policy.
    assert.throws(() => sourceConfiguration(sdk, {}, home), /Missing source Cargo config|Unreviewed source Cargo config/);
    fs.rmSync(path.join(fixture, '.cargo'), { recursive: true });
    const consumer = path.join(fixture, 'consumer');
    fs.mkdirSync(consumer);
    fs.writeFileSync(path.join(consumer, 'Cargo.toml'), 'manifest');
    fs.writeFileSync(path.join(consumer, 'build.rs'), 'generator');
    const seal = inventory(consumer);
    for (const file of ['build.rs', 'unknown-consumer.rs', 'generator-input.bin', 'Cargo.toml']) {
      const existed = fs.existsSync(path.join(consumer, file));
      const before = existed ? fs.readFileSync(path.join(consumer, file)) : null;
      fs.writeFileSync(path.join(consumer, file), 'changed');
      assert.notDeepEqual(inventory(consumer), seal, `Unsealed consumer input: ${file}`);
      if (existed) fs.writeFileSync(path.join(consumer, file), before); else fs.unlinkSync(path.join(consumer, file));
    }
    assert.deepEqual(inventory(consumer), seal);
    fs.appendFileSync(path.join(sdk, 'Cargo.toml'), '# changed');
    assert.throws(() => verify(fixture), /Changed prepared sdk/);
    fs.writeFileSync(path.join(sdk, 'Cargo.toml'), '[workspace]\n[patch.crates-io]\n');
    verify(fixture);
    fs.appendFileSync(script, 'changed');
    assert.throws(() => verify(fixture), /Changed script/);
    fs.writeFileSync(script, 'original');
    fs.appendFileSync(path.join(fixture, 'candidate-metadata.json'), 'changed');
    assert.throws(() => verify(fixture), /Changed proof/);
    fs.writeFileSync(path.join(sdk, 'Cargo.toml'), [
      'anyhow = { version = "1.0.81", default-features = false }',
      'memmap2 = { version = "0.9.3" }', 'rustix = { version = "1.0.3" }',
      'tokio = { version = "1.45.0" }', 'tracing-subscriber = { version = "=0.3.19" }',
    ].join('\n'));
    composeMinimums(fixture);
    assert(fs.readFileSync(path.join(sdk, 'Cargo.toml'), 'utf8').includes('version = "=0.3.20"'));
    assert.throws(() => composeMinimums(fixture), /Changed manifest minimum/);
    assert.throws(() => assertCompositionSources(fixture));
    for (const [dir] of compositionSources) fs.cpSync(new URL(`../template/vendor/${dir}/`, import.meta.url), path.join(vendor, dir), { recursive: true });
    assertCompositionSources(fixture);
    fs.appendFileSync(path.join(vendor, 'lru-0.7.8/src/lib.rs'), '// drift');
    assert.throws(() => assertCompositionSources(fixture), /Changed composed source/);
    fs.writeFileSync(path.join(sdk, 'Cargo.toml'), '[workspace]\n[patch.crates-io]\n');
    const providerDir = path.join(vendor, provider);
    fs.cpSync(new URL('../template/vendor/str0m-openssl/', import.meta.url), providerDir, { recursive: true });
    assertProviderSource(vendor);
    patchProvider(sdk, vendor);
    assert.throws(() => patchProvider(sdk, vendor), /Duplicate provider patch/);
    fs.appendFileSync(path.join(providerDir, 'src/dtls_ossl.rs'), '// drift');
    assert.throws(() => assertProviderSource(vendor), /Changed reviewed provider source/);
    const id = name => `fixture#${name}`;
    const chosen = [
      { id: id(provider), name: provider, version: '0.6.0', source: null, manifest: path.join(fixture, `vendor/${provider}/Cargo.toml`), features: ['vendored'], parents: [id('str0m')] },
      { id: id('str0m'), name: 'str0m', version: '0.23.1', features: ['openssl', 'vendored'], parents: [id('litep2p')] },
      { id: id('litep2p'), name: 'litep2p', version: '0.15.3', features: ['webrtc'], parents: [id('sc-network')] },
      { id: id('sc-network'), name: 'sc-network', version: '0.60.0', parents: [] },
    ];
    assertProviderGraph(fixture, chosen);
    fs.writeFileSync(path.join(fixture, 'baseline-metadata.json'), JSON.stringify({ packages: [
      { name: provider, version: '0.6.0', source: registry, id: `${registry}#${provider}@0.6.0`, manifest_path: '/registry/str0m-openssl/Cargo.toml' },
    ] }));
    assert.throws(() => assertRetainedGraph(fixture, { packages: [
      { name: provider, id: id(provider), source: null, manifest_path: chosen[0].manifest },
    ], resolve: { nodes: [] } }, chosen), /Unrelated host graph delta/);
    assert.throws(() => assertProviderGraph(fixture, [...chosen, chosen[0]]), /duplicate selected/);
    assert.throws(() => assertProviderGraph(fixture, chosen.map((pkg, i) => i ? pkg : { ...pkg, source: registry })), /Unpatched registry provider/);
    assert.throws(() => assertProviderGraph(fixture, chosen.map((pkg, i) => i ? pkg : { ...pkg, features: ['dimpl', 'vendored'] })), /not dimpl/);
    const treeMeta = { packages: [{ id: 'root', name: 'polkadot-omni-node', version: '1', manifest_path: '/sdk/Cargo.toml' },
      { id: 'a', name: 'a', version: '1', source: 'registry+fixture', manifest_path: '/registry/a/Cargo.toml' },
      { id: 'unused', name: 'unused', version: '1', manifest_path: '/sdk/unused/Cargo.toml' }] };
    const actual = treeClosure(treeMeta, '0polkadot-omni-node v1 (/sdk)|default\n1a v1 (proc-macro) (/registry/a)|x\n1a v1|y (*)\n');
    assert.deepEqual([...actual.reached], ['root', 'a']);
    assert.deepEqual(actual.nodes.get('a').features, ['x', 'y']);
    assert.equal(actual.nodes.get('root').deps[0].pkg, 'a');
  } finally { fs.rmSync(fixture, { recursive: true, force: true }); }
  console.log('Source precondition, exact parent retention/requirements, tree/features, freeze and script/source/graph invalidation self-tests passed');
}

const [mode, ...args] = process.argv.slice(2);
if (mode === 'patch') patch(...args);
else if (mode === 'patch-provider') patchProvider(...args);
else if (mode === 'retain-parents') retainParents(...args);
else if (mode === 'compose-patches') composePatches(...args);
else if (mode === 'compose-minimums') composeMinimums(...args);
else if (mode === 'compose-retain') composeRetain(...args);
else if (mode === 'compose-report') composeReport(...args);
else if (mode === 'graph') graph(...args);
else if (mode === 'source-preflight') sourcePreflight(...args);
else if (mode === 'freeze') freeze(...args);
else if (mode === 'freeze-existing') freezeComposed(...args);
else if (mode === 'verify') verify(...args);
else if (mode === 'environment') environment(...args);
else if (mode === 'self-test') selfTest();
else throw Error('Internal support mode required');
