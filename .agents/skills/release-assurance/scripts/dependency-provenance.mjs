#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// Source exclusions require independently reviewed advisory prerequisites, not just
// membership in a native graph. The ledger/reference retain their exact limits.
const reachabilityClasses = new Set([
  'lockfile-only',
  'native-observability-only',
  'trusted-generation-only',
  'source-prerequisite-excluded',
]);

if (process.argv[2] === '--self-test') {
  assert.equal(process.argv.length, 3);
  const directory = await mkdtemp(join(tmpdir(), 'deos-dependency-self-test.'));
  const date = new Date().toISOString().slice(0, 10);
  const advisory = 'RUSTSEC-SELFTEST-0001';
  const exception = {
    ecosystem: 'cargo', id: advisory, expires: date,
    reachability: 'source-prerequisite-excluded',
    reason: 'Fixture-only source prerequisite exclusion; no package safety assertion.',
  };
  const base = {
    cargo: { vulnerabilities: { list: [{ advisory: { id: advisory } }] },
      database: { 'last-commit': 'fixture', 'last-updated': date } },
    npm: { metadata: { vulnerabilities: { critical: 0 } } },
    ledger: { formatVersion: 1, reviewedAt: date, exceptions: [exception], licenseExceptions: [] },
    lock: { packages: {} },
  };
  const invoke = async (data) => {
    const paths = ['cargo', 'npm', 'ledger', 'lock'].map(name => join(directory, `${name}.json`));
    await Promise.all(paths.map((path, index) => writeFile(path,
      JSON.stringify(data[['cargo', 'npm', 'ledger', 'lock'][index]]))));
    return spawnSync(process.execPath, [process.argv[1], ...paths], { encoding: 'utf8' });
  };
  try {
    assert.equal((await invoke(base)).status, 0);
    const unreviewed = structuredClone(base);
    unreviewed.cargo.vulnerabilities.list.push({ advisory: { id: 'RUSTSEC-SELFTEST-0002' } });
    const refused = await invoke(unreviewed);
    assert.equal(refused.status, 1);
    assert.match(refused.stderr, /unreviewed material dependency finding/);
    const unknown = structuredClone(base);
    unknown.ledger.exceptions[0].reachability = 'native-only';
    const invalid = await invoke(unknown);
    assert.equal(invalid.status, 1);
    assert.match(invalid.stderr, /unknown reachability classification/);
    const critical = structuredClone(base);
    critical.npm.metadata.vulnerabilities.critical = 1;
    critical.npm.vulnerabilities = { fixture: { severity: 'critical' } };
    critical.ledger.exceptions.push({ ...exception, ecosystem: 'npm', id: 'fixture' });
    const blocked = await invoke(critical);
    assert.equal(blocked.status, 1);
    assert.match(blocked.stderr, /npm critical advisory remains present even if listed/);
    critical.npm.metadata.vulnerabilities.critical = 0;
    const inconsistent = await invoke(critical);
    assert.equal(inconsistent.status, 1);
    assert.match(inconsistent.stderr, /npm critical advisory remains present even if listed/);
    console.log('[SUCCESS] Dependency source-exclusion, unknown, unreviewed and critical refusal self-test passed');
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

const [cargoPath, npmPath, exceptionPath, packageLockPath] = process.argv.slice(2);
if (!cargoPath || !npmPath || !exceptionPath || !packageLockPath) {
  console.error(
    'Usage: dependency-provenance.mjs CARGO_AUDIT_JSON NPM_AUDIT_JSON EXCEPTIONS_JSON PACKAGE_LOCK | --self-test',
  );
  process.exit(2);
}

const [cargo, npm, ledger, packageLock] = await Promise.all([
  readJson(cargoPath),
  readJson(npmPath),
  readJson(exceptionPath),
  readJson(packageLockPath),
]);

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
for (const finding of cargo.vulnerabilities?.list ?? []) {
  findings.add(`cargo:${finding.advisory.id}`);
}
for (const finding of cargo.warnings?.unsound ?? []) {
  findings.add(`cargo:${finding.advisory.id}`);
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

if (!cargo.database?.['last-commit'] || !cargo.database?.['last-updated']) {
  fail('cargo advisory database identity is missing');
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
