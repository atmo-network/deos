import assert from 'node:assert/strict';
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, symlinkSync, writeFileSync } from 'node:fs';
import { mkdtemp, rm } from 'node:fs/promises';
import { homedir, hostname, tmpdir } from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import test from 'node:test';
import {
  artifactManifest, assembleIdentity, authorityIdentity, canonicalJson, candidateIdentity,
  environmentIdentity, repetitionContract, semanticEnvironment, validateRecord, writeRecordAtomic,
} from './validation-evidence.mjs';

const helper = path.resolve('scripts/validation-evidence.mjs');
const actorBoundaries = [
  'actors.semantic-manifest.baseline-generate', 'actors.semantic-manifest.current-generate',
  'actors.semantic-manifest.exact-compare', 'actors.golden-equivalence.baseline-reactive-corpus',
  'actors.golden-equivalence.current-reactive-corpus', 'actors.golden-equivalence.baseline-semantic-anchor-family',
  'actors.golden-equivalence.current-semantic-anchor-family',
  'actors.scheduler.scheduler_stress_fifo_over_capacity_fairness_matrix',
  'actors.scheduler.scheduler_stress_fifo_dense_vs_sparse_topology_matrix',
  'actors.scheduler.scheduler_stress_fifo_sparse_topology_long_run_liveness',
  'actors.scheduler.stress_10k_actors_queue_scheduler',
  'actors.scheduler.checkpoint_a_s6_dense_10k_wakeups_converge_without_drops',
  'actors.scheduler.profile_scheduler_queue_wakeup_occupancy_10k',
];
const fullBoundaries = ['full.regeneration.canonical-pass-1', 'full.tracked-zero-drift.pass-1', ...actorBoundaries, 'full.regeneration.canonical-pass-2', 'full.tracked-zero-drift.pass-2', 'full.ignored-and-generated-artifacts.exact-sha256-compare'];
const semanticNames = Object.keys(semanticEnvironment('full'));
const fixtureRoots = new Set();
const testBin = mkdtempSync(path.join(tmpdir(), 'deos-evidence-tools-'));
const npmVersion = spawnSync('npm', ['--version'], { encoding: 'utf8' }).stdout.trim();
writeFileSync(path.join(testBin, 'npm'), `#!/usr/bin/env bash\nif [[ "\${1:-}" == config ]]; then printf 'userconfig=${homedir()}/.npmrc\\nglobalconfig=${homedir()}/.npm-globalrc\\n'; else printf '${npmVersion}\\n'; fi\n`);
chmodSync(path.join(testBin, 'npm'), 0o755);
process.on('exit', () => {
  rmSync(testBin, { recursive: true, force: true });
  for (const root of fixtureRoots) rmSync(root, { recursive: true, force: true });
});
function cleanEnvironment() {
  const env = { ...process.env, PATH: `${testBin}:${process.env.PATH}`, DEOS_VALIDATION_RESULT_JSON: '1', DEOS_VALIDATION_TEST_MODE: '1' };
  for (const name of semanticNames) delete env[name];
  for (const name of Object.keys(env)) if (/^(RUSTFLAGS|RUST_|RUSTDOCFLAGS|RUSTC_|RUSTUP_|CARGO_|NODE_|NPM_|NPM_CONFIG_|npm_config_|GIT_CONFIG_(COUNT|KEY_|VALUE_))/.test(name)) delete env[name];
  delete env.CI; delete env.DEOS_VALIDATION_CACHE;
  delete env.DEOS_PROJECT_ROOT; delete env.DEOS_BINARY_DIR;
  return env;
}
function command(name, args, options = {}) {
  const result = spawnSync(name, args, { cwd: options.cwd, env: options.env ?? cleanEnvironment(), encoding: 'utf8', timeout: options.timeout ?? 30_000 });
  if (!options.allowFailure && result.status !== 0) throw new Error(`${name} ${args.join(' ')} failed (${result.status}):\n${result.stdout}\n${result.stderr}`);
  return result;
}
function git(repo, ...args) { return command('git', ['-C', repo, ...args]).stdout.trim(); }
async function fixture() {
  const root = await mkdtemp(path.join(tmpdir(), 'deos-evidence-v2-'));
  fixtureRoots.add(root);
  const repo = path.join(root, 'repo');
  for (const dir of ['scripts', 'template', 'web-client', '.agents/skills/wiki-sync']) mkdirSync(path.join(repo, dir), { recursive: true });
  writeFileSync(path.join(repo, '.gitignore'), 'counter\nprepared\nbehavior.json\nartifact.bin\n');
  writeFileSync(path.join(repo, 'template/Cargo.lock'), 'lock\n');
  writeFileSync(path.join(repo, 'template/rust-toolchain.toml'), '[toolchain]\nchannel="stable"\n');
  writeFileSync(path.join(repo, 'web-client/package.json'), '{"name":"fixture"}\n');
  writeFileSync(path.join(repo, 'web-client/package-lock.json'), '{"lockfileVersion":3}\n');
  writeFileSync(path.join(repo, '.agents/skills/wiki-sync/package-lock.json'), '{}\n');
  writeFileSync(path.join(repo, 'scripts/validation-authority.v1.json'), `${JSON.stringify({
    schema: 'validation-authority/v2', roots: ['.gitignore', 'scripts', 'template', 'web-client', '.agents'],
    fullArtifactOutputs: [{ path: 'artifact.bin', kind: 'file', requiredMembers: [] }], immutableRefInputs: [],
  }, null, 2)}\n`);
  const shell = `#!/usr/bin/env bash
set -euo pipefail
repo_root="$(pwd -P)"
[[ "\${DEOS_PROJECT_ROOT:-}" == "$repo_root" ]]
[[ "\${DEOS_BINARY_DIR:-}" == "$repo_root/bin" ]]
if [[ -d "$DEOS_BINARY_DIR" ]]; then PATH="$DEOS_BINARY_DIR:$PATH"; fi
! command -v deos-hostile-tool >/dev/null 2>&1
profile="\${2:-}"
[[ "\${DEOS_VALIDATION_INTERNAL:-}" == 1 ]]
if [[ "\${1:-}" == --internal-prepare ]]; then printf prepared > prepared; exit 0; fi
[[ "\${1:-}" == --internal-run ]]
count=0; [[ ! -f counter ]] || count="$(cat counter)"; printf '%s' "$((count+1))" > counter
behavior="$(node -e 'const fs=require("fs"); try { process.stdout.write(JSON.stringify(JSON.parse(fs.readFileSync("behavior.json")))) } catch { process.stdout.write("{}") }')"
mode="$(node -e 'process.stdout.write(JSON.parse(process.argv[1]).mode||"")' "$behavior")"
if [[ "$mode" == fail ]]; then exit 9; fi
if [[ "$mode" == tracked ]]; then printf drift >> template/Cargo.lock; fi
if [[ "$mode" == staged ]]; then printf staged > staged.txt; git add staged.txt; fi
if [[ "$mode" == untracked ]]; then printf untracked > surprise.txt; fi
if [[ "$mode" == environment-drift ]]; then mkdir -p ../.cargo; printf '[build]\\nrustc-wrapper="/bin/false"\\n' > ../.cargo/config.toml; fi
if [[ "$mode" == environment-tmp-disappearance ]]; then rm -f "$TMPDIR/.cargo/config.toml"; fi
if [[ "$mode" == candidate-config-appearance ]]; then mkdir -p .cargo; printf '[build]\\nrustc-wrapper="/bin/false"\\n' > .cargo/config.toml; fi
if [[ "$mode" == candidate-config-disappearance ]]; then rm -f .cargo/config.toml; fi
if [[ "$mode" == candidate-config-content ]]; then printf '# drift\\n' >> .cargo/config.toml; fi
if [[ "$mode" == candidate-config-type ]]; then rm -f .cargo/config.toml; mkdir .cargo/config.toml; fi
if [[ "$mode" == candidate-symlink-target-content ]]; then printf '# drift\\n' >> config-target.toml; fi
ids='${JSON.stringify({ fast: [], heavy: actorBoundaries, full: fullBoundaries })}'
node -e 'const fs=require("fs"),{spawnSync}=require("child_process"); let a=JSON.parse(process.argv[1])[process.argv[2]]; const b=JSON.parse(process.argv[3]); if(b.mode==="skip") a=a.filter(x=>x!==b.id); if(b.mode==="reorder") a=[a[1],a[0],...a.slice(2)]; for(const id of a){if(b.mode==="second-pass-perturb"&&id==="full.regeneration.canonical-pass-1")fs.writeFileSync("artifact.bin","first"); if(b.mode==="second-pass-perturb"&&id==="full.regeneration.canonical-pass-2")fs.writeFileSync("artifact.bin","second"); if(b.mode==="second-pass-perturb"&&id==="full.ignored-and-generated-artifacts.exact-sha256-compare")process.exit(8); const r=spawnSync(process.execPath,[process.argv[4],"boundary",id],{stdio:"inherit",env:process.env}); if(r.status)process.exit(r.status)}' "$ids" "$profile" "$behavior" '${helper}'
if [[ "$profile" == full ]]; then printf '%s' "\${ARTIFACT_VALUE:-artifact}" > artifact.bin; fi
`;
  writeFileSync(path.join(repo, 'scripts/validate-local.sh'), shell); chmodSync(path.join(repo, 'scripts/validate-local.sh'), 0o755);
  command('git', ['init', '-q', repo]); git(repo, 'config', 'user.name', 'Evidence'); git(repo, 'config', 'user.email', 'e@example.test'); git(repo, 'add', '.'); git(repo, 'commit', '-qm', 'fixture');
  return repo;
}
function args(repo, profile = 'fast', extra = []) { return [helper, 'run', '--repo', repo, '--profile', profile, ...extra]; }
function runHelper(repo, profile = 'fast', options = {}) {
  const result = command('node', args(repo, profile, options.extra), { env: options.env, allowFailure: options.allowFailure, timeout: options.timeout });
  const line = result.stdout.trim().split('\n').filter(Boolean).at(-1);
  return { ...result, output: line?.startsWith('{') ? JSON.parse(line) : null };
}
function records(repo) { const root = path.join(repo, '.git/deos-validation/v2/records'); return existsSync(root) ? readdirSync(root).filter((x) => x.endsWith('.json')) : []; }
async function cleanupFixture(repo) { const root = path.dirname(repo); fixtureRoots.delete(root); return rm(root, { recursive: true, force: true }); }
function fakeNpmEnvironment(root) {
  const bin = path.join(root, 'bin');
  const userconfig = path.join(root, 'npm-user.rc');
  const globalconfig = path.join(root, 'npm-global.rc');
  mkdirSync(bin, { recursive: true });
  writeFileSync(userconfig, 'registry=https://registry.example/user\n');
  writeFileSync(globalconfig, 'registry=https://registry.example/global\n');
  writeFileSync(path.join(bin, 'npm'), `#!/usr/bin/env bash\nif [[ "\${1:-}" == config ]]; then printf 'userconfig=${userconfig}\\nglobalconfig=${globalconfig}\\n'; else printf '${npmVersion}\\n'; fi\n`);
  chmodSync(path.join(bin, 'npm'), 0o755);
  const env = cleanEnvironment(); env.PATH = `${bin}:${env.PATH}`;
  return { env, userconfig, globalconfig };
}
function ignorePath(repo, relativePath, mechanism) {
  if (mechanism === 'gitignore') {
    writeFileSync(path.join(repo, '.gitignore'), `${readFileSync(path.join(repo, '.gitignore'), 'utf8')}${relativePath}\n`);
    git(repo, 'add', '.gitignore'); git(repo, 'commit', '-qm', `ignore ${relativePath}`);
  } else if (mechanism === 'info') {
    writeFileSync(path.join(repo, '.git/info/exclude'), `${readFileSync(path.join(repo, '.git/info/exclude'), 'utf8')}\n${relativePath}\n`);
  } else if (mechanism === 'global') {
    const excludes = path.join(path.dirname(repo), 'global-excludes');
    writeFileSync(excludes, `${relativePath}\n`); git(repo, 'config', 'core.excludesFile', excludes);
  } else throw new Error(`Unknown ignore mechanism: ${mechanism}`);
}
function waitForPath(target, timeoutMs = 10_000) {
  const started = Date.now();
  return new Promise((resolve, reject) => {
    const inspect = () => {
      if (existsSync(target)) { resolve(); return; }
      if (Date.now() - started >= timeoutMs) { reject(new Error(`Timed out waiting for ${target}`)); return; }
      setTimeout(inspect, 10);
    };
    inspect();
  });
}

test('tree identity is topology-insensitive and authority manifest comes from candidate tree', async (t) => {
  const repo = await fixture(); t.after(() => rm(repo, { recursive: true, force: true }));
  const tree = candidateIdentity(repo); git(repo, 'commit', '--allow-empty', '-qm', 'topology'); assert.deepEqual(candidateIdentity(repo), tree);
  const first = await authorityIdentity(repo, tree.treeOid);
  writeFileSync(path.join(repo, 'scripts/validation-authority.v1.json'), '{"schema":"bad"}');
  const stillCandidate = await authorityIdentity(repo, tree.treeOid);
  assert.deepEqual(stillCandidate.identity, first.identity);
});

test('production helper exposes no arbitrary command or entrypoint CLI', () => {
  for (const option of ['--command-json', '--prepare-command-json', '--entrypoint', '--authority-manifest']) {
    const result = command('node', [helper, 'run', '--profile', 'fast', option, 'x'], { allowFailure: true });
    assert.notEqual(result.status, 0); assert.match(result.stderr, /Unknown run argument/);
  }
});

test('exact hit skips canonical execution; fresh and topology-only commit behave correctly', async (t) => {
  const repo = await fixture(); t.after(() => rm(repo, { recursive: true, force: true }));
  assert.equal(runHelper(repo).output.outcome, 'executed-recorded'); assert.equal(readFileSync(path.join(repo, 'counter'), 'utf8'), '1');
  assert.equal(runHelper(repo).output.outcome, 'reused'); git(repo, 'commit', '--allow-empty', '-qm', 'same tree'); assert.equal(runHelper(repo).output.outcome, 'reused');
  assert.equal(runHelper(repo, 'fast', { extra: ['--fresh'] }).output.outcome, 'executed-recorded'); assert.equal(readFileSync(path.join(repo, 'counter'), 'utf8'), '2');
});

test('blocked record lookup never reuses after tracked, staged, untracked, authority, or environment drift', async (t) => {
  const cases = {
    tracked(repo) { writeFileSync(path.join(repo, 'template/Cargo.lock'), 'tracked drift\n'); },
    staged(repo) { writeFileSync(path.join(repo, 'staged.txt'), 'staged\n'); git(repo, 'add', 'staged.txt'); },
    untracked(repo) { writeFileSync(path.join(repo, 'untracked.txt'), 'untracked\n'); },
    authority(repo) { writeFileSync(path.join(repo, 'scripts/validate-local.sh'), `${readFileSync(path.join(repo, 'scripts/validate-local.sh'), 'utf8')}\n# authority drift\n`); },
    environment(repo) { writeFileSync(path.join(path.dirname(repo), '.cargo/config.toml'), '[build]\nrustc-wrapper = "/bin/false"\n'); },
  };
  for (const [label, mutate] of Object.entries(cases)) {
    const repo = await fixture(); t.after(() => cleanupFixture(repo));
    if (label === 'environment') { mkdirSync(path.join(path.dirname(repo), '.cargo')); writeFileSync(path.join(path.dirname(repo), '.cargo/config.toml'), '[build]\nrustc-wrapper = "/bin/true"\n'); }
    runHelper(repo);
    const gate = path.join(path.dirname(repo), `record-read-${label}`);
    const env = cleanEnvironment(); env.DEOS_VALIDATION_TEST_RECORD_READ_GATE = gate;
    const child = spawn('node', args(repo), { env });
    const exited = new Promise((resolve) => child.once('exit', resolve));
    let stdout = ''; let stderr = '';
    child.stdout.on('data', (chunk) => { stdout += chunk; }); child.stderr.on('data', (chunk) => { stderr += chunk; });
    await waitForPath(`${gate}.ready`);
    mutate(repo);
    writeFileSync(`${gate}.release`, '');
    const status = await exited;
    assert.notEqual(status, 0, label);
    assert.doesNotMatch(stdout, /"outcome":"reused"/, label);
    assert.match(stderr, /changed|dirty/, label);
    assert.equal(readFileSync(path.join(repo, 'counter'), 'utf8'), '1', label);
  }
});

test('final hit lookup detects an ignored candidate-local config appearance', async (t) => {
  const repo = await fixture(); t.after(() => cleanupFixture(repo));
  ignorePath(repo, '.cargo/config.toml', 'info'); runHelper(repo);
  const gate = path.join(path.dirname(repo), 'record-read-ignored-config');
  const env = cleanEnvironment(); env.DEOS_VALIDATION_TEST_RECORD_READ_GATE = gate;
  const child = spawn('node', args(repo), { env });
  const exited = new Promise((resolve) => child.once('exit', resolve));
  let stdout = ''; let stderr = '';
  child.stdout.on('data', (chunk) => { stdout += chunk; }); child.stderr.on('data', (chunk) => { stderr += chunk; });
  await waitForPath(`${gate}.ready`);
  mkdirSync(path.join(repo, '.cargo')); writeFileSync(path.join(repo, '.cargo/config.toml'), '[build]\nrustc-wrapper="/bin/false"\n');
  assert.equal(git(repo, 'status', '--porcelain'), ''); writeFileSync(`${gate}.release`, '');
  const status = await exited;
  assert.notEqual(status, 0); assert.doesNotMatch(stdout, /"outcome":"reused"/); assert.match(stderr, /identity changed/);
  assert.equal(readFileSync(path.join(repo, 'counter'), 'utf8'), '1');
});

test('ancestor Cargo config path/content and npm user/global/ancestor drift change keys and prevent reuse', async (t) => {
  const cargoRepo = await fixture(); t.after(() => cleanupFixture(cargoRepo));
  const cargoRoot = path.dirname(cargoRepo); const cargoDirectory = path.join(cargoRoot, '.cargo');
  mkdirSync(cargoDirectory);
  const firstTarget = path.join(cargoRoot, 'cargo-a.toml'); const secondTarget = path.join(cargoRoot, 'cargo-b.toml'); const cargoConfig = path.join(cargoDirectory, 'config.toml');
  writeFileSync(firstTarget, '[build]\nrustc-wrapper = "/bin/true"\n'); writeFileSync(secondTarget, '[build]\nrustc-wrapper = "/bin/true"\n'); symlinkSync(firstTarget, cargoConfig);
  const cargoFirst = runHelper(cargoRepo);
  rmSync(cargoConfig); symlinkSync(secondTarget, cargoConfig);
  const cargoPathDrift = runHelper(cargoRepo); assert.notEqual(cargoPathDrift.output.key, cargoFirst.output.key);
  writeFileSync(secondTarget, '[build]\nrustc-wrapper = "/bin/false"\n');
  const cargoContentDrift = runHelper(cargoRepo); assert.notEqual(cargoContentDrift.output.key, cargoPathDrift.output.key);
  assert.equal(readFileSync(path.join(cargoRepo, 'counter'), 'utf8'), '3');
  rmSync(cargoConfig); symlinkSync(path.join(cargoRoot, 'missing.toml'), cargoConfig);
  assert.match(runHelper(cargoRepo, 'fast', { allowFailure: true }).stderr, /resolve external configuration/);

  const npmRepo = await fixture(); t.after(() => cleanupFixture(npmRepo));
  const npmRoot = path.dirname(npmRepo); const npm = fakeNpmEnvironment(npmRoot);
  writeFileSync(path.join(npmRoot, '.npmrc'), 'fund=false\n');
  const npmFirst = runHelper(npmRepo, 'fast', { env: npm.env });
  writeFileSync(npm.userconfig, 'registry=https://registry.example/user-2\n');
  const userDrift = runHelper(npmRepo, 'fast', { env: npm.env }); assert.notEqual(userDrift.output.key, npmFirst.output.key);
  writeFileSync(npm.globalconfig, 'registry=https://registry.example/global-2\n');
  const globalDrift = runHelper(npmRepo, 'fast', { env: npm.env }); assert.notEqual(globalDrift.output.key, userDrift.output.key);
  writeFileSync(path.join(npmRoot, '.npmrc'), 'fund=true\n');
  const ancestorDrift = runHelper(npmRepo, 'fast', { env: npm.env }); assert.notEqual(ancestorDrift.output.key, globalDrift.output.key);
  assert.equal(readFileSync(path.join(npmRepo, 'counter'), 'utf8'), '4');
});

test('ignored candidate-local Cargo and npm configs are environment-bound across ignore mechanisms', async (t) => {
  const cases = [
    ['.cargo/config', 'gitignore'],
    ['.cargo/config.toml', 'info'],
    ['.cargo/config.toml', 'global'],
    ['.npmrc', 'gitignore'],
    ['web-client/.npmrc', 'info'],
    ['.agents/skills/wiki-sync/.npmrc', 'global'],
  ];
  for (const [relativePath, mechanism] of cases) {
    const repo = await fixture(); t.after(() => cleanupFixture(repo));
    ignorePath(repo, relativePath, mechanism);
    const first = runHelper(repo);
    const absolute = path.join(repo, relativePath); mkdirSync(path.dirname(absolute), { recursive: true });
    writeFileSync(absolute, 'registry=https://hostile.invalid\nrustc-wrapper=/bin/false\n');
    assert.equal(git(repo, 'status', '--porcelain'), '', `${mechanism}:${relativePath}`);
    const appeared = runHelper(repo); assert.equal(appeared.output.outcome, 'executed-recorded'); assert.notEqual(appeared.output.key, first.output.key);
    writeFileSync(absolute, 'registry=https://changed.invalid\nrustc-wrapper=/bin/false\n');
    const changed = runHelper(repo); assert.equal(changed.output.outcome, 'executed-recorded'); assert.notEqual(changed.output.key, appeared.output.key);
    rmSync(absolute);
    const disappeared = runHelper(repo); assert.equal(disappeared.output.outcome, 'reused'); assert.equal(disappeared.output.key, first.output.key);
    assert.equal(readFileSync(path.join(repo, 'counter'), 'utf8'), '3');
  }
});

test('clean tracked regular configs stay tree-bound without checkout-path noise', async (t) => {
  const repos = [await fixture(), await fixture()];
  for (const repo of repos) {
    t.after(() => cleanupFixture(repo));
    mkdirSync(path.join(repo, '.cargo'), { recursive: true });
    writeFileSync(path.join(repo, '.cargo/config.toml'), '[build]\nincremental=false\n');
    writeFileSync(path.join(repo, '.npmrc'), 'fund=false\n');
    writeFileSync(path.join(repo, 'web-client/.npmrc'), 'audit=false\n');
    git(repo, 'add', '.cargo/config.toml', '.npmrc', 'web-client/.npmrc'); git(repo, 'commit', '-qm', 'tracked configs');
  }
  const identities = repos.map((repo) => environmentIdentity(repo, candidateIdentity(repo).treeOid, {}, cleanEnvironment()));
  assert.equal(identities[0].sha256, identities[1].sha256);
  for (let index = 0; index < repos.length; index += 1) {
    const serialized = canonicalJson(identities[index].values.externalConfiguration);
    assert.equal(serialized.includes(repos[index]), false);
  }
});

test('tracked config symlink binds target path, type, and content as environment', async (t) => {
  const repo = await fixture(); t.after(() => cleanupFixture(repo));
  ignorePath(repo, 'config-target.toml', 'gitignore');
  writeFileSync(path.join(repo, 'config-target.toml'), '[build]\nincremental=false\n');
  mkdirSync(path.join(repo, '.cargo'), { recursive: true }); symlinkSync('../config-target.toml', path.join(repo, '.cargo/config.toml'));
  git(repo, 'add', '.cargo/config.toml'); git(repo, 'commit', '-qm', 'tracked config symlink');
  const first = runHelper(repo);
  const firstIdentity = environmentIdentity(repo, candidateIdentity(repo).treeOid, {}, cleanEnvironment());
  const entry = firstIdentity.values.externalConfiguration.cargoConfigs.find((item) => item.path === path.join(repo, '.cargo/config.toml'));
  assert.equal(entry.type, 'symlink'); assert.equal(entry.linkTarget, '../config-target.toml');
  writeFileSync(path.join(repo, 'config-target.toml'), '[build]\nincremental=true\n');
  assert.equal(git(repo, 'status', '--porcelain'), '');
  const changed = runHelper(repo); assert.equal(changed.output.outcome, 'executed-recorded'); assert.notEqual(changed.output.key, first.output.key);
});

test('candidate-local effective config drift during execution fails closed', async (t) => {
  for (const mode of ['candidate-config-appearance', 'candidate-config-disappearance', 'candidate-config-content', 'candidate-config-type']) {
    const repo = await fixture(); t.after(() => cleanupFixture(repo));
    ignorePath(repo, '.cargo/config.toml', 'info');
    if (mode !== 'candidate-config-appearance') { mkdirSync(path.join(repo, '.cargo')); writeFileSync(path.join(repo, '.cargo/config.toml'), '[build]\nincremental=false\n'); }
    writeFileSync(path.join(repo, 'behavior.json'), JSON.stringify({ mode }));
    const result = runHelper(repo, 'fast', { extra: ['--fresh'], allowFailure: true });
    assert.notEqual(result.status, 0, mode); assert.match(result.stderr, /identity changed|not a regular file or symlink/, mode); assert.equal(records(repo).length, 0, mode);
  }
  const repo = await fixture(); t.after(() => cleanupFixture(repo));
  ignorePath(repo, 'config-target.toml', 'gitignore');
  writeFileSync(path.join(repo, 'config-target.toml'), '[build]\nincremental=false\n');
  mkdirSync(path.join(repo, '.cargo')); symlinkSync('../config-target.toml', path.join(repo, '.cargo/config.toml'));
  git(repo, 'add', '.cargo/config.toml'); git(repo, 'commit', '-qm', 'tracked config symlink');
  writeFileSync(path.join(repo, 'behavior.json'), JSON.stringify({ mode: 'candidate-symlink-target-content' }));
  const drift = runHelper(repo, 'fast', { extra: ['--fresh'], allowFailure: true });
  assert.notEqual(drift.status, 0); assert.match(drift.stderr, /identity changed/); assert.equal(records(repo).length, 0);
});

test('actual boundary report rejects skipped, reordered, and removed commands', async (t) => {
  for (const behavior of [{ mode: 'skip', id: actorBoundaries[3] }, { mode: 'reorder' }]) {
    const repo = await fixture(); t.after(() => rm(repo, { recursive: true, force: true })); writeFileSync(path.join(repo, 'behavior.json'), JSON.stringify(behavior));
    const result = runHelper(repo, 'heavy', { allowFailure: true }); assert.notEqual(result.status, 0); assert.match(result.stderr, /Boundary report mismatch/); assert.equal(records(repo).length, 0);
  }
});

test('failed command and tracked, staged, or untracked mid-run mutation never record', async (t) => {
  for (const mode of ['fail', 'tracked', 'staged', 'untracked']) {
    const repo = await fixture(); t.after(() => rm(repo, { recursive: true, force: true })); writeFileSync(path.join(repo, 'behavior.json'), JSON.stringify({ mode }));
    const result = runHelper(repo, 'fast', { allowFailure: true }); assert.notEqual(result.status, 0); assert.equal(records(repo).length, 0);
  }
});

test('direct validate-local mode overwrites caller DEOS paths before sourcing shared helpers', async (t) => {
  const root = await mkdtemp(path.join(tmpdir(), 'deos-hostile-direct-')); t.after(() => rm(root, { recursive: true, force: true }));
  mkdirSync(path.join(root, 'scripts'), { recursive: true });
  writeFileSync(path.join(root, 'scripts/_common.sh'), 'exit 77\n');
  const env = cleanEnvironment(); env.DEOS_PROJECT_ROOT = root; env.DEOS_BINARY_DIR = path.join(root, 'bin');
  const result = command('bash', ['scripts/validate-local.sh', '--help'], { env });
  assert.match(result.stdout, /DEOS_PROJECT_ROOT and DEOS_BINARY_DIR are always replaced/);
});

test('caller DEOS paths cannot redirect the candidate, inject tools, change the key, or prevent reuse', async (t) => {
  const repo = await fixture(); t.after(() => cleanupFixture(repo));
  const first = runHelper(repo);
  const hostileRoot = path.join(path.dirname(repo), 'hostile-project');
  const hostileBin = path.join(path.dirname(repo), 'hostile-bin');
  mkdirSync(path.join(hostileRoot, 'scripts'), { recursive: true }); mkdirSync(hostileBin);
  writeFileSync(path.join(hostileRoot, 'scripts/validate-local.sh'), '#!/usr/bin/env bash\nprintf redirected > "${TMPDIR:-/tmp}/deos-hostile-redirect"\n');
  writeFileSync(path.join(hostileBin, 'deos-hostile-tool'), '#!/usr/bin/env bash\nexit 99\n');
  chmodSync(path.join(hostileRoot, 'scripts/validate-local.sh'), 0o755); chmodSync(path.join(hostileBin, 'deos-hostile-tool'), 0o755);
  const env = cleanEnvironment(); env.DEOS_PROJECT_ROOT = hostileRoot; env.DEOS_BINARY_DIR = hostileBin;
  const hostile = runHelper(repo, 'fast', { env });
  assert.equal(hostile.output.outcome, 'reused'); assert.equal(hostile.output.key, first.output.key);
  assert.equal(readFileSync(path.join(repo, 'counter'), 'utf8'), '1');
});

test('nonexistent temporary config roots are path-stable while effective appearance and disappearance invalidate reuse', async (t) => {
  const repo = await fixture(); t.after(() => cleanupFixture(repo));
  const firstRoot = path.join(path.dirname(repo), 'nonexistent-a'); const secondRoot = path.join(path.dirname(repo), 'nonexistent-b');
  const firstEnv = cleanEnvironment(); firstEnv.TMPDIR = firstRoot;
  const secondEnv = cleanEnvironment(); secondEnv.TMPDIR = secondRoot;
  const first = runHelper(repo, 'fast', { env: firstEnv });
  const second = runHelper(repo, 'fast', { env: secondEnv });
  assert.equal(second.output.outcome, 'reused'); assert.equal(second.output.key, first.output.key);
  mkdirSync(path.join(secondRoot, '.cargo'), { recursive: true }); writeFileSync(path.join(secondRoot, '.cargo/config.toml'), '[build]\nincremental=false\n');
  const appeared = runHelper(repo, 'fast', { env: secondEnv }); assert.equal(appeared.output.outcome, 'executed-recorded'); assert.notEqual(appeared.output.key, first.output.key);
  writeFileSync(path.join(repo, 'behavior.json'), JSON.stringify({ mode: 'environment-tmp-disappearance' }));
  const disappeared = runHelper(repo, 'fast', { env: secondEnv, extra: ['--fresh'], allowFailure: true });
  assert.notEqual(disappeared.status, 0); assert.match(disappeared.stderr, /identity changed/);
});

test('inherited compiler, Cargo, Node, and npm controls fail before hit or execution', async (t) => {
  const repo = await fixture(); t.after(() => rm(repo, { recursive: true, force: true })); runHelper(repo);
  for (const name of ['RUSTFLAGS', 'RUST_BACKTRACE', 'RUSTC', 'CARGO', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTDOCFLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_HOME', 'CARGO_TARGET_DIR', 'CARGO_TERM_COLOR', 'NODE_EXTRA_CA_CERTS', 'NODE_OPTIONS', 'npm_config_registry', 'NPM_CONFIG_REGISTRY']) {
    const env = cleanEnvironment(); env[name] = 'changed';
    const result = runHelper(repo, 'fast', { env, allowFailure: true }); assert.notEqual(result.status, 0, name); assert.match(result.stderr, /Unsupported inherited validation control/, name);
  }
  assert.equal(readFileSync(path.join(repo, 'counter'), 'utf8'), '1');
});

test('external configuration drift during canonical execution fails pre/post identity and records nothing', async (t) => {
  const repo = await fixture(); t.after(() => cleanupFixture(repo));
  mkdirSync(path.join(path.dirname(repo), '.cargo'));
  writeFileSync(path.join(path.dirname(repo), '.cargo/config.toml'), '[build]\nrustc-wrapper="/bin/true"\n');
  writeFileSync(path.join(repo, 'behavior.json'), JSON.stringify({ mode: 'environment-drift' }));
  const result = runHelper(repo, 'fast', { allowFailure: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /identity changed/);
  assert.equal(records(repo).length, 0);
});

test('dirty local fast is uncached while full and CI fail closed', async (t) => {
  const repo = await fixture(); t.after(() => rm(repo, { recursive: true, force: true })); writeFileSync(path.join(repo, 'dirty.txt'), 'dirty');
  assert.equal(runHelper(repo).output.outcome, 'executed-dirty'); assert.equal(records(repo).length, 0);
  assert.notEqual(runHelper(repo, 'full', { allowFailure: true }).status, 0);
  const env = cleanEnvironment(); env.CI = 'true'; assert.notEqual(runHelper(repo, 'fast', { env, allowFailure: true }).status, 0);
});

test('end-to-end second-pass artifact perturbation prevents a full record', async (t) => {
  const repo = await fixture(); t.after(() => rm(repo, { recursive: true, force: true })); writeFileSync(path.join(repo, 'behavior.json'), JSON.stringify({ mode: 'second-pass-perturb' }));
  const result = runHelper(repo, 'full', { allowFailure: true }); assert.notEqual(result.status, 0); assert.equal(records(repo).length, 0);
});

test('invalid full artifacts miss, execute, and atomically replace evidence', async (t) => {
  const repo = await fixture(); t.after(() => cleanupFixture(repo));
  const first = runHelper(repo, 'full');
  assert.equal(first.output.outcome, 'executed-recorded');
  const firstRecord = readFileSync(first.output.recordPath, 'utf8');
  assert.equal(runHelper(repo, 'full').output.outcome, 'reused');
  for (const value of ['drift', null, '']) {
    if (value === null) await rm(path.join(repo, 'artifact.bin')); else writeFileSync(path.join(repo, 'artifact.bin'), value);
    const rerun = runHelper(repo, 'full');
    assert.equal(rerun.output.outcome, 'executed-recorded');
  }
  assert.equal(readFileSync(path.join(repo, 'counter'), 'utf8'), '4');
  assert.notEqual(readFileSync(first.output.recordPath, 'utf8'), firstRecord);

  writeFileSync(path.join(repo, 'artifact.bin'), 'stale');
  writeFileSync(path.join(repo, 'behavior.json'), JSON.stringify({ mode: 'fail' }));
  const failed = runHelper(repo, 'full', { allowFailure: true });
  assert.notEqual(failed.status, 0);
  assert.equal(records(repo).length, 0);
  writeFileSync(path.join(repo, 'behavior.json'), '{}');
  assert.equal(runHelper(repo, 'full').output.outcome, 'executed-recorded');
  assert.equal(readFileSync(path.join(repo, 'counter'), 'utf8'), '6');
});

test('ref tag drift and deletion do not affect immutable commit-bound identity', async (t) => {
  const repo = await fixture(); t.after(() => rm(repo, { recursive: true, force: true })); git(repo, 'tag', 'mutable-baseline');
  const first = runHelper(repo, 'heavy'); git(repo, 'tag', '-f', 'mutable-baseline', 'HEAD'); assert.equal(runHelper(repo, 'heavy').output.key, first.output.key);
  git(repo, 'tag', '-d', 'mutable-baseline'); assert.equal(runHelper(repo, 'heavy').output.outcome, 'reused');
  const production = JSON.parse(readFileSync('scripts/validation-authority.v1.json'));
  assert.deepEqual(production.immutableRefInputs[0], { profiles: ['heavy', 'full'], name: 'deos-actors-0.7.17-baseline', commit: 'e5bcb85ceb93f201add3db0df08f2583930287c8' });
  const oracle = JSON.parse(readFileSync('template/pallets/actors/tests/fixtures/golden-equivalence.v1.json'));
  assert.equal(production.immutableRefInputs[0].commit, oracle.baseline.commit);
  assert.doesNotMatch(readFileSync('scripts/actors-golden-equivalence.sh', 'utf8'), /v0\.7\.17\^\{commit\}/);
  assert.doesNotMatch(readFileSync('scripts/validate-actors-golden-equivalence.mjs', 'utf8'), /baseline\.tag.*\^\{commit\}/s);
});

test('same-key concurrency executes once across linked worktrees', async (t) => {
  const repo = await fixture(); const linked = `${repo}-linked`; t.after(() => Promise.all([rm(repo, { recursive: true, force: true }), rm(linked, { recursive: true, force: true })]));
  git(repo, 'worktree', 'add', '-q', '--detach', linked, 'HEAD');
  const launch = (cwd) => new Promise((resolve) => { const child = spawn('node', args(cwd), { env: cleanEnvironment() }); let out = ''; child.stdout.on('data', (x) => { out += x; }); child.on('exit', (code) => resolve({ code, out })); });
  const results = await Promise.all([launch(repo), launch(linked)]); assert.deepEqual(results.map((x) => x.code), [0, 0]);
  const outcomes = results.map((x) => JSON.parse(x.out.trim().split('\n').at(-1)).outcome).sort(); assert.deepEqual(outcomes, ['executed-recorded', 'reused']);
});

test('dead lock has one atomic reclaimer and remote lock fails closed', async (t) => {
  const repo = await fixture(); t.after(() => rm(repo, { recursive: true, force: true }));
  const lock = path.join(repo, '.git/deos-validation/v2/lock'); mkdirSync(lock, { recursive: true });
  writeFileSync(path.join(lock, 'owner.json'), JSON.stringify({ schema: 'deos-validation-lock/v2', token: 'dead-token', hostname: hostname(), pid: 2147483647, processStartIdentity: '1', key: 'x', acquiredAt: new Date().toISOString() }));
  const launch = () => new Promise((resolve) => { const child = spawn('node', args(repo), { env: cleanEnvironment() }); let out = ''; child.stdout.on('data', (x) => { out += x; }); child.on('exit', (code) => resolve({ code, out })); });
  const results = await Promise.all([launch(), launch()]); assert.deepEqual(results.map((x) => x.code), [0, 0]); assert.equal(readFileSync(path.join(repo, 'counter'), 'utf8'), '1');
  await rm(path.join(repo, '.git/deos-validation/v2'), { recursive: true, force: true }); mkdirSync(lock, { recursive: true });
  writeFileSync(path.join(lock, 'owner.json'), JSON.stringify({ schema: 'deos-validation-lock/v2', token: 'remote', hostname: 'remote.invalid', pid: 1, processStartIdentity: '1', key: 'x', acquiredAt: new Date().toISOString() }));
  const remote = runHelper(repo, 'fast', { allowFailure: true }); assert.notEqual(remote.status, 0); assert.match(remote.stderr, /remote/);
});

test('stale-lock displacement never deletes a live replacement', async (t) => {
  const repo = await fixture(); t.after(() => rm(repo, { recursive: true, force: true }));
  const lock = path.join(repo, '.git/deos-validation/v2/lock'); mkdirSync(lock, { recursive: true });
  writeFileSync(path.join(lock, 'owner.json'), JSON.stringify({ schema: 'deos-validation-lock/v2', token: 'dead-token', hostname: hostname(), pid: 2147483647, processStartIdentity: '1', key: 'x', acquiredAt: new Date().toISOString() }));
  const env = cleanEnvironment(); env.DEOS_VALIDATION_TEST_RECLAIM_DELAY_MS = '300';
  const child = spawn('node', args(repo), { env, stdio: 'ignore' });
  await new Promise((resolve) => setTimeout(resolve, 120));
  await rm(lock, { recursive: true, force: true }); mkdirSync(lock, { recursive: true });
  writeFileSync(path.join(lock, 'owner.json'), JSON.stringify({ schema: 'deos-validation-lock/v2', token: 'live-replacement', hostname: 'remote.invalid', pid: 1, processStartIdentity: '1', key: 'replacement', acquiredAt: new Date().toISOString() }));
  const code = await new Promise((resolve) => child.on('exit', resolve)); assert.notEqual(code, 0);
  const roots = readdirSync(path.dirname(lock)).filter((entry) => entry === 'lock' || entry.startsWith('lock.stale.'));
  const owners = roots.map((entry) => JSON.parse(readFileSync(path.join(path.dirname(lock), entry, 'owner.json'))));
  assert.ok(owners.some((owner) => owner.token === 'live-replacement'));
});

test('SIGTERM cleans an owned lock and publishes no record', async (t) => {
  const repo = await fixture(); t.after(() => rm(repo, { recursive: true, force: true })); writeFileSync(path.join(repo, 'behavior.json'), JSON.stringify({ mode: 'fail' }));
  const child = spawn('node', args(repo), { env: cleanEnvironment(), stdio: 'ignore' });
  await new Promise((resolve) => setTimeout(resolve, 150)); child.kill('SIGTERM'); await new Promise((resolve) => child.on('exit', resolve));
  await new Promise((resolve) => setTimeout(resolve, 150)); assert.equal(existsSync(path.join(repo, '.git/deos-validation/v2/lock')), false); assert.equal(records(repo).length, 0);
});

test('record atomicity, strict schema, and repetition contract are not self-declared completion', async (t) => {
  const root = await mkdtemp(path.join(tmpdir(), 'deos-record-v2-')); t.after(() => rm(root, { recursive: true, force: true }));
  const target = path.join(root, 'records/key.json'); await assert.rejects(writeRecordAtomic(target, { ok: true }, { beforeRename: async () => { throw new Error('crash'); } }), /crash/); assert.equal(existsSync(target), false);
  await writeRecordAtomic(target, { ok: true }); assert.equal(statSync(target).mode & 0o777, 0o600);
  assert.deepEqual(Object.keys(repetitionContract('heavy')), ['contract', 'contractSha256']);
  const base = { candidate: { treeOid: '0'.repeat(40), indexTreeOid: '0'.repeat(40), trackedClean: true, stagedClean: true, untracked: [], clean: true }, authority: { schema: 'validation-authority/v2', sha256: `sha256:${'a'.repeat(64)}` }, environment: { schema: 'deos-validation-environment/v2', sha256: `sha256:${'b'.repeat(64)}`, values: {} }, profile: 'fast' };
  assert.notEqual(assembleIdentity(base).key, assembleIdentity({ ...base, profile: 'heavy' }).key);
});

test('artifact owner definitions are exact and authority roots cover independently discovered command owners', async () => {
  const authority = JSON.parse(readFileSync('scripts/validation-authority.v1.json'));
  const descriptor = authority.fullArtifactOutputs.find((x) => x.path === 'web-client/.papi/descriptors'); assert.deepEqual(descriptor.requiredMembers, ['package.json', 'generated.json', 'dist/index.js', 'dist/index.d.ts']);
  const queue = ['scripts/validate-local.sh', 'scripts/actors-assurance.sh', 'scripts/actors-golden-equivalence.sh'];
  const references = new Set(queue);
  for (const file of queue) {
    const source = readFileSync(file, 'utf8');
    for (const match of source.matchAll(/run_script_step\s+"[^"]+"\s+"([A-Za-z0-9_.-]+\.(?:sh|mjs))"/g)) references.add(`scripts/${match[1]}`);
    for (const match of source.matchAll(/run_alignment_script_step\s+"[^"]+"\s+([A-Za-z0-9_.-]+\.sh)/g)) references.add(`.agents/skills/alignment/scripts/${match[1]}`);
    for (const match of source.matchAll(/(?:\$PROJECT_ROOT\/|\.\/)?scripts\/([A-Za-z0-9_.-]+\.(?:sh|mjs))/g)) references.add(`scripts/${match[1]}`);
    if (/npm run /.test(source)) references.add('web-client/package.json');
    if (/cargo /.test(source)) references.add('template/Cargo.toml');
  }
  assert.ok(references.size >= 15, `independent owner discovery reached only ${references.size} paths`);
  const covered = (candidate) => authority.roots.some((root) => candidate === root || candidate.startsWith(`${root}/`));
  for (const owner of references) assert.equal(covered(owner), true, owner);
  assert.equal(authority.schema, 'validation-authority/v2');
});

test('environment identity binds external Cargo/npm configuration and rejects changed controls', () => {
  const tree = candidateIdentity('.').treeOid;
  const identity = environmentIdentity('.', tree, {}, cleanEnvironment());
  assert.deepEqual(Object.keys(identity.values.externalConfiguration).sort(), ['cargoConfigs', 'cargoHome', 'effectiveNpm', 'npmConfigs']);
  assert.equal(identity.values.externalConfiguration.effectiveNpm.length, 3);
  assert.ok(identity.values.externalConfiguration.cargoConfigs.every((entry) => entry.exists === true));
  assert.ok(identity.values.externalConfiguration.npmConfigs.every((entry) => entry.exists === true));
  const env = cleanEnvironment(); env.RUSTFLAGS = '-C target-cpu=native'; assert.throws(() => environmentIdentity('.', tree, {}, env), /Unsupported inherited/);
});
