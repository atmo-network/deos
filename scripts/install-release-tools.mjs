#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { chmod, lstat, mkdir, readFile, readdir, rename, writeFile } from 'node:fs/promises';
import { accessSync, constants as fsConstants } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';

const TOOL_NAMES = Object.freeze(['chain-spec-builder', 'polkadot', 'polkadot-omni-node', 'zombienet']);
const SDK_SOURCE = 'https://github.com/paritytech/polkadot-sdk/tree/8ae9775dc43c0d8cdd0f6d87700596e14278b1e1';
const ZOMBIENET_SOURCE = 'https://github.com/paritytech/zombienet/tree/83dd3ab5a58ce833c8a6a99f91ce1a2c58f41136';
const SPDX_SCHEMA = Object.freeze({ version: '2.3', source: 'https://github.com/spdx/spdx-spec/blob/aadf3b0b8dbbabdb4d880b0fc714255fea436ff7/schemas/spdx-schema.json', url: 'https://raw.githubusercontent.com/spdx/spdx-spec/aadf3b0b8dbbabdb4d880b0fc714255fea436ff7/schemas/spdx-schema.json', sha256: '239208b7ac287b3cf5d9a9af23f9d69863971102a5e1587a27a398b43490b89b' });
function fail(message) { throw new Error(message); }
function exact(value, keys, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || JSON.stringify(Object.keys(value).sort()) !== JSON.stringify([...keys].sort())) fail(`${label} fields are invalid`);
}
function digest(bytes) { return createHash('sha256').update(bytes).digest('hex'); }
function firstLine(output) { return output.split('\n').map((line) => line.trim()).find(Boolean) ?? ''; }
function validateTool(tool) {
  const chain = tool?.name === 'chain-spec-builder';
  exact(tool, chain ? ['name', 'version', 'source', 'url', 'sha256', 'versionArgs', 'versionOutput', 'cargoPackage'] : ['name', 'version', 'source', 'url', 'sha256', 'versionArgs', 'versionOutput'], `Tool ${tool?.name ?? '(unknown)'}`);
  if (!TOOL_NAMES.includes(tool.name) || !/^[A-Za-z0-9.-]+$/.test(tool.version) || !/^[0-9a-f]{64}$/.test(tool.sha256) || !Array.isArray(tool.versionArgs) || !tool.versionArgs.every((arg) => /^[A-Za-z0-9._-]+$/.test(arg)) || typeof tool.versionOutput !== 'string' || !tool.versionOutput) fail(`Tool authority is invalid: ${tool.name}`);
  const sdk = tool.name !== 'zombienet';
  if (tool.source !== (sdk ? SDK_SOURCE : ZOMBIENET_SOURCE)) fail(`Tool source commit is invalid: ${tool.name}`);
  const expectedPrefix = sdk ? 'https://github.com/paritytech/polkadot-sdk/releases/download/polkadot-stable2606-1/' : 'https://github.com/paritytech/zombienet/releases/download/v1.3.138/';
  if (tool.url !== `${expectedPrefix}${tool.name === 'zombienet' ? 'zombienet-linux-x64' : tool.name}`) fail(`Tool release URL is invalid: ${tool.name}`);
  if (chain) {
    exact(tool.cargoPackage, ['name', 'version', 'source', 'checksum'], 'chain-spec-builder Cargo package');
    if (tool.cargoPackage.name !== 'staging-chain-spec-builder' || tool.cargoPackage.version !== '19.0.0' || tool.cargoPackage.version !== tool.version || tool.cargoPackage.source !== 'registry+https://github.com/rust-lang/crates.io-index' || tool.cargoPackage.checksum !== '2d0eaaa88ded7ea3d7257bbcd0e96bd70c030680ef2ca93874a21915d94af9fd') fail('chain-spec-builder Cargo source/version/checksum is incompatible with Cargo.lock');
  }
  return tool;
}
export function validateToolLock(lock) {
  exact(lock, ['schema', 'platform', 'tools', 'spdxSchema'], 'Tool lock');
  if (lock.schema !== 'deos-release-tools/v2' || lock.platform !== 'linux-x64' || !Array.isArray(lock.tools)) fail('Unsupported release tool lock');
  exact(lock.spdxSchema, ['version', 'source', 'url', 'sha256'], 'SPDX schema authority');
  if (JSON.stringify(lock.spdxSchema) !== JSON.stringify(SPDX_SCHEMA)) fail('SPDX schema authority is invalid or mutable');
  const names = lock.tools.map(validateTool).map((tool) => tool.name);
  if (JSON.stringify(names) !== JSON.stringify(TOOL_NAMES) || new Set(names).size !== names.length) fail(`Release tool inventory must be exactly: ${TOOL_NAMES.join(', ')}`);
  return lock;
}
async function download(url) {
  const response = await fetch(url, { redirect: 'follow', signal: AbortSignal.timeout(120_000) });
  if (!response.ok || !response.body) fail(`Tool download failed with HTTP ${response.status}: ${url}`);
  return Buffer.from(await response.arrayBuffer());
}
function verifyVersion(file, tool) {
  const result = spawnSync(file, tool.versionArgs, { encoding: 'utf8', timeout: 30_000 });
  const output = `${result.stdout || ''}\n${result.stderr || ''}`;
  if (result.error || result.status !== 0 || firstLine(output) !== tool.versionOutput) fail(`${tool.name} version output differs from exact lock: ${firstLine(output)}`);
}
async function verifyBinary(file, tool) {
  const info = await lstat(file);
  if (!info.isFile() || info.isSymbolicLink() || (info.mode & 0o111) === 0) fail(`Installed ${tool.name} is not an executable regular file`);
  const bytes = await readFile(file);
  if (digest(bytes) !== tool.sha256) fail(`Installed ${tool.name} digest differs from exact lock`);
  verifyVersion(file, tool);
}
function resolvePath(name, envPath) {
  for (const directory of envPath.split(path.delimiter)) {
    const candidate = path.join(directory, name);
    try { accessSync(candidate, fsConstants.X_OK); return path.resolve(candidate); } catch { /* Continue. */ }
  }
  return null;
}
export async function verifyInstalledTools(lock, bin, envPath = process.env.PATH ?? '') {
  validateToolLock(lock);
  const entries = (await readdir(bin)).sort();
  if (JSON.stringify(entries) !== JSON.stringify([...TOOL_NAMES].sort())) fail('Release binary directory contains extra or missing names');
  for (const tool of lock.tools) {
    const file = path.join(bin, tool.name);
    if (resolvePath(tool.name, envPath) !== path.resolve(file)) fail(`PATH does not select the locked ${tool.name}`);
    await verifyBinary(file, tool);
  }
}
async function install(lock, bin) {
  if (process.platform !== 'linux' || process.arch !== 'x64') fail('Release tools support only the locked linux-x64 runner');
  await mkdir(bin, { recursive: true, mode: 0o700 });
  const existing = await readdir(bin);
  if (existing.some((name) => !TOOL_NAMES.includes(name))) fail('Release binary directory contains an uncontrolled extra member');
  for (const tool of lock.tools) {
    const bytes = await download(tool.url);
    if (digest(bytes) !== tool.sha256) fail(`Downloaded ${tool.name} digest does not match immutable lock`);
    const temporary = path.join(bin, `.${tool.name}.${process.pid}.tmp`); const destination = path.join(bin, tool.name);
    await writeFile(temporary, bytes, { flag: 'wx', mode: 0o700 }); await chmod(temporary, 0o700); await rename(temporary, destination); await verifyBinary(destination, tool);
  }
}
function take(args, flag) { const index = args.indexOf(flag); if (index < 0 || !args[index + 1]) fail(`${flag} is required`); const value = args[index + 1]; args.splice(index, 2); return value; }
async function main(args) {
  if (args.includes('--help') || args.length === 0) { console.log('Usage: install-release-tools.mjs install|verify-path --lock FILE --bin DIRECTORY\n       install-release-tools.mjs install-schema --lock FILE --output FILE'); return; }
  const command = args.shift(); const lockPath = path.resolve(take(args, '--lock'));
  const lockBytes = await readFile(lockPath); const lock = validateToolLock(JSON.parse(lockBytes));
  if (command === 'install-schema') {
    const output = path.resolve(take(args, '--output'));
    if (args.length) fail(`Unknown arguments: ${args.join(' ')}`);
    const bytes = await download(lock.spdxSchema.url);
    if (digest(bytes) !== lock.spdxSchema.sha256) fail('Downloaded SPDX schema digest does not match immutable lock');
    JSON.parse(bytes); await writeFile(output, bytes, { flag: 'wx', mode: 0o600 });
  } else {
    const bin = path.resolve(take(args, '--bin'));
    if (args.length) fail(`Unknown arguments: ${args.join(' ')}`);
    if (command === 'install') await install(lock, bin);
    else if (command === 'verify-path') await verifyInstalledTools(lock, bin);
    else fail(`Unknown command: ${command}`);
  }
  console.log(`release-tool-lock-sha256=sha256:${digest(lockBytes)}`);
}
if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(new URL(import.meta.url).pathname)) main(process.argv.slice(2)).catch((error) => { console.error(`install-release-tools: ${error.message}`); process.exitCode = 1; });
