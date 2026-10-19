import assert from 'node:assert/strict';
import { isAbsolute } from 'node:path';

const U128 = (1n << 128n) - 1n;
const U32 = (1n << 32n) - 1n;
const DECLARATION = 259498000n;
const COUNTERS = ['reads', 'repeat_reads', 'writes', 'repeat_writes', 'proof_size'];
const ROW_FIELDS = ['components', 'extrinsic_time', 'storage_root_time', ...COUNTERS];

function requireThat(condition, message) {
  if (!condition) throw new Error(message);
}

/** Parse all JSON number lexemes as exact unsigned u128 BigInts. */
export function parseUnsignedJson(text) {
  requireThat(typeof text === 'string', 'JSON input must be text');
  let supported = false;
  JSON.parse('0', (_key, value, context) => {
    supported = context?.source === '0';
    return value;
  });
  requireThat(supported, 'JSON numeric source support is required (Node 22.22+)');
  return JSON.parse(text, (_key, value, context) => {
    if (typeof value !== 'number') return value;
    const source = context?.source;
    requireThat(typeof source === 'string' && /^(0|[1-9][0-9]*)$/.test(source),
      'JSON number must be an unsigned decimal integer without exponent');
    requireThat(source.length <= 39, 'JSON integer exceeds u128 digit limit');
    const exact = BigInt(source);
    requireThat(exact <= U128, 'JSON integer exceeds u128');
    return exact;
  });
}

function exactFields(value, fields, label) {
  requireThat(value !== null && typeof value === 'object' && !Array.isArray(value), `${label} must be an object`);
  const keys = Object.keys(value);
  requireThat(keys.length === fields.length && fields.every(key => Object.hasOwn(value, key)), `${label} has missing or extra fields`);
}

function unsigned(value, maximum, label) {
  requireThat(typeof value === 'bigint' && value >= 0n && value <= maximum, `${label} must be an unsigned integer <= ${maximum}`);
}

function validateRows(rows, count, label) {
  requireThat(Array.isArray(rows) && rows.length === count, `${label} must contain exactly ${count} rows`);
  rows.forEach((row, index) => {
    const at = `${label}[${index}]`;
    exactFields(row, ROW_FIELDS, at);
    const c = row.components;
    requireThat(Array.isArray(c) && c.length === 1 && Array.isArray(c[0]) && c[0].length === 2
      && c[0][0] === 'c' && c[0][1] === 0n, `${at}.components must be exactly [["c",0]]`);
    for (const field of ['extrinsic_time', 'storage_root_time']) unsigned(row[field], U128, `${at}.${field}`);
    for (const field of COUNTERS) unsigned(row[field], U32, `${at}.${field}`);
    if (label === 'time_results') requireThat(row.extrinsic_time > 0n, `${at} must have positive extrinsic_time`);
    else requireThat(row.reads > 0n && row.writes > 0n, `${at} must have positive reads and writes`);
  });
}

/**
 * Returns { data, observations: { declaration, database, samples, exceedanceCount } }.
 * data preserves every parsed row (numbers are BigInts). database preserves the
 * agreed counters/proof; each sample retains body/root times, counters and proof,
 * plus model and signed residual. All observation integers are BigInts.
 * This is only a conditional charged-DB comparison, not a bound or price.
 */
export function validateFrozenPrepassSamples(text) {
  const data = parseUnsignedJson(text);
  requireThat(Array.isArray(data) && data.length === 1, 'Expected exactly one benchmark batch');
  const batch = data[0];
  exactFields(batch, ['pallet', 'instance', 'benchmark', 'time_results', 'db_results'], 'batch');
  requireThat(batch.pallet === 'pallet_deos_actors' && batch.instance === 'Actors'
    && batch.benchmark === 'mandatory_prepass_control', 'Unexpected benchmark batch identity');
  validateRows(batch.time_results, 6, 'time_results');
  validateRows(batch.db_results, 2, 'db_results');
  const first = batch.db_results[0];
  requireThat(COUNTERS.every(field => first[field] === batch.db_results[1][field]),
    'DB rows disagree on counters/proof; model comparison is inconclusive');
  const database = Object.fromEntries(COUNTERS.map(field => [field, first[field]]));
  const samples = batch.time_results.map(row => {
    const model = 1000n * row.extrinsic_time + first.reads * 25000000n + first.writes * 100000000n;
    return { ...row, model, residual: model - DECLARATION };
  });
  const exceedanceCount = samples.reduce((count, sample) => count + (sample.residual > 0n ? 1n : 0n), 0n);
  return { data, observations: { declaration: DECLARATION, database, samples, exceedanceCount } };
}

/** Returns only the validated local guest dependency identity; performs no I/O. */
export function validateOrdinaryWbuildMetadata(text, { manifestPath, runtimePath }) {
  requireThat(typeof manifestPath === 'string' && isAbsolute(manifestPath)
    && typeof runtimePath === 'string' && isAbsolute(runtimePath), 'Expected absolute manifestPath and runtimePath');
  const metadata = parseUnsignedJson(text);
  requireThat(metadata !== null && typeof metadata === 'object' && metadata.version === 1n
    && Array.isArray(metadata.packages), 'Expected Cargo metadata format version 1 with packages');
  const roots = metadata.packages.filter(pkg => pkg?.manifest_path === manifestPath);
  requireThat(roots.length === 1, 'Expected exactly one matching guest root package');
  requireThat(Array.isArray(roots[0].dependencies), 'Guest root dependencies must be an array');
  const dependencies = roots[0].dependencies.filter(dep => dep?.name === 'deos-runtime' || dep?.rename === 'wasm-project');
  requireThat(dependencies.length === 1, 'Expected exactly one deos-runtime / wasm-project dependency');
  const dep = dependencies[0];
  requireThat(dep.name === 'deos-runtime' && dep.rename === 'wasm-project', 'Guest runtime dependency has wrong name or rename');
  requireThat(dep.source === null && dep.path === runtimePath, 'Guest runtime dependency must use the expected local runtime path');
  requireThat(dep.uses_default_features === false, 'Guest runtime default features must be disabled');
  requireThat(Array.isArray(dep.features) && dep.features.length === 2
    && dep.features.includes('hex-literal') && dep.features.includes('runtime-benchmarks'), 'Guest runtime features must be exactly hex-literal and runtime-benchmarks');
  return { manifestPath, runtimePath, name: dep.name, rename: dep.rename, source: null,
    usesDefaultFeatures: false, features: [...dep.features] };
}

/** Isolated in-memory regression fixtures; returns true or throws, without output. */
export function selfTest() {
  const encode = value => JSON.stringify(value, (_key, item) => typeof item === 'bigint' ? `INTEGER:${item}` : item)
    .replace(/"INTEGER:([0-9]+)"/g, '$1');
  const row = () => ({ components: [['c', 0n]], extrinsic_time: 1n, storage_root_time: 2n,
    reads: 0n, repeat_reads: 0n, writes: 0n, repeat_writes: 0n, proof_size: 0n });
  const fixture = () => [{ pallet: 'pallet_deos_actors', instance: 'Actors', benchmark: 'mandatory_prepass_control',
    time_results: Array.from({ length: 6 }, row),
    db_results: Array.from({ length: 2 }, () => ({ ...row(), reads: 1n, writes: 1n })) }];
  const validate = data => validateFrozenPrepassSamples(encode(data));
  const reject = change => { const data = fixture(); change(data, data[0]); assert.throws(() => validate(data)); };
  const valid = validate(fixture());
  assert.equal(valid.observations.exceedanceCount, 0n);
  assert.equal(valid.observations.samples.length, 6);
  assert.equal(valid.observations.samples[0].model, 125001000n);
  assert.equal(valid.observations.samples[0].residual, -134497000n);
  assert.equal(valid.observations.samples[0].storage_root_time, 2n);
  assert.equal(valid.observations.database.proof_size, 0n);
  assert.equal(parseUnsignedJson('9007199254740993'), 9007199254740993n);
  assert.equal(parseUnsignedJson(String(U128)), U128);
  assert.deepEqual(parseUnsignedJson('[true,false,"x",null]'), [true, false, 'x', null]);
  const large = fixture();
  large[0].time_results[0].extrinsic_time = U128;
  assert.equal(validate(large).observations.samples[0].model, U128 * 1000n + 125000000n);
  for (const text of ['-1', '-0', '1.0', '1e2', '01', '{', String(U128 + 1n), '1'.repeat(40)]) {
    assert.throws(() => parseUnsignedJson(text));
  }
  for (const field of ['pallet', 'instance', 'benchmark', 'time_results', 'db_results']) {
    reject((_data, batch) => { delete batch[field]; });
  }
  for (const field of ['pallet', 'instance', 'benchmark']) reject((_data, batch) => { batch[field] = 'wrong'; });
  reject(data => data.push(data[0]));
  reject(data => data.pop());
  assert.throws(() => validate({}));
  reject((_data, batch) => { batch.extra = true; });
  for (const kind of ['time_results', 'db_results']) {
    reject((_data, batch) => { batch[kind].pop(); });
    reject((_data, batch) => { batch[kind].push(row()); });
    reject((_data, batch) => { batch[kind] = {}; });
    reject((_data, batch) => { batch[kind][0] = null; });
    reject((_data, batch) => { batch[kind][0].extra = 0n; });
    for (const field of ROW_FIELDS) {
      reject((_data, batch) => { delete batch[kind][0][field]; });
      reject((_data, batch) => { batch[kind][0][field] = '0'; });
    }
    for (const components of [[], [['d', 0n]], [['c', 1n]], [['c', 0n, 0n]], [['c', 0n], ['c', 0n]], [[0n, 0n]]]) {
      reject((_data, batch) => { batch[kind][0].components = components; });
    }
    for (const field of COUNTERS) reject((_data, batch) => { batch[kind][0][field] = U32 + 1n; });
  }
  for (const field of COUNTERS) reject((_data, batch) => { batch.db_results[1][field] += 1n; });
  reject((_data, batch) => { batch.time_results[0].extrinsic_time = 0n; });
  for (const field of ['reads', 'writes']) reject((_data, batch) => { batch.db_results.forEach(item => { item[field] = 0n; }); });
  reject((_data, batch) => { [...batch.time_results, ...batch.db_results].forEach(item => { for (const field of ROW_FIELDS.slice(1)) item[field] = 0n; }); });
  const options = { manifestPath: '/target/wbuild/guest/Cargo.toml', runtimePath: '/source/runtime' };
  const guest = () => ({ version: 1n, packages: [{ name: 'arbitrary-guest', manifest_path: options.manifestPath,
    dependencies: [{ name: 'deos-runtime', rename: 'wasm-project', source: null, path: options.runtimePath,
      uses_default_features: false, features: ['runtime-benchmarks', 'hex-literal'] }, { name: 'unrelated' }] },
  { manifest_path: '/unrelated/Cargo.toml', dependencies: [] }] });
  const checkGuest = data => validateOrdinaryWbuildMetadata(encode(data), options);
  assert.equal(checkGuest(guest()).runtimePath, options.runtimePath);
  const rejectGuest = change => { const data = guest(); change(data, data.packages[0], data.packages[0].dependencies[0]); assert.throws(() => checkGuest(data)); };
  for (const [field, value] of [['source', 'registry'], ['path', '/wrong'], ['uses_default_features', true],
    ['rename', null], ['name', 'wrong'], ['features', ['hex-literal']],
    ['features', ['hex-literal', 'runtime-benchmarks', 'extra']], ['features', ['hex-literal', 'hex-literal']]]) {
    rejectGuest((_data, _root, dep) => { dep[field] = value; });
  }
  rejectGuest((_data, root) => { root.dependencies.push({ ...root.dependencies[0] }); });
  rejectGuest((_data, root) => { root.dependencies = []; });
  rejectGuest((data, root) => { data.packages.push(root); });
  rejectGuest((data) => { data.packages = []; });
  rejectGuest((_data, root) => { root.manifest_path = '/wrong'; });
  rejectGuest(data => { data.version = 2n; });
  for (const field of ['manifestPath', 'runtimePath']) {
    assert.throws(() => validateOrdinaryWbuildMetadata(encode(guest()), { ...options, [field]: 'relative' }));
  }
  return true;
}
