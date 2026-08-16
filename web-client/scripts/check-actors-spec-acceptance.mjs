/*
Domain: Actors accepted specification identity
Owns: Fail-closed binding of the accepted normative specification bytes.
Excludes: Implementation conformance, metadata identity, and release approval.
Zone: Release validation entrypoint; update only through an explicit specification acceptance.
*/
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ACCEPTED_SPEC_SHA256 =
  '723eeee9281bf315375e41d3df8c6cadddf743852da237fc4296f1e62b75f9d1';
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const specPath = path.resolve(
  scriptDir,
  '../../template/pallets/actors/docs/specification.en.md',
);
const actual = createHash('sha256')
  .update(await readFile(specPath))
  .digest('hex');
if (actual !== ACCEPTED_SPEC_SHA256) {
  console.error(
    `Actors accepted specification hash drift: expected=${ACCEPTED_SPEC_SHA256} actual=${actual}`,
  );
  process.exit(1);
}
console.log(`Actors accepted specification hash passed: ${actual}`);
