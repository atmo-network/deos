import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { mkdtemp, mkdir, readFile, rm, symlink, writeFile, open } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import test from 'node:test';
import { assembleIdentity, canonicalJson } from './validation-evidence.mjs';
import {
  appendOutput, discoverTrustedArtifact, discoverTrustedArtifactOrNull, GitHubApi,
  validateCiEnvelope, verifyCiEvidence,
} from './validation-ci-evidence.mjs';

const repository = 'atmo-network/deos';
const repositoryId = 101;
const treeOid = 'a'.repeat(40);
const baseSha = 'b'.repeat(40);
const headSha = 'c'.repeat(40);
const workflowDigest = `sha256:${'d'.repeat(64)}`;
const runId = 202;
const runAttempt = 3;
const artifactId = 303;
const jobId = 404;
const workflowPath = '.github/workflows/ci.yml';
const artifactName = `deos-validation-fast-${treeOid}-${runId}-${runAttempt}`;

function hash(value) { return `sha256:${createHash('sha256').update(value).digest('hex')}`; }
function clone(value) { return structuredClone(value); }
function expectedIdentity() {
  return assembleIdentity({
    candidate: { treeOid, indexTreeOid: treeOid, trackedClean: true, stagedClean: true, untracked: [], clean: true },
    authority: { schema: 'validation-authority/v2', sha256: `sha256:${'e'.repeat(64)}` },
    environment: { schema: 'deos-validation-environment/v2', sha256: hash('{}'), values: {} },
    profile: 'fast',
  });
}
function localRecord(identity = expectedIdentity()) {
  return {
    schema: 'deos-validation-evidence/v2', key: identity.key,
    candidate: identity.candidate, authority: identity.authority, environment: identity.environment,
    refIdentity: identity.refIdentity, freshness: identity.freshness, invocation: identity.invocation,
    repetition: identity.repetition, artifacts: null, conclusion: 'success',
    startedAt: '2026-08-16T00:00:00.000Z', completedAt: '2026-08-16T00:01:00.000Z',
  };
}
function envelope(identity = expectedIdentity()) {
  return {
    schema: 'deos-validation-ci-evidence/v1', localEvidence: localRecord(identity),
    workflow: {
      repository, repositoryId, workflowPath,
      workflowAuthority: { commit: baseSha, sha256: workflowDigest },
      event: 'pull_request', runId, runAttempt, job: 'validation-gate', headSha,
      headRepository: repository, headRepositoryId: repositoryId, baseSha, artifactName,
      conclusion: 'success',
    },
  };
}
function discovery() {
  return {
    schema: 'deos-validation-ci-discovery/v1', repository, repositoryId, workflowPath,
    workflowAuthority: { commit: baseSha, sha256: workflowDigest }, event: 'pull_request',
    runId, runAttempt, job: 'validation-gate', jobId, headSha, headRepository: repository,
    headRepositoryId: repositoryId, baseSha,
    artifact: { id: artifactId, name: artifactName, expired: false },
    runConclusion: 'success', jobConclusion: 'success',
  };
}
function apiFixture(mutator = () => {}) {
  const workflowBytes = Buffer.from('name: CI\n');
  const values = {
    pull: { merged_at: '2026-08-16T00:00:00Z', base: { sha: baseSha, repo: { full_name: repository, id: repositoryId } }, head: { sha: headSha, repo: { full_name: repository, id: repositoryId } } },
    run: { id: runId, run_attempt: runAttempt, repository: { full_name: repository, id: repositoryId }, path: workflowPath, event: 'pull_request', status: 'completed', conclusion: 'success', head_sha: headSha, head_repository: { full_name: repository, id: repositoryId } },
    baseContents: { type: 'file', encoding: 'base64', content: workflowBytes.toString('base64') },
    headContents: { type: 'file', encoding: 'base64', content: workflowBytes.toString('base64') },
    job: { id: jobId, name: 'validation-gate', status: 'completed', conclusion: 'success', run_attempt: runAttempt, head_sha: headSha },
    artifact: { id: artifactId, name: artifactName, expired: false, workflow_run: { id: runId, repository_id: repositoryId, head_repository_id: repositoryId, head_sha: headSha } },
  };
  mutator(values);
  return async (endpoint) => {
    if (endpoint.includes(`/commits/${'f'.repeat(40)}/pulls`)) return [values.pull];
    if (endpoint.includes('/actions/workflows/')) return { workflow_runs: values.runs ?? [values.run] };
    if (endpoint.includes(`/contents/${workflowPath}?ref=${baseSha}`)) return values.baseContents;
    if (endpoint.includes(`/contents/${workflowPath}?ref=${headSha}`)) return values.headContents;
    if (endpoint.includes('/jobs?')) return { jobs: values.jobs ?? [values.job] };
    if (endpoint.includes('/artifacts?')) return { artifacts: values.artifacts ?? [values.artifact] };
    throw new Error(`Unexpected endpoint ${endpoint}`);
  };
}

async function discover(api = apiFixture()) {
  return discoverTrustedArtifact({ repository, repositoryId, mainSha: 'f'.repeat(40), api });
}
function runCli(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ['./scripts/validation-ci-evidence.mjs', ...args], { cwd: process.cwd() });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => { stdout += chunk; });
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    child.on('error', reject);
    child.on('close', (code) => resolve({ code, stdout, stderr }));
  });
}
async function withTempDirectory(action) {
  const directory = await mkdtemp(path.join(tmpdir(), 'deos-ci-evidence-test-'));
  try { return await action(directory); }
  finally { await rm(directory, { recursive: true, force: true }); }
}

test('validation workflows do not inject forbidden inherited Cargo presentation controls', async () => {
  for (const file of ['.github/workflows/ci.yml', '.github/workflows/_validate.yml']) {
    const source = await readFile(file, 'utf8');
    assert.doesNotMatch(source, /^\s*CARGO_TERM_COLOR:/m, file);
  }
});

test('trusted discovery independently binds repository, workflow, run, job, head, artifact, and stable base authority', async () => {
  const found = await discover();
  assert.equal(found.repository, repository);
  assert.equal(found.artifact.id, artifactId);
  assert.equal(found.workflowAuthority.sha256, hash('name: CI\n'));
});

test('workflow authority bootstrap and every GitHub trust mismatch cause a miss', async () => {
  const cases = [
    ['fork', (x) => { x.pull.head.repo.full_name = 'fork/deos'; }],
    ['repository', (x) => { x.run.repository.full_name = 'other/deos'; }],
    ['event', (x) => { x.run.event = 'push'; }],
    ['workflow', (x) => { x.run.path = '.github/workflows/other.yml'; }],
    ['run conclusion', (x) => { x.run.conclusion = 'failure'; }],
    ['cancelled run', (x) => { x.run.conclusion = 'cancelled'; }],
    ['attempt', (x) => { x.job.run_attempt = runAttempt + 1; }],
    ['job', (x) => { x.job.name = 'other'; }],
    ['job conclusion', (x) => { x.job.conclusion = 'failure'; }],
    ['head sha', (x) => { x.run.head_sha = '1'.repeat(40); }],
    ['head repository', (x) => { x.run.head_repository.full_name = 'fork/deos'; }],
    ['authority bootstrap', (x) => { x.headContents.content = Buffer.from('changed\n').toString('base64'); }],
    ['expired', (x) => { x.artifact.expired = true; }],
    ['artifact run', (x) => { x.artifact.workflow_run.id = runId + 1; }],
    ['artifact head', (x) => { x.artifact.workflow_run.head_sha = '2'.repeat(40); }],
    ['artifact spoof', (x) => { x.artifact.name = `deos-validation-fast-${treeOid}-999-1`; }],
    ['duplicate jobs', (x) => { x.jobs = [x.job, clone(x.job)]; }],
    ['duplicate artifacts', (x) => { x.artifacts = [x.artifact, clone(x.artifact)]; }],
    ['replacement artifact', (x) => { const replacement = clone(x.artifact); replacement.id += 1; x.artifacts = [x.artifact, replacement]; }],
    ['prior-attempt artifact', (x) => { const prior = clone(x.artifact); prior.name = `deos-validation-fast-${treeOid}-${runId}-${runAttempt - 1}`; x.artifacts = [x.artifact, prior]; }],
    ['duplicate run identity', (x) => { x.runs = [x.run, clone(x.run)]; }],
    ['multiple run attempts', (x) => { const prior = clone(x.run); prior.run_attempt -= 1; x.runs = [x.run, prior]; }],
  ];
  for (const [label, mutate] of cases) assert.equal(await discover(apiFixture(mutate)), null, label);
});

test('unavailable GitHub API is an explicit fallback miss', async () => {
  let observed;
  const result = await discoverTrustedArtifactOrNull({ repository, repositoryId, mainSha: 'f'.repeat(40), api: async () => { throw new Error('offline'); } }, (error) => { observed = error.message; });
  assert.equal(result, null);
  assert.equal(observed, 'offline');
});

test('a never-settling GitHub API request is aborted within its bounded timeout', async () => {
  let signal;
  const api = new GitHubApi('token', 'https://api.invalid', {
    requestTimeoutMs: 20,
    discoveryTimeoutMs: 100,
    fetch: (_url, options) => { signal = options.signal; return new Promise(() => {}); },
  });
  const started = Date.now();
  await assert.rejects(api.get('/stalled'), /timed out/);
  assert.equal(signal.aborted, true);
  assert.ok(Date.now() - started < 500);
});

test('identical tree evidence survives squash/topology SHA changes', () => {
  const identity = expectedIdentity();
  const artifact = envelope(identity);
  const api = discovery();
  api.headSha = '7'.repeat(40); artifact.workflow.headSha = api.headSha;
  assert.equal(verifyCiEvidence({ envelope: artifact, discovery: api, expectedIdentity: identity, repository, repositoryId, workflowSha256: workflowDigest }), true);
});

test('artifact-internal claims never override API provenance or exact semantic identity', () => {
  const identity = expectedIdentity();
  const cases = [
    ['repository', (artifact) => { artifact.workflow.repository = 'other/deos'; }],
    ['event', (artifact) => { artifact.workflow.event = 'push'; }],
    ['workflow', (artifact) => { artifact.workflow.workflowPath = '.github/workflows/other.yml'; }],
    ['run', (artifact) => { artifact.workflow.runId += 1; }],
    ['attempt', (artifact) => { artifact.workflow.runAttempt += 1; }],
    ['job', (artifact) => { artifact.workflow.job = 'other'; }],
    ['head repository', (artifact) => { artifact.workflow.headRepository = 'fork/deos'; }],
    ['head sha', (artifact) => { artifact.workflow.headSha = '8'.repeat(40); }],
    ['conclusion', (artifact) => { artifact.workflow.conclusion = 'failure'; }],
    ['authority', (artifact) => { artifact.workflow.workflowAuthority.sha256 = `sha256:${'9'.repeat(64)}`; }],
    ['tree', (artifact) => { artifact.localEvidence.candidate.treeOid = '6'.repeat(40); }],
    ['environment', (artifact) => { artifact.localEvidence.environment.values.changed = true; }],
    ['profile', (artifact) => { artifact.localEvidence.invocation.profile = 'heavy'; }],
    ['argv', (artifact) => { artifact.localEvidence.invocation.argv = ['heavy']; }],
    ['freshness', (artifact) => { artifact.localEvidence.freshness.inputs = ['network']; }],
    ['repetition', (artifact) => { artifact.localEvidence.repetition.contractSha256 = `sha256:${'5'.repeat(64)}`; }],
  ];
  for (const [label, mutate] of cases) {
    const artifact = envelope(identity); mutate(artifact);
    assert.throws(() => verifyCiEvidence({ envelope: artifact, discovery: discovery(), expectedIdentity: identity, repository, repositoryId, workflowSha256: workflowDigest }), undefined, label);
  }
  assert.throws(() => validateCiEnvelope({ schema: 'spoof' }), /fields|schema/);
});

test('API discovery claims are revalidated before artifact equivalence', () => {
  const identity = expectedIdentity();
  const cases = [
    ['expired', (api) => { api.artifact.expired = true; }],
    ['run failure', (api) => { api.runConclusion = 'failure'; }],
    ['job failure', (api) => { api.jobConclusion = 'failure'; }],
    ['repository', (api) => { api.repository = 'other/deos'; }],
    ['authority', (api) => { api.workflowAuthority.sha256 = `sha256:${'0'.repeat(64)}`; }],
  ];
  for (const [label, mutate] of cases) {
    const api = discovery(); mutate(api);
    assert.throws(() => verifyCiEvidence({ envelope: envelope(identity), discovery: api, expectedIdentity: identity, repository, repositoryId, workflowSha256: workflowDigest }), undefined, label);
  }
});

test('artifact CLI rejects extra, nonregular, traversal-shaped, oversized, truncated, and corrupt payloads', async () => {
  const cases = [
    ['extra file', async (directory) => { await writeFile(path.join(directory, 'evidence.json'), '{}'); await writeFile(path.join(directory, 'extra'), 'x'); }],
    ['extra directory', async (directory) => { await writeFile(path.join(directory, 'evidence.json'), '{}'); await mkdir(path.join(directory, 'extra')); }],
    ['symlink', async (directory, root) => { const target = path.join(root, 'target.json'); await writeFile(target, '{}'); await symlink(target, path.join(directory, 'evidence.json')); }],
    ['traversal-shaped entry', async (directory) => { await writeFile(path.join(directory, 'evidence.json'), '{}'); await writeFile(path.join(directory, '..evidence.json'), '{}'); }],
    ['oversized', async (directory) => { const handle = await open(path.join(directory, 'evidence.json'), 'w'); await handle.truncate(8 * 1024 * 1024 + 1); await handle.close(); }],
    ['truncated', async (directory) => { await writeFile(path.join(directory, 'evidence.json'), '{"schema":'); }],
    ['corrupt', async (directory) => { await writeFile(path.join(directory, 'evidence.json'), 'not-json'); }],
  ];
  for (const [label, prepare] of cases) {
    await withTempDirectory(async (root) => {
      const artifactDirectory = path.join(root, 'artifact');
      await mkdir(artifactDirectory);
      await prepare(artifactDirectory, root);
      const discoveryPath = path.join(root, 'discovery.json');
      const outputPath = path.join(root, 'github-output');
      await writeFile(discoveryPath, '{}');
      const result = await runCli(['decide', '--repo', '.', '--repository', repository, '--repository-id', String(repositoryId), '--discovery', discoveryPath, '--artifact-directory', artifactDirectory, '--github-output', outputPath]);
      assert.equal(result.code, 0, `${label}: ${result.stderr}`);
      assert.equal(await readFile(outputPath, 'utf8'), 'reuse=false\n', label);
    });
  }
});

test('GitHub output is bounded and injection-safe, while output failure exits without reusable evidence', async () => {
  await withTempDirectory(async (root) => {
    const outputPath = path.join(root, 'github-output');
    await assert.rejects(appendOutput(outputPath, { reuse: 'true\nforged=true' }), /unsafe/);
    await assert.rejects(appendOutput(outputPath, { ['bad\nkey']: 'true' }), /unsafe/);
    await assert.rejects(appendOutput(outputPath, { reuse: 'x'.repeat(1025) }), /unsafe/);

    const artifactDirectory = path.join(root, 'artifact');
    const discoveryPath = path.join(root, 'discovery.json');
    const outputDirectory = path.join(root, 'output-directory');
    await mkdir(artifactDirectory);
    await mkdir(outputDirectory);
    await writeFile(path.join(artifactDirectory, 'evidence.json'), 'not-json');
    await writeFile(discoveryPath, '{}');
    const result = await runCli(['decide', '--repo', '.', '--repository', repository, '--repository-id', String(repositoryId), '--discovery', discoveryPath, '--artifact-directory', artifactDirectory, '--github-output', outputDirectory]);
    assert.notEqual(result.code, 0);
    assert.match(result.stderr, /ERROR/);
  });
});

test('workflow static contract keeps one stable gate, least permissions, exact action pins, cache exclusion, and fresh release behavior', () => {
  const workflows = ['.github/workflows/_validate.yml', '.github/workflows/ci.yml', '.github/workflows/release-candidate.yml', '.github/workflows/stress-lane.yml', '.github/workflows/delete-workflow-runs.yml'];
  for (const file of workflows) {
    const source = readFileSync(file, 'utf8');
    assert.doesNotMatch(source, /ubuntu-latest/, file);
    assert.match(source, /^permissions:\n/m, `${file}: explicit permissions`);
    for (const match of source.matchAll(/^\s*-?\s*uses:\s*(\S+)/gm)) {
      if (match[1].startsWith('./')) assert.equal(match[1], './.github/workflows/_validate.yml', file);
      else assert.match(match[1], /@[0-9a-f]{40}$/, `${file}: ${match[1]}`);
    }
  }
  const ci = readFileSync('.github/workflows/ci.yml', 'utf8');
  assert.equal((ci.match(/^\s{2}validation-gate:$/gm) ?? []).length, 1);
  assert.match(ci, /^\s{4}name: validation-gate$/m);
  assert.match(ci, /^permissions:\n  actions: read\n  contents: read\n  pull-requests: read$/m);
  assert.match(ci, /^    timeout-minutes: 150$/m);
  assert.match(ci, /validate-local\.sh --fresh fast/);
  assert.match(ci, /github\.event_name == 'pull_request'/);
  assert.match(ci, /github\.event_name == 'push'/);
  assert.match(ci, /if: \$\{\{ !cancelled\(\) && \(github\.event_name == 'pull_request' \|\| steps\.decide\.outcome != 'success' \|\| steps\.decide\.outputs\.reuse != 'true'\) \}\}/);
  assert.match(ci, /steps\.discover\.outcome == 'success' && steps\.discover\.outputs\.found == 'true'/);
  assert.equal((ci.match(/^\s{8}continue-on-error: true$/gm) ?? []).length, 3);
  for (const step of ['Discover trusted PR evidence', 'Download trusted PR evidence', 'Verify trusted PR evidence']) {
    const block = ci.slice(ci.indexOf(`- name: ${step}`));
    assert.match(block.slice(0, block.indexOf('\n\n')), /continue-on-error: true/, step);
  }
  assert.doesNotMatch(ci, /if: github\.event_name == 'pull_request' \|\| steps\.decide\.outputs\.reuse/);
  assert.doesNotMatch(ci, /\.git\/deos-validation/);
  const reusable = readFileSync('.github/workflows/_validate.yml', 'utf8');
  assert.match(reusable, /validate-local\.sh --fresh "\$\{\{ inputs\.profile \}\}"/);
  const release = readFileSync('.github/workflows/release-candidate.yml', 'utf8');
  assert.match(release, /tags:\n\s+- "v\*"/);
  assert.match(release, /profile: full/);
  assert.doesNotMatch(release, /validation-ci-evidence|download-artifact/);
});
