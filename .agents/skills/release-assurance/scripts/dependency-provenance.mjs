#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { dirname, join, relative, resolve } from 'node:path';

// Source exclusions require independently reviewed advisory prerequisites, not just
// membership in a native graph. The ledger/reference retain their exact limits.
const reachabilityClasses = new Set([
  'lockfile-only',
  'native-observability-only',
  'trusted-generation-only',
  'source-prerequisite-excluded',
  'source-backport-reviewed',
]);

if (process.argv[2] === '--self-test') {
  assert.equal(process.argv.length, 3);
  const directory = await mkdtemp(join(tmpdir(), 'deos-dependency-self-test.'));
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
    expect(cli(['one', 'two', 'three', 'four']), 2, /Usage/);
    assert.equal(await readFile(actualLock, 'utf8'), lock);
    console.log('[SUCCESS] Dependency CLI self-test: ' + positives + ' positive, ' + negatives +
      ' negative cases; isolated projection, union, DB, source bindings and fail-closed contracts');
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
  return JSON.parse(await readFile(path, 'utf8'));
}

// Only Cargo-generated lock syntax is supported. Reconstructing exactly the same
// bytes except inserted source lines proves names, versions and edges unchanged.
const registrySource = 'registry+https://github.com/rust-lang/crates.io-index';
function projectLock(lock, metadata) {
  assert.ok(Array.isArray(metadata.packages) && Array.isArray(metadata.workspace_members) &&
    Array.isArray(metadata.resolve?.nodes), 'full locked Cargo metadata is required');
  const members = new Set(metadata.workspace_members);
  const resolved = new Set(metadata.resolve.nodes.map(node => node.id));
  assert.ok(metadata.packages.every(pkg => resolved.has(pkg.id)), 'incomplete resolved Cargo metadata');
  const sourceNull = new Set(metadata.packages.filter(pkg => pkg.source === null)
    .map(pkg => `${pkg.name}@${pkg.version}`));
  const locals = metadata.packages.filter(pkg => pkg.source === null && !members.has(pkg.id));
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

if (process.argv[2] === '--project') {
  assert.equal(process.argv.length, 6, 'Usage: --project ACTUAL_LOCK FULL_METADATA_JSON TEMP_LOCK');
  const [, , , lockPath, metadataPath, output] = process.argv;
  assert.notEqual(resolve(lockPath), resolve(output), 'projection cannot overwrite actual lock');
  const { projected, locals } = projectLock(await readFile(lockPath, 'utf8'), await readJson(metadataPath));
  await writeFile(output, projected, { flag: 'wx' });
  console.log(`[INFO] Counterfactual version view: ${locals.length} local third-party packages; not source provenance`);
  process.exit(0);
}

const [cargoPath, projectedAuditPath, npmPath, exceptionPath, packageLockPath,
  lockPath, metadataPath, projectedLockPath, repoRoot] = process.argv.slice(2);
if (process.argv.length !== 11) {
  console.error('Usage: dependency-provenance.mjs ACTUAL_AUDIT PROJECTED_AUDIT NPM_AUDIT EXCEPTIONS PACKAGE_LOCK ACTUAL_LOCK FULL_METADATA PROJECTED_LOCK REPO_ROOT | --project ACTUAL_LOCK FULL_METADATA TEMP_LOCK | --self-test');
  process.exit(2);
}
const [cargo, projectedAudit, npm, ledger, packageLock, lock, metadata, projectedLock] = await Promise.all([
  readJson(cargoPath), readJson(projectedAuditPath), readJson(npmPath), readJson(exceptionPath),
  readJson(packageLockPath), readFile(lockPath, 'utf8'), readJson(metadataPath), readFile(projectedLockPath, 'utf8'),
]);
assert.notEqual(await realpath(cargoPath), await realpath(projectedAuditPath), 'both distinct audit files are required');
const { projected, locals } = projectLock(lock, metadata);
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
