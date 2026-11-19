import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import os from 'node:os';
import { gunzipSync } from 'node:zlib';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const owner = path.dirname(fileURLToPath(import.meta.url));
const sourcePath = path.join(owner, 'source.json');
function readRegular(file, encoding) {
    requireThat(fs.lstatSync(file).isFile(), `Not a regular non-symlink file: ${file}`);
    const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
    try {
        requireThat(fs.fstatSync(fd).isFile(), `Not a regular file: ${file}`);
        return fs.readFileSync(fd, encoding);
    } finally { fs.closeSync(fd); }
}
const expected = JSON.parse(readRegular(sourcePath, 'utf8'));
const digest = value => crypto.createHash('sha256').update(value).digest('hex');
const hash = file => digest(readRegular(file));
const json = file => JSON.parse(readRegular(file, 'utf8'));
const save = (file, value) => fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, { flag: 'wx' });
function requireThat(condition, message) { if (!condition) throw new Error(message); }

function hasEntry(file) {
    try { fs.lstatSync(file); return true; } catch (error) { if (error.code === 'ENOENT') return false; throw error; }
}

function inventory(directory, excludeBuildState = false) {
    requireThat(fs.lstatSync(directory).isDirectory(), `Not a plain source directory: ${directory}`);
    const rows = [];
    function walk(dir) {
        for (const entry of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name, 'en'))) {
            // Only package-root build output/VCS state is not source. Registry and
            // root archive checks additionally reject any unarchived payload.
            if (excludeBuildState && dir === directory && ['target', '.git'].includes(entry.name)) continue;
            const file = path.join(dir, entry.name);
            if (entry.isDirectory()) walk(file);
            else if (entry.isFile()) rows.push([path.relative(directory, file), hash(file), fs.lstatSync(file).mode & 0o777]);
            else throw new Error(`Unsupported source entry: ${file}`);
        }
    }
    walk(directory);
    return rows;
}

function tree(directory) {
    return digest(JSON.stringify(inventory(directory, true).map(([file, sha256]) => [file, sha256])));
}

function callerClosure(work) {
    const root = path.resolve(owner, '../../..');
    const files = ['scripts/02-install-tools.sh', 'scripts/04-generate-chain-spec.sh', 'scripts/_common.sh', 'scripts/inputs/chain-spec-builder/provenance.mjs', 'scripts/inputs/chain-spec-builder/source.json', '.node-version'];
    const closure = Object.fromEntries(files.map(file => [file, hash(path.join(root, file))]));
    if (work) requireThat(hash(path.join(work, 'common-loaded.sh')) === closure['scripts/_common.sh'], 'Loaded common helper differs from canonical helper');
    return closure;
}

// Cargo's registry extraction has no checksum sidecar on every supported Cargo.
// Compare payload bytes directly with the lock-checksummed crate instead.
function verifyExtraction(archive, directory, overrides = new Map()) {
    const tar = gunzipSync(readRegular(archive));
    const files = new Set();
    let longName;
    for (let offset = 0; offset + 512 <= tar.length;) {
        const header = tar.subarray(offset, offset + 512);
        if (header.every(byte => byte === 0)) break;
        const field = (start, end) => header.subarray(start, end).toString().split('\0')[0];
        const size = Number.parseInt(field(124, 136).trim() || '0', 8);
        requireThat(Number.isSafeInteger(size) && size >= 0 && offset + 512 + size <= tar.length, 'Invalid crate tar size');
        const payload = tar.subarray(offset + 512, offset + 512 + size);
        const type = field(156, 157);
        const prefix = field(345, 500);
        const name = longName ?? (prefix ? `${prefix}/${field(0, 100)}` : field(0, 100));
        longName = undefined;
        offset += 512 + Math.ceil(size / 512) * 512;
        if (type === 'L') { longName = payload.toString().split('\0')[0]; continue; }
        requireThat(type === '' || type === '0' || type === '5', `Unsupported crate tar record: ${type}`);
        const relative = name.split('/').slice(1).join('/');
        requireThat(!path.isAbsolute(relative) && !relative.split('/').includes('..'), 'Invalid crate path');
        if (type === '5' || !relative) continue;
        requireThat(!files.has(relative), `Duplicate crate entry: ${relative}`);
        files.add(relative);
        const file = path.join(directory, relative);
        requireThat(fs.lstatSync(file).isFile() && hash(file) === crypto.createHash('sha256').update(overrides.get(relative) ?? payload).digest('hex'), `Registry source drift: ${file}`);
    }
    function checkExtras(dir) {
        for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
            const file = path.join(dir, entry.name);
            if (entry.isDirectory()) checkExtras(file);
            else requireThat(files.has(path.relative(directory, file)) || ['.cargo-ok', '.cargo-checksum.json'].includes(path.relative(directory, file)), `Unarchived registry input: ${file}`);
        }
    }
    checkExtras(directory);
    return [...files].sort();
}

function exactKeys(value, keys, label) {
    assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), `Unsupported ${label} schema`);
}

function maintenanceConstruction(contract) {
    exactKeys(contract, ['schema', 'role', 'baseVersion', 'originalManifestSha256', 'originalLockSha256', 'lockFile', 'lockSha256', 'patches', 'sourceReview'], 'maintenance');
    exactKeys(contract.sourceReview, ['status', 'targets', 'expiresAt', 'scope', 'constructionBasisSha256'], 'source review');
    for (const patch of contract.patches) exactKeys(patch, ['name', 'directory', 'sha256'], 'patch');
    // Only this named approval object is outside construction identity. The raw
    // contract hash and object remain in the attempt snapshot and receipt.
    return { schema: contract.schema, role: contract.role, baseVersion: contract.baseVersion,
        originalManifestSha256: contract.originalManifestSha256, originalLockSha256: contract.originalLockSha256,
        lockFile: contract.lockFile, lockSha256: contract.lockSha256, patches: contract.patches };
}

function maintenanceInputs() {
    const contract = json(path.join(owner, 'maintenance.json'));
    maintenanceConstruction(contract);
    requireThat(contract.schema === 1 && contract.role === expected.role && contract.baseVersion === expected.version, 'Wrong maintenance role/version');
    requireThat(contract.originalLockSha256 === expected.lockSha256, 'Maintenance must retain the original archive lock identity');
    requireThat(contract.lockFile === 'maintenance.lock' && hash(path.join(owner, contract.lockFile)) === contract.lockSha256, 'Maintenance lock drift');
    const directories = ['template/vendor/hickory-proto', 'template/vendor/lru-0.12.3', 'template/vendor/webpki-roots'];
    requireThat(JSON.stringify(contract.patches.map(patch => patch.directory)) === JSON.stringify(directories), 'Unsupported maintenance source');
    const patches = contract.patches.map(patch => {
        requireThat(/^[a-z][a-z0-9-]*$/.test(patch.name), 'Invalid maintenance package');
        const directory = path.resolve(owner, '../../..', patch.directory);
        requireThat(tree(directory) === patch.sha256, `Maintenance source drift: ${patch.name}`);
        return { ...patch, path: directory };
    });
    return { contract, contractSha256: hash(path.join(owner, 'maintenance.json')), patches };
}

function maintainedManifest(original, patches) {
    requireThat(!original.includes('[patch.'), 'Unexpected original source patch');
    return `${original}\n[patch.crates-io]\n${patches.map(patch => `${patch.name} = { path = ${JSON.stringify(patch.path)} }`).join('\n')}\n`;
}

function prepareSource(work) {
    const source = path.join(work, `${expected.package}-${expected.version}`);
    requireThat(hash(path.join(work, 'source.crate')) === expected.archiveSha256, 'Wrong builder archive');
    verifyExtraction(path.join(work, 'source.crate'), source);
    const inputs = maintenanceInputs();
    requireThat(hash(path.join(source, 'Cargo.toml')) === inputs.contract.originalManifestSha256 && hash(path.join(source, 'Cargo.lock')) === expected.lockSha256, 'Wrong original source/lock');
    for (const name of ['Cargo.toml', 'Cargo.lock']) fs.copyFileSync(path.join(source, name), path.join(work, `original-${name}`), fs.constants.COPYFILE_EXCL);
    fs.writeFileSync(path.join(source, 'Cargo.toml'), maintainedManifest(readRegular(path.join(work, 'original-Cargo.toml'), 'utf8'), inputs.patches));
    fs.copyFileSync(path.join(owner, 'maintenance.lock'), path.join(source, 'Cargo.lock'));
    save(path.join(work, 'maintenance-inputs.json'), inputs);
}

function preparedMaintenance(work, source) {
    const inputs = maintenanceInputs();
    assert.deepEqual(json(path.join(work, 'maintenance-inputs.json')), inputs, 'Changed maintenance inputs');
    requireThat(hash(path.join(work, 'original-Cargo.toml')) === inputs.contract.originalManifestSha256 && hash(path.join(work, 'original-Cargo.lock')) === expected.lockSha256, 'Changed original manifest/lock');
    requireThat(hash(path.join(source, 'Cargo.lock')) === inputs.contract.lockSha256, 'Wrong maintained lock');
    const manifest = maintainedManifest(readRegular(path.join(work, 'original-Cargo.toml'), 'utf8'), inputs.patches);
    requireThat(readRegular(path.join(source, 'Cargo.toml'), 'utf8') === manifest, 'Wrong maintained manifest');
    verifyExtraction(path.join(work, 'source.crate'), source, new Map([
        ['Cargo.toml', manifest], ['Cargo.lock', readRegular(path.join(owner, 'maintenance.lock'))]
    ]));
    return inputs;
}

function snapshot(work) {
    const source = path.join(work, `${expected.package}-${expected.version}`);
    requireThat(hash(path.join(work, 'source.crate')) === expected.archiveSha256, 'Wrong builder archive');
    const maintenance = preparedMaintenance(work, source);
    const manifest = readRegular(path.join(source, 'Cargo.toml'), 'utf8');
    requireThat(manifest.includes(`name = "${expected.package}"`) && manifest.includes(`version = "${expected.version}"`), 'Wrong builder version');
    requireThat(json(path.join(source, '.cargo_vcs_info.json')).git.sha1 === expected.revision, 'Wrong builder revision');
    const metadata = json(path.join(work, 'metadata.json'));
    const lock = readRegular(path.join(source, 'Cargo.lock'), 'utf8');
    const lockPackages = lock.split('[[package]]').slice(1).map(block => Object.fromEntries([...block.matchAll(/^(name|version|checksum|source) = "([^"]+)"$/gm)].map(match => [match[1], match[2]])));
    const packages = metadata.packages.map(pkg => {
        let archiveSha256 = null;
        if (!pkg.source && path.dirname(pkg.manifest_path) !== source) {
            const patch = maintenance.patches.find(patch => patch.path === path.dirname(pkg.manifest_path));
            requireThat(patch && tree(patch.path) === patch.sha256, `Unknown local package: ${pkg.id}`);
        }
        if (pkg.source) {
            requireThat(pkg.source === 'registry+https://github.com/rust-lang/crates.io-index', `Unsupported package source: ${pkg.id}`);
            const pin = lockPackages.find(pin => pin.name === pkg.name && pin.version === pkg.version && pin.source === pkg.source);
            requireThat(pin?.checksum, `Missing lock pin: ${pkg.id}`);
            const archive = path.join(path.dirname(pkg.manifest_path).replace('/registry/src/', '/registry/cache/'), '..', `${pkg.name}-${pkg.version}.crate`);
            archiveSha256 = hash(archive);
            requireThat(archiveSha256 === pin.checksum, `Registry archive drift: ${pkg.id}`);
            verifyExtraction(archive, path.dirname(pkg.manifest_path));
        }
        const directory = path.dirname(pkg.manifest_path);
        const files = inventory(directory, maintenance.patches.some(patch => patch.path === directory));
        return { id: pkg.id, source: pkg.source, archiveSha256, digest: digest(JSON.stringify(files.map(([file, sha256]) => [file, sha256]))), files };
    }).sort((a, b) => a.id.localeCompare(b.id));
    const environment = json(path.join(work, 'environment.json'));
    for (const [key, value] of Object.entries(expected.profileEnvironment)) requireThat(environment[key] === value, `Wrong profile: ${key}`);
    requireThat(environment.RUSTFLAGS === expected.rustflags && !environment.CARGO_ENCODED_RUSTFLAGS && !environment.RUSTC_WRAPPER && !environment.RUSTC_WORKSPACE_WRAPPER, 'Unsupported compiler flags/wrapper');
    requireThat(environment.CARGO_BUILD_JOBS === '2', 'Expected jobs=2');
    requireThat(expected.targets.includes(environment.CARGO_BUILD_TARGET), 'Unsupported target');
    const rustc = readRegular(path.join(work, 'rustc.txt'), 'utf8');
    requireThat(rustc.includes(`release: ${expected.toolchain}\n`) && rustc.includes(`commit-hash: ${expected.rustcCommit}\n`), 'Wrong compiler');
    const files = [sourcePath, fileURLToPath(import.meta.url), path.resolve(owner, '../../02-install-tools.sh'), path.resolve(owner, '../../04-generate-chain-spec.sh'), path.join(work, 'common-loaded.sh')];
    const tools = json(path.join(work, 'tools.json'));
    exactKeys(tools, ['rustc', 'cargo', 'node', 'cc', 'c++', 'perl', 'make'], 'tools');
    for (const [name, tool] of Object.entries(tools)) {
        exactKeys(tool, ['invocationPath', 'path', 'sha256'], 'tool');
        requireThat(fs.realpathSync(tool.invocationPath) === tool.path && hash(tool.path) === tool.sha256, `Changed tool/resolution: ${name}`);
    }
    const cargoHome = environment.CARGO_HOME;
    for (let dir = source; ; dir = path.dirname(dir)) {
        for (const config of ['.cargo/config', '.cargo/config.toml']) requireThat(!hasEntry(path.join(dir, config)), `Undeclared Cargo config: ${dir}/${config}`);
        if (dir === path.dirname(dir)) break;
    }
    for (const config of ['config', 'config.toml']) requireThat(!hasEntry(path.join(cargoHome, config)), `Undeclared CARGO_HOME/${config}`);
    const inputs = {};
    for (const key of ['PROTOC', 'PERL5LIB', 'LD_LIBRARY_PATH', 'LIBCLANG_PATH']) {
        if (!environment[key]) continue;
        inputs[key] = environment[key].split(':').map(input => {
            requireThat(path.isAbsolute(input) && fs.existsSync(input), `Invalid ${key}`);
            return [input, fs.lstatSync(input).isDirectory() ? { files: inventory(input) } : { sha256: hash(input) }];
        });
    }
    const captured = {
        schema: 1, role: expected.role, version: expected.version, archiveSha256: expected.archiveSha256,
        lockSha256: maintenance.contract.lockSha256, maintenance, sourceSha256: tree(source), expectedSha256: hash(sourcePath),
        metadataSha256: hash(path.join(work, 'metadata.json')), featuresSha256: hash(path.join(work, 'features.txt')),
        command: ['cargo', 'build', '--frozen', '--release', '--bin', 'chain-spec-builder', '--message-format=json'],
        environment, tools, inputs, packages,
        producer: Object.fromEntries(files.map(file => [file, hash(file)])),
        compiler: rustc, cargo: readRegular(path.join(work, 'cargo.txt'), 'utf8'),
        host: readRegular(path.join(work, 'host.txt'), 'utf8'),
        cfg: readRegular(path.join(work, 'cfg.txt'), 'utf8'),
        callerClosure: callerClosure(work)
    };
    return { ...captured, constructionBasisSha256: basisDigest(constructionBasis(work, captured)) };
}

function constructionBasis(work, captured) {
    const source = path.join(work, `${expected.package}-${expected.version}`);
    const rootId = `path+file://${source}#${expected.package}@${expected.version}`;
    const canonicalId = id => id === rootId ? `path+file://$SOURCE#${expected.package}@${expected.version}` : id;
    const metadata = json(path.join(work, 'metadata.json'));
    requireThat(metadata.workspace_root === source && metadata.target_directory === path.join(work, 'target') && metadata.build_directory === path.join(work, 'target'), 'Wrong metadata workspace/target/build directories');
    exactKeys(metadata, ['packages', 'workspace_members', 'workspace_default_members', 'resolve', 'target_directory', 'build_directory', 'version', 'workspace_root', 'metadata'], 'metadata');
    const graph = structuredClone(metadata);
    graph.workspace_root = '$SOURCE';
    graph.target_directory = '$WORK/target';
    graph.build_directory = '$WORK/target';
    graph.workspace_members = graph.workspace_members.map(canonicalId);
    graph.workspace_default_members = graph.workspace_default_members.map(canonicalId);
    graph.resolve.root = canonicalId(graph.resolve.root);
    for (const node of graph.resolve.nodes) {
        node.id = canonicalId(node.id);
        node.dependencies = node.dependencies.map(canonicalId);
        for (const dep of node.deps) dep.pkg = canonicalId(dep.pkg);
    }
    for (const pkg of graph.packages) {
        if (pkg.id !== rootId) continue;
        requireThat(pkg.manifest_path === path.join(source, 'Cargo.toml'), 'Wrong root manifest path');
        pkg.id = canonicalId(pkg.id);
        pkg.manifest_path = '$SOURCE/Cargo.toml';
        for (const target of pkg.targets) {
            const relative = path.relative(source, target.src_path);
            requireThat(!relative.startsWith('../') && !path.isAbsolute(relative), 'Root target escapes source');
            target.src_path = `$SOURCE/${relative}`;
        }
    }
    const environment = captured.environment;
    const keys = ['HOME', 'CARGO_HOME', 'PATH', 'LANG', 'LC_ALL', 'TZ', 'CARGO_BUILD_TARGET', 'CARGO_BUILD_JOBS', 'CARGO_TARGET_DIR', 'RUSTFLAGS', ...Object.keys(expected.profileEnvironment)];
    const optional = ['PROTOC', 'PERL5LIB', 'LD_LIBRARY_PATH', 'LIBCLANG_PATH'].filter(key => Object.hasOwn(environment, key));
    exactKeys(environment, [...keys, ...optional], 'build environment');
    requireThat(environment.HOME === path.join(work, 'home') && environment.CARGO_TARGET_DIR === path.join(work, 'target'), 'Wrong private HOME/target');
    const firstLine = `${expected.package} v${expected.version} (${source})`;
    const features = readRegular(path.join(work, 'features.txt'), 'utf8').split('\n');
    requireThat(features[0] === firstLine, 'Wrong feature graph root');
    features[0] = `${expected.package} v${expected.version} ($SOURCE)`;
    return {
        schema: 1, role: captured.role, version: captured.version,
        sourceContractSha256: captured.expectedSha256, archiveSha256: captured.archiveSha256,
        maintenance: maintenanceConstruction(captured.maintenance.contract),
        patches: captured.maintenance.patches, lockSha256: captured.lockSha256,
        sourceSha256: captured.sourceSha256,
        packages: captured.packages.map(pkg => ({ id: canonicalId(pkg.id), source: pkg.source, archiveSha256: pkg.archiveSha256, digest: pkg.digest, files: pkg.files })),
        metadata: graph, featureGraph: features.join('\n'),
        command: captured.command, environment: { ...environment, HOME: '$WORK/home', CARGO_TARGET_DIR: '$WORK/target' },
        tools: captured.tools, configurationInputs: captured.inputs,
        cargoConfiguration: { ancestorOverrides: 'absent', cargoHomeOverrides: 'absent', cargoHome: environment.CARGO_HOME },
        compiler: captured.compiler, cargo: captured.cargo, host: captured.host, cfg: captured.cfg,
        callerClosure: captured.callerClosure
    };
}

function basisDigest(basis) { return digest(JSON.stringify(basis)); }

function sourceAdmission(work, captured = snapshot(work)) {
    const review = maintenanceInputs().contract.sourceReview;
    const basis = constructionBasis(work, captured);
    requireThat(review.status === 'accepted' && expected.targets.includes(captured.environment.CARGO_BUILD_TARGET) && review.targets.includes(captured.environment.CARGO_BUILD_TARGET), 'Independent target-specific Builder19 composed-source review is required BEFORE compilation; use --builder-source-only');
    requireThat(beforeExpiry(review.expiresAt) && review.expiresAt === '2026-12-12', 'Missing, invalid or expired Builder source approval');
    requireThat(/^[a-f0-9]{64}$/.test(review.constructionBasisSha256) && basisDigest(basis) === review.constructionBasisSha256, 'Builder construction basis differs from independently reviewed project approval');
    return basis;
}

function beforeExpiry(expiresAt, now = Date.now()) {
    if (typeof expiresAt !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(expiresAt)) return false;
    const deadline = Date.parse(`${expiresAt}T00:00:00Z`);
    return Number.isFinite(deadline) && new Date(deadline).toISOString().slice(0, 10) === expiresAt && now < deadline;
}

function checkReceipt(receipt, binary, acceptance, requireSecurity = true) {
    requireThat(receipt.schema === 1 && receipt.role === expected.role, 'Wrong receipt role/schema');
    requireThat(receipt.exit === 0 && receipt.complete === true, 'Incomplete/failed producer');
    const lockSha256 = receipt.inputs.maintenance ? maintenanceInputs().contract.lockSha256 : expected.lockSha256;
    if (receipt.inputs.maintenance) assert.deepEqual(receipt.inputs.maintenance, maintenanceInputs(), 'Changed maintenance contract/source');
    requireThat(receipt.inputs.version === expected.version && receipt.inputs.archiveSha256 === expected.archiveSha256 && receipt.inputs.lockSha256 === lockSha256, 'Wrong source/version');
    requireThat(receipt.inputs.expectedSha256 === hash(sourcePath), 'Changed expected source contract');
    requireThat(receipt.inputs.compiler.includes(`commit-hash: ${expected.rustcCommit}\n`) && receipt.inputs.compiler.includes(`release: ${expected.toolchain}\n`), 'Wrong compiler');
    requireThat(receipt.inputs.environment.RUSTFLAGS === expected.rustflags, 'Wrong flags');
    for (const [key, value] of Object.entries(expected.profileEnvironment)) requireThat(receipt.inputs.environment[key] === value, `Wrong profile: ${key}`);
    requireThat(expected.targets.includes(receipt.inputs.environment.CARGO_BUILD_TARGET), 'Wrong target');
    requireThat(acceptance.schema === 1 && acceptance.role === expected.role && acceptance.receiptSha256 === receipt._sha256, 'Unknown producer receipt; caller-selected hashes are not authority');
    requireThat(acceptance.subjectSha256 === hash(binary) && receipt.subject.sha256 === acceptance.subjectSha256, 'Wrong subject digest');
    requireThat(receipt.subject.target === receipt.inputs.environment.CARGO_BUILD_TARGET, 'Subject target mismatch');
    if (requireSecurity) {
        requireThat(acceptance.securityQualified === true, 'Builder provenance/source review is not independent artifact security qualification; reference execution refused');
        requireThat(beforeExpiry(acceptance.expiresAt) && acceptance.expiresAt === '2026-12-12', 'Missing, invalid or expired Builder security approval');
        const review = maintenanceInputs().contract.sourceReview;
        requireThat(review.status === 'accepted' && review.targets.includes(receipt.subject.target) && beforeExpiry(review.expiresAt) && review.expiresAt === '2026-12-12', 'Current target-specific source review required');
        requireThat(/^[a-f0-9]{64}$/.test(review.constructionBasisSha256) && acceptance.constructionBasisSha256 === review.constructionBasisSha256 && receipt.inputs.constructionBasisSha256 === acceptance.constructionBasisSha256, 'Artifact/source reviewed construction basis mismatch');
        assert.deepEqual(receipt.inputs.callerClosure, acceptance.callerClosure, 'Artifact caller closure differs from producer receipt');
        assert.deepEqual(callerClosure(), acceptance.callerClosure, 'Canonical caller/helper closure drift; a reviewed receipt for this caller is required');
        requireThat(receipt.inputs.tools?.node?.path === fs.realpathSync(process.execPath) && receipt.inputs.tools.node.sha256 === hash(process.execPath), 'Verifier Node differs from reviewed producer helper runtime');
    }
}

function checkOutputPath(file) {
    for (let current = path.resolve(file); ; current = path.dirname(current)) {
        try {
            const stat = fs.lstatSync(current);
            requireThat(current === path.resolve(file) ? stat.isFile() : stat.isDirectory(), `Unsafe output path: ${current}`);
        } catch (error) { if (error.code !== 'ENOENT') throw error; }
        if (current === path.dirname(current)) break;
    }
}

function checkRawSpec(file, wasm) {
    const spec = json(file);
    const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
    const hex = value => typeof value === 'string' && /^0x(?:[0-9a-fA-F]{2})*$/.test(value);
    requireThat(object(spec) && typeof spec.name === 'string' && spec.name.length > 0 && typeof spec.id === 'string' && spec.id.length > 0, 'Invalid ChainSpec root/name/id');
    requireThat(['Development', 'Local', 'Live'].includes(spec.chainType) && Array.isArray(spec.bootNodes) && spec.bootNodes.every(node => typeof node === 'string'), 'Invalid ChainSpec type/bootNodes');
    requireThat(spec.properties === null || object(spec.properties), 'Invalid ChainSpec properties');
    for (const key of ['protocolId', 'forkId']) requireThat(spec[key] === undefined || spec[key] === null || typeof spec[key] === 'string', `Invalid ChainSpec ${key}`);
    requireThat(spec.telemetryEndpoints === undefined || spec.telemetryEndpoints === null || (Array.isArray(spec.telemetryEndpoints) && spec.telemetryEndpoints.every(endpoint => Array.isArray(endpoint) && endpoint.length === 2 && typeof endpoint[0] === 'string' && Number.isInteger(endpoint[1]) && endpoint[1] >= 0 && endpoint[1] <= 255)), 'Invalid telemetry endpoints');
    requireThat(spec.codeSubstitutes === undefined || (object(spec.codeSubstitutes) && Object.values(spec.codeSubstitutes).every(hex)), 'Invalid code substitutes');
    requireThat(spec.relay_chain === undefined || typeof spec.relay_chain === 'string', 'Invalid relay chain');
    requireThat(spec.para_id === undefined || (Number.isInteger(spec.para_id) && spec.para_id >= 0 && spec.para_id <= 0xffffffff), 'Invalid para id');
    requireThat(object(spec.genesis), 'Missing raw genesis');
    exactKeys(spec.genesis, ['raw'], 'raw genesis');
    const raw = spec.genesis.raw;
    requireThat(object(raw), 'Invalid raw genesis');
    exactKeys(raw, ['top', 'childrenDefault'], 'raw storage');
    const storage = map => requireThat(object(map) && Object.entries(map).every(([key, value]) => hex(key) && hex(value)), 'Invalid raw storage hex map');
    storage(raw.top);
    requireThat(object(raw.childrenDefault), 'Invalid raw children');
    for (const [key, child] of Object.entries(raw.childrenDefault)) { requireThat(hex(key), 'Invalid child storage key'); storage(child); }
    const code = raw.top['0x3a636f6465'];
    requireThat(hex(code) && code.length > 2, 'Missing runtime :code');
    requireThat(Buffer.from(code.slice(2), 'hex').equals(readRegular(wasm)), 'Raw genesis :code differs from selected runtime Wasm');
}

function privateProject(directory) {
    const root = path.join(directory, 'project');
    const input = path.join(root, 'scripts/inputs/chain-spec-builder');
    fs.mkdirSync(input, { recursive: true });
    for (const file of Object.keys(callerClosure())) {
        fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
        fs.copyFileSync(path.resolve(owner, '../../..', file), path.join(root, file));
    }
    for (const file of ['maintenance.json', 'maintenance.lock', 'accepted.json']) fs.copyFileSync(path.join(owner, file), path.join(input, file));
    for (const patch of maintenanceInputs().patches) {
        const target = path.join(root, patch.directory);
        fs.mkdirSync(path.dirname(target), { recursive: true });
        fs.cpSync(patch.path, target, { recursive: true, filter: file => !['target', '.git'].some(name => file === path.join(patch.path, name)) });
    }
    return { root, input, helper: path.join(input, 'provenance.mjs') };
}

function runFixture(helper, args, pass = true) {
    const result = spawnSync(process.execPath, [helper, ...args], { encoding: 'utf8', timeout: 120000, maxBuffer: 1024 * 1024 });
    requireThat(!result.error, `Fixture process failed: ${result.error}`);
    if (pass) assert.equal(result.status, 0, result.stderr);
    else assert.notEqual(result.status, 0, 'Expected public guard refusal');
    return result;
}

function receiptBoundaryTests(dir) {
    const project = privateProject(path.join(dir, 'receipt-fixture'));
    const contractFile = path.join(project.input, 'maintenance.json');
    const contract = json(contractFile);
    contract.sourceReview = { ...contract.sourceReview, status: 'accepted', targets: [expected.targets[0]], constructionBasisSha256: 'b'.repeat(64) };
    fs.writeFileSync(contractFile, JSON.stringify(contract));
    const sourceFile = path.join(project.input, 'source.json');
    const closure = Object.fromEntries(Object.keys(callerClosure()).map(file => [file, hash(path.join(project.root, file))]));
    const binary = path.join(project.root, 'stub');
    const wasm = path.join(project.root, 'runtime.wasm');
    fs.writeFileSync(wasm, 'fixture');
    fs.writeFileSync(binary, '#!/usr/bin/env bash\nprintf invoked >"${0}.invoked"\nprintf \'%s\\n\' \'{"name":"Fixture","id":"fixture","chainType":"Development","bootNodes":[],"properties":{},"genesis":{"raw":{"top":{"0x3a636f6465":"0x66697874757265"},"childrenDefault":{}}}}\' >chain_spec.json\n');
    fs.chmodSync(binary, 0o700);
    const receipt = { schema: 1, role: expected.role, complete: true, exit: 0, inputs: {
        version: expected.version, archiveSha256: expected.archiveSha256, lockSha256: contract.lockSha256,
        expectedSha256: hash(sourceFile), compiler: `release: ${expected.toolchain}\ncommit-hash: ${expected.rustcCommit}\n`,
        environment: { ...expected.profileEnvironment, RUSTFLAGS: expected.rustflags, CARGO_BUILD_TARGET: expected.targets[0] },
        constructionBasisSha256: contract.sourceReview.constructionBasisSha256, callerClosure: closure,
        tools: { node: { path: fs.realpathSync(process.execPath), sha256: hash(process.execPath) } },
        maintenance: { contract, contractSha256: hash(contractFile), patches: contract.patches.map(patch => ({ ...patch, path: path.join(project.root, patch.directory) })) }
    }, subject: { sha256: hash(binary), target: expected.targets[0] } };
    const receiptFile = `${binary}.producer.json`;
    fs.writeFileSync(receiptFile, JSON.stringify(receipt));
    const accepted = { schema: 1, role: expected.role, receiptSha256: hash(receiptFile), subjectSha256: hash(binary), securityQualified: true, expiresAt: '2026-12-12', constructionBasisSha256: receipt.inputs.constructionBasisSha256, callerClosure: closure };
    fs.writeFileSync(path.join(project.input, 'accepted.json'), JSON.stringify(accepted));
    const caller = path.join(project.root, 'scripts/04-generate-chain-spec.sh');
    const verifyPoint = (point, pass) => {
        const code = 'source "$1"; SELECTED_BUILDER="$2"; RUNTIME_WASM_PATH="$3"; CHAIN_SPEC_PATH="$4"; resolve_chain_profile; if [[ "$5" == pre ]]; then verify_builder_producer; else generate_chain_spec; fi';
        const result = spawnSync('bash', ['-c', code, '_', caller, binary, wasm, path.join(project.root, 'output.json'), point], { encoding: 'utf8', timeout: 10000, env: { ...process.env, PATH: `${path.dirname(process.execPath)}:${process.env.PATH}` } });
        requireThat(!result.error, `Caller fixture failed: ${result.error}`);
        if (pass) assert.equal(result.status, 0, result.stderr);
        else { assert.notEqual(result.status, 0); assert.equal(fs.existsSync(`${binary}.invoked`), false); }
    };
    verifyPoint('pre', true);
    verifyPoint('adjacent', true);
    fs.unlinkSync(`${binary}.invoked`);
    const immutableReceipt = hash(receiptFile);
    for (const file of [caller, project.helper, path.join(project.root, 'scripts/_common.sh')]) {
        const original = readRegular(file, 'utf8');
        fs.writeFileSync(file, file === caller ? original.replace('--verify \\\n', '--no-verify \\\n') : `${original}\n#DRIFT\n`.replace('#DRIFT', file.endsWith('.mjs') ? '// helper drift' : '# helper drift'));
        requireThat(hash(file) !== digest(original), 'Missing actual caller/helper mutation');
        verifyPoint('pre', false);
        verifyPoint('adjacent', false);
        assert.equal(hash(receiptFile), immutableReceipt);
        assert.equal(hash(binary), accepted.subjectSha256);
        fs.writeFileSync(file, original);
    }
    runFixture(project.helper, ['verify', receiptFile, binary]);
    console.log('Private approved fixtures: both public caller verification points pass baseline and refuse actual script04 invocation/common/MJS drift with unchanged receipt/subject; stub not invoked');
}

function sourceBoundaryTests(directory, preparedWork) {
    const project = privateProject(path.join(directory, 'source-boundary'));
    const work = path.join(directory, 'source-boundary', 'attempt');
    fs.mkdirSync(work);
    const sourceName = `${expected.package}-${expected.version}`;
    const source = path.join(work, sourceName);
    const oldSource = path.join(preparedWork, sourceName);
    fs.cpSync(oldSource, source, { recursive: true });
    for (const name of ['Cargo.toml', 'Cargo.lock']) fs.copyFileSync(path.join(preparedWork, `original-${name}`), path.join(source, name));
    for (const name of ['source.crate', 'tools.json', 'rustc.txt', 'cfg.txt', 'cargo.txt', 'host.txt']) fs.copyFileSync(path.join(preparedWork, name), path.join(work, name));
    fs.copyFileSync(path.join(project.root, 'scripts/_common.sh'), path.join(work, 'common-loaded.sh'));
    const environment = json(path.join(preparedWork, 'environment.json'));
    environment.HOME = path.join(work, 'home');
    environment.CARGO_TARGET_DIR = path.join(work, 'target');
    save(path.join(work, 'environment.json'), environment);
    const metadata = json(path.join(preparedWork, 'metadata.json'));
    const paths = new Map([[oldSource, source], ...maintenanceInputs().patches.map(patch => [patch.path, path.join(project.root, patch.directory)])]);
    const ids = new Map();
    for (const pkg of metadata.packages) {
        const replacement = paths.get(path.dirname(pkg.manifest_path));
        if (!replacement) continue;
        const original = path.dirname(pkg.manifest_path);
        const newId = pkg.id.replace(`path+file://${original}#`, `path+file://${replacement}#`);
        ids.set(pkg.id, newId);
        pkg.id = newId;
        pkg.manifest_path = path.join(replacement, 'Cargo.toml');
        for (const target of pkg.targets) target.src_path = path.join(replacement, path.relative(original, target.src_path));
    }
    const id = value => ids.get(value) ?? value;
    metadata.workspace_root = source;
    metadata.target_directory = metadata.build_directory = path.join(work, 'target');
    metadata.workspace_members = metadata.workspace_members.map(id);
    metadata.workspace_default_members = metadata.workspace_default_members.map(id);
    metadata.resolve.root = id(metadata.resolve.root);
    for (const node of metadata.resolve.nodes) {
        node.id = id(node.id);
        node.dependencies = node.dependencies.map(id);
        for (const dep of node.deps) dep.pkg = id(dep.pkg);
    }
    save(path.join(work, 'metadata.json'), metadata);
    const features = readRegular(path.join(preparedWork, 'features.txt'), 'utf8').split('\n').map(line => {
        for (const [from, to] of paths) if (line.endsWith(` (${from})`)) return `${line.slice(0, -from.length - 1)}${to})`;
        return line;
    });
    fs.writeFileSync(path.join(work, 'features.txt'), features.join('\n'));
    runFixture(project.helper, ['source-prepare', work]);
    const beforeFile = path.join(work, 'before.json');
    const capture = () => {
        fs.rmSync(beforeFile, { force: true });
        runFixture(project.helper, ['before', work]);
        return json(beforeFile);
    };
    const unreviewed = capture();
    runFixture(project.helper, ['build-admission', work], false);
    const contractFile = path.join(project.input, 'maintenance.json');
    const contract = json(contractFile);
    contract.sourceReview = { ...contract.sourceReview, status: 'accepted', targets: [environment.CARGO_BUILD_TARGET], constructionBasisSha256: unreviewed.constructionBasisSha256 };
    fs.writeFileSync(contractFile, JSON.stringify(contract));
    const maintenanceFile = path.join(work, 'maintenance-inputs.json');
    const updateMaintenance = value => fs.writeFileSync(maintenanceFile, JSON.stringify({ contract: value, contractSha256: hash(contractFile), patches: value.patches.map(patch => ({ ...patch, path: path.join(project.root, patch.directory) })) }));
    updateMaintenance(contract);
    // Approval alone preserves the construction digest, but never the raw attempt.
    runFixture(project.helper, ['build-admission', work], false);
    const approved = capture();
    assert.equal(approved.constructionBasisSha256, unreviewed.constructionBasisSha256);
    assert.notEqual(approved.maintenance.contractSha256, unreviewed.maintenance.contractSha256);
    runFixture(project.helper, ['build-admission', work]);
    const relocated = `${work}-relocated`;
    fs.cpSync(work, relocated, { recursive: true });
    const relocatedSource = path.join(relocated, sourceName);
    const moved = structuredClone(metadata);
    const oldId = `path+file://${source}#${expected.package}@${expected.version}`;
    const newId = `path+file://${relocatedSource}#${expected.package}@${expected.version}`;
    const moveId = value => value === oldId ? newId : value;
    const rootPackage = moved.packages.find(pkg => pkg.id === oldId);
    rootPackage.id = newId;
    rootPackage.manifest_path = path.join(relocatedSource, 'Cargo.toml');
    for (const target of rootPackage.targets) target.src_path = path.join(relocatedSource, path.relative(source, target.src_path));
    moved.workspace_root = relocatedSource;
    moved.target_directory = moved.build_directory = path.join(relocated, 'target');
    moved.workspace_members = moved.workspace_members.map(moveId);
    moved.workspace_default_members = moved.workspace_default_members.map(moveId);
    moved.resolve.root = moveId(moved.resolve.root);
    for (const node of moved.resolve.nodes) {
        node.id = moveId(node.id);
        node.dependencies = node.dependencies.map(moveId);
        for (const dep of node.deps) dep.pkg = moveId(dep.pkg);
    }
    fs.writeFileSync(path.join(relocated, 'metadata.json'), JSON.stringify(moved));
    fs.writeFileSync(path.join(relocated, 'environment.json'), JSON.stringify({ ...environment, HOME: path.join(relocated, 'home'), CARGO_TARGET_DIR: path.join(relocated, 'target') }));
    const movedFeatures = readRegular(path.join(work, 'features.txt'), 'utf8').split('\n');
    movedFeatures[0] = `${expected.package} v${expected.version} (${relocatedSource})`;
    fs.writeFileSync(path.join(relocated, 'features.txt'), movedFeatures.join('\n'));
    fs.unlinkSync(path.join(relocated, 'before.json'));
    runFixture(project.helper, ['before', relocated]);
    assert.equal(json(path.join(relocated, 'before.json')).constructionBasisSha256, approved.constructionBasisSha256);
    runFixture(project.helper, ['build-admission', relocated]);
    moved.metadata = { arbitraryPayloadMustNotRelocate: relocated };
    fs.writeFileSync(path.join(relocated, 'metadata.json'), JSON.stringify(moved));
    fs.unlinkSync(path.join(relocated, 'before.json'));
    runFixture(project.helper, ['before', relocated]);
    runFixture(project.helper, ['build-admission', relocated], false);
    for (const file of [path.join(project.root, 'scripts/04-generate-chain-spec.sh'), project.helper, path.join(work, 'cfg.txt'), path.join(work, 'features.txt')]) {
        const original = readRegular(file, 'utf8');
        fs.appendFileSync(file, file.endsWith('.sh') ? '\n# caller drift\n' : '\n// source basis drift\n');
        const changed = capture();
        assert.notEqual(changed.constructionBasisSha256, approved.constructionBasisSha256);
        runFixture(project.helper, ['build-admission', work], false);
        fs.writeFileSync(file, original);
    }
    const lockFile = path.join(project.input, 'maintenance.lock');
    const originalLock = readRegular(lockFile);
    const originalContract = readRegular(contractFile);
    fs.appendFileSync(lockFile, '\n');
    contract.lockSha256 = hash(lockFile);
    fs.writeFileSync(contractFile, JSON.stringify(contract));
    fs.copyFileSync(lockFile, path.join(source, 'Cargo.lock'));
    updateMaintenance(contract);
    assert.notEqual(capture().constructionBasisSha256, approved.constructionBasisSha256);
    runFixture(project.helper, ['build-admission', work], false);
    fs.writeFileSync(lockFile, originalLock);
    fs.writeFileSync(contractFile, originalContract);
    fs.copyFileSync(lockFile, path.join(source, 'Cargo.lock'));
    updateMaintenance(json(contractFile));
    const patchedContract = json(contractFile);
    const patch = patchedContract.patches[0];
    const patchRoot = path.join(project.root, patch.directory);
    const patchFile = path.join(patchRoot, 'src/lib.rs');
    const originalPatch = readRegular(patchFile);
    fs.appendFileSync(patchFile, '\n// replaced declared source package; same Cargo graph\n');
    patch.sha256 = tree(patchRoot);
    fs.writeFileSync(contractFile, JSON.stringify(patchedContract));
    updateMaintenance(patchedContract);
    assert.notEqual(capture().constructionBasisSha256, approved.constructionBasisSha256);
    runFixture(project.helper, ['build-admission', work], false);
    fs.writeFileSync(patchFile, originalPatch);
    fs.writeFileSync(contractFile, originalContract);
    updateMaintenance(json(contractFile));
    for (const name of ['src/lib.rs', 'build.rs', 'Cargo.toml']) {
        const file = path.join(source, name);
        const original = readRegular(file);
        fs.appendFileSync(file, '\n// replaced source with same Cargo graph\n');
        runFixture(project.helper, ['build-admission', work], false);
        fs.writeFileSync(file, original);
    }
    const extra = path.join(source, 'untracked-generator.rs');
    fs.writeFileSync(extra, 'new generator');
    runFixture(project.helper, ['build-admission', work], false);
    fs.unlinkSync(extra);
    const metadataFile = path.join(work, 'metadata.json');
    const originalMetadata = readRegular(metadataFile);
    metadata.packages.push({ id: 'unreviewed-local', source: null, manifest_path: path.join(work, 'unknown/Cargo.toml') });
    fs.writeFileSync(metadataFile, JSON.stringify(metadata));
    runFixture(project.helper, ['build-admission', work], false);
    fs.writeFileSync(metadataFile, originalMetadata);
    const originalMaintenance = readRegular(maintenanceFile);
    const changedMaintenance = json(maintenanceFile);
    changedMaintenance.patches[0].sha256 = 'unreviewed';
    fs.writeFileSync(maintenanceFile, JSON.stringify(changedMaintenance));
    runFixture(project.helper, ['build-admission', work], false);
    fs.writeFileSync(maintenanceFile, originalMaintenance);
    const config = path.join(work, '.cargo');
    fs.mkdirSync(config);
    fs.writeFileSync(path.join(config, 'config.toml'), '[build]\nrustflags = ["--cfg", "unreviewed"]\n');
    runFixture(project.helper, ['build-admission', work], false);
    fs.rmSync(config, { recursive: true });
    assert.equal(capture().constructionBasisSha256, approved.constructionBasisSha256);
    runFixture(project.helper, ['build-admission', work]);
    console.log('Actual-source public admission: approval-only construction equivalence/raw attempt drift; exact-schema relocation baseline; fresh caller/helper/cfg/features/lock+declared-SHA drift, declared local-package replacement with same graph, source/build/manifest replacement, untracked generator, unknown package and configuration refusals before Cargo');
}

function selfTest(preparedWork) {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'deos-builder-proof-'));
    try {
        const binary = path.join(dir, 'builder');
        fs.writeFileSync(binary, 'fixture, never executed');
        const receipt = { schema: 1, role: expected.role, exit: 0, complete: true, _sha256: 'fixture', inputs: {
            version: expected.version, archiveSha256: expected.archiveSha256, lockSha256: expected.lockSha256,
            expectedSha256: hash(sourcePath), compiler: `commit-hash: ${expected.rustcCommit}\nrelease: ${expected.toolchain}\n`,
            environment: { ...expected.profileEnvironment, RUSTFLAGS: '', CARGO_BUILD_TARGET: expected.targets[0] }
        }, subject: { sha256: hash(binary), target: expected.targets[0] } };
        const acceptance = { schema: 1, role: expected.role, receiptSha256: 'fixture', subjectSha256: hash(binary), securityQualified: false };
        checkReceipt(receipt, binary, acceptance, false);
        const cases = [
            r => { r.role = 'relay'; }, r => { r.inputs.version = '21.0.0'; },
            r => { r.inputs.archiveSha256 = 'wrong'; }, r => { r.inputs.lockSha256 = 'wrong'; },
            r => { r.inputs.compiler = 'rustc 1.98.0'; }, r => { r.inputs.environment.RUSTFLAGS = '-C opt-level=0'; },
            r => { r.inputs.environment.CARGO_PROFILE_RELEASE_LTO = 'fat'; },
            r => { r.subject.sha256 = 'wrong'; }, r => { r.complete = false; }, r => { r.inputs.environment.CARGO_BUILD_TARGET = 'unknown'; }
        ];
        for (const mutate of cases) {
            const changed = structuredClone(receipt); mutate(changed);
            assert.throws(() => checkReceipt(changed, binary, acceptance, false));
        }
        assert.throws(() => checkReceipt(receipt, binary, { ...acceptance, receiptSha256: 'arbitrary' }, false));
        assert.throws(() => checkReceipt(receipt, binary, acceptance));
        assert.equal(beforeExpiry('2026-12-12', Date.parse('2026-12-11T23:59:59Z')), true);
        for (const expiresAt of [undefined, 'invalid', '2026-02-31', '2026-12-12']) assert.equal(beforeExpiry(expiresAt, Date.parse('2026-12-12T00:00:00Z')), false);
        const maintenance = maintenanceInputs();
        assert.ok(['unreviewed', 'accepted'].includes(maintenance.contract.sourceReview.status));
        assert.match(maintainedManifest('[package]\nname = "fixture"\n', maintenance.patches), /\[patch\.crates-io\]/);
        assert.throws(() => maintainedManifest('[patch.crates-io]\n', maintenance.patches));
        const maintained = structuredClone(receipt);
        maintained.inputs.maintenance = maintenance;
        maintained.inputs.lockSha256 = maintenance.contract.lockSha256;
        checkReceipt(maintained, binary, acceptance, false);
        maintained.inputs.maintenance.patches[0].sha256 = 'changed';
        assert.throws(() => checkReceipt(maintained, binary, acceptance, false));
        assert.throws(() => json(path.join(dir, 'missing')));
        fs.writeFileSync(path.join(dir, 'bad.json'), '{');
        assert.throws(() => json(path.join(dir, 'bad.json')));
        const wasm = path.join(dir, 'runtime.wasm');
        const specFile = path.join(dir, 'raw.json');
        fs.writeFileSync(wasm, 'arbitrary selected runtime');
        const validSpec = { name: 'Fixture', id: 'fixture', chainType: 'Development', bootNodes: [], properties: {}, genesis: { raw: { top: { '0x3a636f6465': `0x${readRegular(wasm).toString('hex')}`, '0x00': '0x' }, childrenDefault: { '0x01': { '0x02': '0x03' } } } } };
        fs.writeFileSync(specFile, JSON.stringify(validSpec));
        checkRawSpec(specFile, wasm);
        for (const mutate of [
            spec => { delete spec.name; }, spec => { spec.genesis = { runtime: {} }; },
            spec => { spec.genesis.raw.top = {}; }, spec => { spec.genesis.raw.top['0x00'] = 1; },
            spec => { spec.genesis.raw.top['0xz'] = '0x00'; }, spec => { spec.genesis.raw.top['0x00'] = '0x1'; },
            spec => { delete spec.genesis.raw.childrenDefault; }, spec => { spec.genesis.raw.childrenDefault = []; },
            spec => { spec.genesis.raw.childrenDefault['0x01'] = { '0x02': null }; },
            spec => { spec.genesis.raw.top['0x3a636f6465'] = '0x'; },
            spec => { spec.genesis.raw.top['0x3a636f6465'] = '0x00'; }
        ]) {
            const changed = structuredClone(validSpec); mutate(changed);
            fs.writeFileSync(specFile, JSON.stringify(changed));
            assert.throws(() => checkRawSpec(specFile, wasm));
        }
        const symlink = path.join(dir, 'symlink');
        fs.symlinkSync(wasm, symlink);
        const fifo = path.join(dir, 'fifo');
        assert.equal(spawnSync('mkfifo', [fifo]).status, 0);
        for (const unsafe of [dir, symlink, fifo]) {
            assert.throws(() => hash(unsafe));
            assert.throws(() => json(unsafe));
            assert.throws(() => checkOutputPath(unsafe));
        }
        receiptBoundaryTests(dir);
        if (preparedWork) {
            sourceBoundaryTests(dir, preparedWork);
        }
        console.log('Builder proof: positive provenance fixture; source/version/compiler/profile/flags/role/target/digest/receipt negatives; security refusal passed (no product execution)');
    } finally { fs.rmSync(dir, { recursive: true, force: true }); }
}

const [action, work, binary] = process.argv.slice(2);
try {
    if (action === 'self-test') selfTest(work);
    else if (action === 'check-raw-spec') checkRawSpec(work, binary);
    else if (action === 'check-output-path') checkOutputPath(work);
    else if (action === 'environment') {
        const environment = Object.fromEntries(Object.entries(process.env).filter(([key]) => !['_', 'SHLVL', 'PWD'].includes(key)).sort(([a], [b]) => a.localeCompare(b)));
        save(path.join(work, 'environment.json'), environment);
    } else if (action === 'tools') {
        const tools = {};
        for (const file of process.argv.slice(4)) tools[path.basename(file)] = { invocationPath: file, path: fs.realpathSync(file), sha256: hash(fs.realpathSync(file)) };
        save(path.join(work, 'tools.json'), tools);
    } else if (action === 'source-prepare') prepareSource(work);
    else if (action === 'source-review') {
        const review = maintenanceInputs().contract.sourceReview;
        requireThat(review.status === 'accepted' && expected.targets.includes(work) && review.targets.includes(work), 'Independent target-specific Builder19 composed-source review is required BEFORE compilation; use --builder-source-only');
        requireThat(beforeExpiry(review.expiresAt) && review.expiresAt === '2026-12-12' && /^[a-f0-9]{64}$/.test(review.constructionBasisSha256), 'Missing, invalid or expired Builder source approval/basis');
    } else if (action === 'build-admission') {
        const current = snapshot(work);
        assert.deepEqual(current, json(path.join(work, 'before.json')), 'Pre-build attempt drift');
        sourceAdmission(work, current);
    } else if (action === 'source-finish') {
        const after = snapshot(work);
        assert.deepEqual(after, json(path.join(work, 'before.json')), 'Source preparation inputs changed');
        save(path.join(work, 'after.json'), after);
        save(path.join(work, 'construction-basis.json'), constructionBasis(work, after));
        save(path.join(work, 'source-prepared.json'), { schema: 1, role: expected.role, purpose: 'source-only', complete: true, securityQualified: false, inputs: after, beforeSha256: hash(path.join(work, 'before.json')), afterSha256: hash(path.join(work, 'after.json')) });
        console.log('Builder19 maintained source prepared and sealed; no compilation, artifact approval or execution');
    } else if (action === 'before') save(path.join(work, 'before.json'), snapshot(work));
    else if (action === 'finish') {
        const exit = Number(process.argv[5]);
        let after, error;
        try { after = snapshot(work); assert.deepEqual(after, json(path.join(work, 'before.json'))); } catch (e) { error = e.message; }
        if (after) save(path.join(work, 'after.json'), after);
        const messages = readRegular(path.join(work, 'build.jsonl'), 'utf8').trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
        const finished = messages.filter(message => message.reason === 'build-finished');
        if (exit === 0 && (finished.length !== 1 || finished[0].success !== true)) error = 'Missing successful Cargo build-finished message';
        const receipt = { schema: 1, role: expected.role, exit, complete: exit === 0 && !error,
            inputs: json(path.join(work, 'before.json')), error: error ?? null,
            artifacts: messages.filter(message => message.reason === 'compiler-artifact').map(message => ({ packageId: message.package_id, target: message.target, features: message.features, profile: message.profile, fresh: message.fresh, executable: message.executable })),
            buildScripts: messages.filter(message => message.reason === 'build-script-executed'),
            buildLogSha256: hash(path.join(work, 'build.log')), buildMessagesSha256: hash(path.join(work, 'build.jsonl')),
            subject: exit === 0 && fs.existsSync(binary) ? { sha256: hash(binary), size: fs.statSync(binary).size, target: after?.environment.CARGO_BUILD_TARGET } : null };
        save(path.join(work, 'receipt.json'), receipt);
        requireThat(receipt.complete, `Producer did not complete: exit=${exit}, ${error ?? 'see build.log'}`);
    } else if (action === 'verify' || action === 'verify-producer') {
        const receipt = json(work); receipt._sha256 = hash(work);
        checkReceipt(receipt, binary, json(path.join(owner, 'accepted.json')), action === 'verify');
        console.log(action === 'verify' ? 'Qualified builder source, producer and subject verified' : 'Known local producer identity verified; SECURITY UNQUALIFIED, no reference execution authorized');
    } else throw new Error(`Unknown builder provenance action: ${action}`);
} catch (error) { console.error(`Builder provenance: ${error.message}`); process.exitCode = 1; }
