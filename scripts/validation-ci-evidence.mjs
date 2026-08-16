#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { constants as fsConstants } from 'node:fs';
import { lstat, open, readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  calculateIdentity, canonicalJson, gitCommonDirectory, readValidRecord, validateRecord,
} from './validation-evidence.mjs';

const CI_SCHEMA = 'deos-validation-ci-evidence/v1';
const DISCOVERY_SCHEMA = 'deos-validation-ci-discovery/v1';
const WORKFLOW_PATH = '.github/workflows/ci.yml';
const PROFILE = 'fast';
const JOB_NAME = 'validation-gate';
const API_VERSION = '2022-11-28';
const DEFAULT_API_REQUEST_TIMEOUT_MS = 15_000;
const DEFAULT_DISCOVERY_TIMEOUT_MS = 90_000;
const MAX_API_REQUEST_TIMEOUT_MS = 60_000;
const MAX_DISCOVERY_TIMEOUT_MS = 5 * 60_000;
const MAX_ARTIFACT_BYTES = 8 * 1024 * 1024;
const MAX_GITHUB_OUTPUT_BYTES = 1024 * 1024;

function fail(message) { throw new Error(message); }
function isString(value) { return typeof value === 'string' && value.length > 0; }
function isInteger(value) { return Number.isSafeInteger(value) && value > 0; }
function isOid(value) { return typeof value === 'string' && /^[0-9a-f]{40}([0-9a-f]{24})?$/.test(value); }
function isSha256(value) { return typeof value === 'string' && /^sha256:[0-9a-f]{64}$/.test(value); }
function sha256(bytes) { return `sha256:${createHash('sha256').update(bytes).digest('hex')}`; }
function exactKeys(value, keys, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) fail(`${label} must be an object`);
  if (canonicalJson(Object.keys(value).sort()) !== canonicalJson([...keys].sort())) fail(`${label} fields are invalid`);
}
function repositoryName(value, label) {
  if (!isString(value) || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(value)) fail(`${label} is invalid`);
  return value;
}
function positiveInteger(value, label) {
  const parsed = typeof value === 'string' && /^(0|[1-9][0-9]*)$/.test(value) ? Number(value) : value;
  if (!isInteger(parsed)) fail(`${label} must be a positive safe integer`);
  return parsed;
}
function git(repo, args, encoding = 'utf8') {
  const result = spawnSync('git', ['-C', repo, ...args], { encoding, maxBuffer: 16 * 1024 * 1024 });
  if (result.error) throw result.error;
  if (result.status !== 0) fail(`git ${args.join(' ')} failed: ${String(result.stderr).trim()}`);
  return result.stdout;
}
function candidateWorkflowAuthority(repo, treeOid, workflowPath = WORKFLOW_PATH) {
  const bytes = git(repo, ['show', `${treeOid}:${workflowPath}`], 'buffer');
  return { sha256: sha256(bytes) };
}
function artifactName(treeOid, runId, runAttempt) {
  return `deos-validation-${PROFILE}-${treeOid}-${runId}-${runAttempt}`;
}
function semanticRecordFields(record) {
  return {
    candidate: record.candidate,
    authority: record.authority,
    environment: record.environment,
    refIdentity: record.refIdentity,
    freshness: record.freshness,
    invocation: record.invocation,
    repetition: record.repetition,
  };
}
function identitySemanticFields(identity) {
  return {
    candidate: identity.candidate,
    authority: identity.authority,
    environment: identity.environment,
    refIdentity: identity.refIdentity,
    freshness: identity.freshness,
    invocation: identity.invocation,
    repetition: identity.repetition,
  };
}

export function validateCiEnvelope(envelope) {
  exactKeys(envelope, ['schema', 'localEvidence', 'workflow'], 'CI evidence');
  if (envelope.schema !== CI_SCHEMA) fail('Unsupported CI evidence schema');
  validateRecord(envelope.localEvidence);
  const workflow = envelope.workflow;
  exactKeys(workflow, [
    'repository', 'repositoryId', 'workflowPath', 'workflowAuthority', 'event', 'runId',
    'runAttempt', 'job', 'headSha', 'headRepository', 'headRepositoryId', 'baseSha',
    'artifactName', 'conclusion',
  ], 'CI workflow evidence');
  repositoryName(workflow.repository, 'Workflow repository');
  positiveInteger(workflow.repositoryId, 'Workflow repository id');
  if (workflow.workflowPath !== WORKFLOW_PATH || workflow.event !== 'pull_request' || workflow.job !== JOB_NAME || workflow.conclusion !== 'success') fail('Workflow authority/event/job/conclusion is invalid');
  exactKeys(workflow.workflowAuthority, ['commit', 'sha256'], 'Workflow authority');
  if (!isOid(workflow.workflowAuthority.commit) || !isSha256(workflow.workflowAuthority.sha256)) fail('Workflow authority identity is invalid');
  positiveInteger(workflow.runId, 'Workflow run id');
  positiveInteger(workflow.runAttempt, 'Workflow run attempt');
  if (!isOid(workflow.headSha) || !isOid(workflow.baseSha) || workflow.workflowAuthority.commit !== workflow.baseSha) fail('Workflow head/base SHA is invalid');
  repositoryName(workflow.headRepository, 'Workflow head repository');
  positiveInteger(workflow.headRepositoryId, 'Workflow head repository id');
  if (workflow.artifactName !== artifactName(envelope.localEvidence.candidate.treeOid, workflow.runId, workflow.runAttempt)) fail('Workflow artifact name is invalid');
  return envelope;
}

export async function createCiEnvelope(options) {
  const repo = path.resolve(options.repo);
  const identity = await calculateIdentity(repo, PROFILE);
  const common = await gitCommonDirectory(repo);
  const recordPath = path.join(common, 'deos-validation', 'v2', 'records', `${identity.key.slice(7)}.json`);
  const record = await readValidRecord(recordPath, identity.key);
  if (!record) fail('Fresh successful fast validation evidence is unavailable');
  const workflowAuthority = candidateWorkflowAuthority(repo, identity.candidate.treeOid, options.workflowPath);
  const workflow = {
    repository: repositoryName(options.repository, 'Repository'),
    repositoryId: positiveInteger(options.repositoryId, 'Repository id'),
    workflowPath: options.workflowPath,
    workflowAuthority: { commit: options.baseSha, sha256: workflowAuthority.sha256 },
    event: options.event,
    runId: positiveInteger(options.runId, 'Run id'),
    runAttempt: positiveInteger(options.runAttempt, 'Run attempt'),
    job: options.job,
    headSha: options.headSha,
    headRepository: repositoryName(options.headRepository, 'Head repository'),
    headRepositoryId: positiveInteger(options.headRepositoryId, 'Head repository id'),
    baseSha: options.baseSha,
    artifactName: artifactName(identity.candidate.treeOid, positiveInteger(options.runId, 'Run id'), positiveInteger(options.runAttempt, 'Run attempt')),
    conclusion: 'success',
  };
  return validateCiEnvelope({ schema: CI_SCHEMA, localEvidence: record, workflow });
}

function decodeContentsResponse(value, label) {
  if (!value || value.type !== 'file' || value.encoding !== 'base64' || !isString(value.content)) fail(`${label} workflow contents are unavailable`);
  return Buffer.from(value.content.replace(/\n/g, ''), 'base64');
}
function exactArtifactPattern(runId, attempt) {
  return new RegExp(`^deos-validation-${PROFILE}-[0-9a-f]{40}(?:[0-9a-f]{24})?-${runId}-${attempt}$`);
}
function runArtifactPattern(runId) {
  return new RegExp(`^deos-validation-${PROFILE}-[0-9a-f]{40}(?:[0-9a-f]{24})?-${runId}-[1-9][0-9]*$`);
}
function completePage(values) {
  return Array.isArray(values) && values.length < 100;
}

export async function discoverTrustedArtifact(options) {
  const repository = repositoryName(options.repository, 'Repository');
  const repositoryId = positiveInteger(options.repositoryId, 'Repository id');
  const mainSha = options.mainSha;
  if (!isOid(mainSha)) fail('Main SHA is invalid');
  const api = options.api;
  const pulls = await api(`/repos/${repository}/commits/${mainSha}/pulls?per_page=100`);
  if (!completePage(pulls)) fail('Associated pull request response is invalid or ambiguous');
  for (const pull of pulls) {
    if (!pull?.merged_at || pull?.base?.repo?.full_name !== repository || pull?.base?.repo?.id !== repositoryId) continue;
    if (pull?.head?.repo?.full_name !== repository || pull?.head?.repo?.id !== repositoryId || !isOid(pull?.head?.sha) || !isOid(pull?.base?.sha)) continue;
    const query = new URLSearchParams({ event: 'pull_request', status: 'completed', head_sha: pull.head.sha, per_page: '100' });
    const workflowId = encodeURIComponent(path.posix.basename(WORKFLOW_PATH));
    const runsResult = await api(`/repos/${repository}/actions/workflows/${workflowId}/runs?${query}`);
    const runs = Array.isArray(runsResult?.workflow_runs) ? runsResult.workflow_runs : [];
    if (!completePage(runs)) continue;
    for (const run of runs.sort((a, b) => Number(b.id) - Number(a.id))) {
      if (runs.filter((candidate) => candidate?.id === run?.id).length !== 1) continue;
      if (!isInteger(run?.id) || !isInteger(run?.run_attempt) || run?.repository?.full_name !== repository || run?.repository?.id !== repositoryId) continue;
      if (run?.path !== WORKFLOW_PATH || run?.event !== 'pull_request' || run?.status !== 'completed' || run?.conclusion !== 'success') continue;
      if (run?.head_sha !== pull.head.sha || run?.head_repository?.full_name !== repository || run?.head_repository?.id !== repositoryId) continue;
      const [baseContents, headContents, jobsResult, artifactsResult] = await Promise.all([
        api(`/repos/${repository}/contents/${WORKFLOW_PATH}?ref=${pull.base.sha}`),
        api(`/repos/${repository}/contents/${WORKFLOW_PATH}?ref=${pull.head.sha}`),
        api(`/repos/${repository}/actions/runs/${run.id}/attempts/${run.run_attempt}/jobs?per_page=100`),
        api(`/repos/${repository}/actions/runs/${run.id}/artifacts?per_page=100`),
      ]);
      const baseAuthority = sha256(decodeContentsResponse(baseContents, 'Base'));
      const headAuthority = sha256(decodeContentsResponse(headContents, 'Head'));
      if (baseAuthority !== headAuthority) continue;
      const jobs = Array.isArray(jobsResult?.jobs) ? jobsResult.jobs : [];
      if (!completePage(jobs)) continue;
      const namedJobs = jobs.filter((job) => job?.name === JOB_NAME);
      const matchingJobs = namedJobs.filter((job) => job?.status === 'completed' && job?.conclusion === 'success' && job?.run_attempt === run.run_attempt && job?.head_sha === pull.head.sha);
      if (namedJobs.length !== 1 || matchingJobs.length !== 1 || !isInteger(matchingJobs[0]?.id)) continue;
      const artifacts = Array.isArray(artifactsResult?.artifacts) ? artifactsResult.artifacts : [];
      if (!completePage(artifacts)) continue;
      const runArtifacts = artifacts.filter((artifact) => runArtifactPattern(run.id).test(artifact?.name ?? ''));
      const matchingArtifacts = runArtifacts.filter((artifact) => exactArtifactPattern(run.id, run.run_attempt).test(artifact.name));
      if (runArtifacts.length !== 1 || matchingArtifacts.length !== 1) continue;
      const artifact = matchingArtifacts[0];
      if (!isInteger(artifact?.id) || artifact?.expired !== false || artifact?.workflow_run?.id !== run.id || artifact?.workflow_run?.repository_id !== repositoryId || artifact?.workflow_run?.head_repository_id !== repositoryId || artifact?.workflow_run?.head_sha !== pull.head.sha) continue;
      return {
        schema: DISCOVERY_SCHEMA,
        repository,
        repositoryId,
        workflowPath: WORKFLOW_PATH,
        workflowAuthority: { commit: pull.base.sha, sha256: baseAuthority },
        event: 'pull_request',
        runId: run.id,
        runAttempt: run.run_attempt,
        job: JOB_NAME,
        jobId: matchingJobs[0].id,
        headSha: pull.head.sha,
        headRepository: repository,
        headRepositoryId: repositoryId,
        baseSha: pull.base.sha,
        artifact: { id: artifact.id, name: artifact.name, expired: false },
        runConclusion: 'success',
        jobConclusion: 'success',
      };
    }
  }
  return null;
}

export async function discoverTrustedArtifactOrNull(options, onError = () => {}) {
  try { return await discoverTrustedArtifact(options); }
  catch (error) { onError(error); return null; }
}

function validateDiscovery(value) {
  exactKeys(value, [
    'schema', 'repository', 'repositoryId', 'workflowPath', 'workflowAuthority', 'event',
    'runId', 'runAttempt', 'job', 'jobId', 'headSha', 'headRepository',
    'headRepositoryId', 'baseSha', 'artifact', 'runConclusion', 'jobConclusion',
  ], 'CI discovery');
  if (value.schema !== DISCOVERY_SCHEMA || value.workflowPath !== WORKFLOW_PATH || value.event !== 'pull_request' || value.job !== JOB_NAME || value.runConclusion !== 'success' || value.jobConclusion !== 'success') fail('CI discovery authority is invalid');
  repositoryName(value.repository, 'Discovery repository');
  repositoryName(value.headRepository, 'Discovery head repository');
  for (const [item, label] of [[value.repositoryId, 'repository id'], [value.runId, 'run id'], [value.runAttempt, 'run attempt'], [value.jobId, 'job id'], [value.headRepositoryId, 'head repository id']]) positiveInteger(item, `Discovery ${label}`);
  if (!isOid(value.headSha) || !isOid(value.baseSha)) fail('Discovery SHA is invalid');
  exactKeys(value.workflowAuthority, ['commit', 'sha256'], 'Discovery workflow authority');
  if (value.workflowAuthority.commit !== value.baseSha || !isSha256(value.workflowAuthority.sha256)) fail('Discovery workflow authority is invalid');
  exactKeys(value.artifact, ['id', 'name', 'expired'], 'Discovery artifact');
  positiveInteger(value.artifact.id, 'Discovery artifact id');
  if (!isString(value.artifact.name) || value.artifact.expired !== false) fail('Discovery artifact is invalid');
  return value;
}

export function verifyCiEvidence({ envelope, discovery, expectedIdentity, repository, repositoryId, workflowSha256 }) {
  validateCiEnvelope(envelope);
  validateDiscovery(discovery);
  const expectedRepository = repositoryName(repository, 'Expected repository');
  const expectedRepositoryId = positiveInteger(repositoryId, 'Expected repository id');
  if (!isSha256(workflowSha256)) fail('Expected workflow authority is invalid');
  if (discovery.repository !== expectedRepository || discovery.repositoryId !== expectedRepositoryId || discovery.headRepository !== expectedRepository || discovery.headRepositoryId !== expectedRepositoryId) fail('API repository provenance mismatch');
  if (discovery.workflowAuthority.sha256 !== workflowSha256) fail('Workflow authority changed');
  const workflow = envelope.workflow;
  const apiWorkflow = {
    repository: discovery.repository,
    repositoryId: discovery.repositoryId,
    workflowPath: discovery.workflowPath,
    workflowAuthority: discovery.workflowAuthority,
    event: discovery.event,
    runId: discovery.runId,
    runAttempt: discovery.runAttempt,
    job: discovery.job,
    headSha: discovery.headSha,
    headRepository: discovery.headRepository,
    headRepositoryId: discovery.headRepositoryId,
    baseSha: discovery.baseSha,
    artifactName: discovery.artifact.name,
    conclusion: 'success',
  };
  if (canonicalJson(workflow) !== canonicalJson(apiWorkflow)) fail('Artifact workflow claims do not match GitHub API provenance');
  validateRecord(envelope.localEvidence, expectedIdentity.key);
  if (canonicalJson(semanticRecordFields(envelope.localEvidence)) !== canonicalJson(identitySemanticFields(expectedIdentity))) fail('Local semantic evidence does not match the main candidate');
  return true;
}

function boundedMilliseconds(value, fallback, maximum, label) {
  if (value === undefined) return fallback;
  const parsed = positiveInteger(value, label);
  if (parsed > maximum) fail(`${label} exceeds ${maximum}ms`);
  return parsed;
}

export class GitHubApi {
  constructor(token, baseUrl = 'https://api.github.com', options = {}) {
    this.token = token;
    this.baseUrl = baseUrl.replace(/\/$/, '');
    this.requestTimeoutMs = boundedMilliseconds(options.requestTimeoutMs, DEFAULT_API_REQUEST_TIMEOUT_MS, MAX_API_REQUEST_TIMEOUT_MS, 'API request timeout');
    const discoveryTimeoutMs = boundedMilliseconds(options.discoveryTimeoutMs, DEFAULT_DISCOVERY_TIMEOUT_MS, MAX_DISCOVERY_TIMEOUT_MS, 'API discovery timeout');
    this.deadline = Date.now() + discoveryTimeoutMs;
    this.fetch = options.fetch ?? fetch;
  }

  async get(endpoint) {
    const remaining = this.deadline - Date.now();
    if (remaining <= 0) fail('GitHub API discovery timeout expired');
    const timeoutMs = Math.min(this.requestTimeoutMs, remaining);
    const controller = new AbortController();
    let timer;
    const timeout = new Promise((_, reject) => {
      timer = setTimeout(() => {
        controller.abort();
        reject(new Error(`GitHub API request timed out after ${timeoutMs}ms for ${endpoint}`));
      }, timeoutMs);
    });
    try {
      return await Promise.race([(async () => {
        const response = await this.fetch(`${this.baseUrl}${endpoint}`, {
          signal: controller.signal,
          headers: { Accept: 'application/vnd.github+json', Authorization: `Bearer ${this.token}`, 'X-GitHub-Api-Version': API_VERSION, 'User-Agent': 'deos-validation-evidence' },
        });
        if (!response.ok) fail(`GitHub API ${response.status} for ${endpoint}`);
        return response.json();
      })(), timeout]);
    } finally {
      clearTimeout(timer);
    }
  }
}

export async function appendOutput(file, entries) {
  if (!file) return;
  const pairs = Object.entries(entries);
  if (pairs.length === 0 || pairs.length > 8) fail('GitHub output entry count is invalid');
  const lines = pairs.map(([key, rawValue]) => {
    const value = String(rawValue);
    if (!/^[a-z][a-z0-9-]{0,63}$/.test(key) || value.length === 0 || value.length > 1024 || /[\r\n\0]/.test(value)) fail('GitHub output entry is unsafe');
    return `${key}=${value}`;
  });
  const payload = `${lines.join('\n')}\n`;
  if (Buffer.byteLength(payload) > 4096) fail('GitHub output payload is oversized');
  const handle = await open(file, fsConstants.O_WRONLY | fsConstants.O_APPEND | fsConstants.O_CREAT | fsConstants.O_NOFOLLOW, 0o600);
  try {
    const info = await handle.stat();
    if (!info.isFile() || info.size > MAX_GITHUB_OUTPUT_BYTES - Buffer.byteLength(payload)) fail('GitHub output file is invalid or oversized');
    await handle.writeFile(payload);
  } finally {
    await handle.close();
  }
}

export async function readSingleArtifact(directory) {
  const resolvedDirectory = path.resolve(directory);
  const root = await lstat(resolvedDirectory);
  if (!root.isDirectory() || root.isSymbolicLink()) fail('Downloaded artifact root must be a regular directory');
  const entries = await readdir(resolvedDirectory, { withFileTypes: true });
  if (entries.length !== 1 || entries[0].name !== 'evidence.json' || !entries[0].isFile() || entries[0].isSymbolicLink()) fail('Downloaded artifact must contain exactly one regular evidence.json');
  const target = path.join(resolvedDirectory, entries[0].name);
  if (path.dirname(target) !== resolvedDirectory || path.basename(target) !== 'evidence.json') fail('Downloaded evidence path is invalid');
  const handle = await open(target, fsConstants.O_RDONLY | fsConstants.O_NOFOLLOW);
  try {
    const info = await handle.stat();
    if (!info.isFile() || info.size <= 0 || info.size > MAX_ARTIFACT_BYTES) fail('Downloaded evidence file is invalid');
    const bytes = await handle.readFile();
    if (bytes.length !== info.size || bytes.length > MAX_ARTIFACT_BYTES) fail('Downloaded evidence file changed or was truncated');
    return JSON.parse(bytes.toString('utf8'));
  } finally {
    await handle.close();
  }
}
function parseOptions(args) {
  const options = {};
  while (args.length) {
    const name = args.shift();
    if (!name.startsWith('--') || args.length === 0) fail(`Invalid argument: ${name}`);
    options[name.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())] = args.shift();
  }
  return options;
}
function usage() {
  console.log('Usage: validation-ci-evidence.mjs export --repo PATH --output FILE [workflow identity options]\n       validation-ci-evidence.mjs discover --repository OWNER/REPO --repository-id ID --main-sha SHA --output FILE --github-output FILE\n       validation-ci-evidence.mjs decide --repo PATH --repository OWNER/REPO --repository-id ID --discovery FILE --artifact-directory DIR --github-output FILE');
}

async function main(argv) {
  const command = argv.shift();
  if (['-h', '--help', undefined].includes(command)) { usage(); return; }
  const options = parseOptions(argv);
  if (command === 'export') {
    const envelope = await createCiEnvelope(options);
    await writeFile(options.output, `${JSON.stringify(envelope, null, 2)}\n`, { mode: 0o600 });
    await appendOutput(options.githubOutput, { 'artifact-name': envelope.workflow.artifactName });
    return;
  }
  if (command === 'discover') {
    let discovery = null;
    try {
      const api = new GitHubApi(options.token ?? process.env.GITHUB_TOKEN, options.apiUrl ?? process.env.GITHUB_API_URL, {
        requestTimeoutMs: options.apiRequestTimeoutMs,
        discoveryTimeoutMs: options.apiDiscoveryTimeoutMs,
      });
      discovery = await discoverTrustedArtifactOrNull(
        { ...options, api: (endpoint) => api.get(endpoint) },
        (error) => console.error(`[WARNING] Trusted PR evidence unavailable: ${error.message}`),
      );
      if (discovery) {
        validateDiscovery(discovery);
        await writeFile(options.output, `${JSON.stringify(discovery, null, 2)}\n`, { mode: 0o600 });
      }
    } catch (error) {
      discovery = null;
      console.error(`[WARNING] Trusted PR evidence unavailable: ${error.message}`);
    }
    await appendOutput(options.githubOutput, discovery
      ? { found: 'true', 'artifact-id': discovery.artifact.id, 'run-id': discovery.runId }
      : { found: 'false' });
    return;
  }
  if (command === 'decide') {
    let reuse = false;
    try {
      const repo = path.resolve(options.repo);
      const discovery = JSON.parse(await readFile(options.discovery, 'utf8'));
      const envelope = await readSingleArtifact(options.artifactDirectory);
      const expected = await calculateIdentity(repo, PROFILE);
      const workflowAuthority = candidateWorkflowAuthority(repo, expected.candidate.treeOid);
      verifyCiEvidence({ envelope, discovery, expectedIdentity: expected, repository: options.repository, repositoryId: options.repositoryId, workflowSha256: workflowAuthority.sha256 });
      const finalIdentity = await calculateIdentity(repo, PROFILE);
      if (canonicalJson(finalIdentity) !== canonicalJson(expected)) fail('Main validation identity changed during CI evidence verification');
      reuse = true;
      console.error(`[SUCCESS] Reused trusted PR fast evidence ${expected.key}`);
    } catch (error) {
      console.error(`[WARNING] Trusted PR evidence rejected: ${error.message}`);
    }
    await appendOutput(options.githubOutput, { reuse: String(reuse) });
    return;
  }
  fail(`Unknown command: ${command}`);
}

const isEntrypoint = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isEntrypoint) main(process.argv.slice(2)).catch((error) => { console.error(`[ERROR] ${error.message}`); process.exitCode = 1; });
