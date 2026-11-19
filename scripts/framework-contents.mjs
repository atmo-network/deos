import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { lstatSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const [root, version, commit, tree] = process.argv.slice(2);
assert.equal(process.argv.length, 6, 'Usage: framework-contents.mjs ROOT VERSION COMMIT TREE');
assert.match(version, /^\d+\.\d+\.\d+$/);
for (const id of [commit, tree]) assert.match(id, /^(?:[a-f0-9]{40}|[a-f0-9]{64})$/);

const files = [];
function walk(prefix) {
  for (const name of readdirSync(join(root, prefix)).sort()) {
    const path = prefix ? `${prefix}/${name}` : name;
    const stat = lstatSync(join(root, path));
    assert.ok(!['web-client', 'wiki', 'bin', '.agents', 'template/research', 'template/chain_spec.json',
      'template/zombienet.toml'].includes(path), `Optional or generated path: ${path}`);
    assert.ok(!['.git', 'node_modules', '.svelte-kit'].includes(name) &&
      !(name === 'target' && (prefix === 'template' || readdirSync(join(root, prefix)).includes('Cargo.toml'))),
    `Build or checkout state: ${path}`);
    if (stat.isDirectory()) walk(path);
    else {
      assert.ok(stat.isFile(), `Not a regular release input: ${path}`);
      const bytes = readFileSync(join(root, path));
      files.push({ path, mode: (stat.mode & 0o111) ? '100755' : '100644', size: bytes.length,
        sha256: createHash('sha256').update(bytes).digest('hex') });
    }
  }
}
walk('');
const paths = new Set(files.map(file => file.path));
for (const path of ['LICENSE', 'README.md', 'CHANGELOG.md', 'docs/deos/framework-distribution.en.md',
  '.node-version', 'template/Cargo.toml', 'template/Cargo.lock', 'template/rust-toolchain.toml',
  'template/tools/pvf-validation/Cargo.lock', 'template/vendor/pallet-assets/LICENSE-APACHE',
  'template/vendor/str0m-openssl/LICENSE-MIT.txt',
  'notices/README.md', 'notices/SHA256SUMS', 'notices/mit-23f18e03dc49.txt', 'notices/unicode-68f5b9f5ea36.txt',
  'notices/unicode-f367c1b8e1aa.txt',
  'scripts/validate-local.sh', 'scripts/export-papi-metadata.sh', 'scripts/pvf-assurance.sh',
  'simulators/tmctol/tests.js']) assert.ok(paths.has(path), `Missing release input: ${path}`);
console.log(JSON.stringify({ schema: 1, version, profile: 'framework-source', source: { commit, tree }, files }, null, 2));
