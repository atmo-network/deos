/*
Domain: AAA accepted specification identity
Owns: Fail-closed binding of the 0.7.12 normative specification bytes.
Excludes: Implementation conformance, metadata identity, and release evidence approval.
Zone: Release validation entrypoint; update only through an explicit specification acceptance.
*/
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ACCEPTED_SPEC_SHA256 =
  '839dbf89c4cd94ff6059133e9a6b2737005cf66ab8e8f462263d18ab828f281a';
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const specPath = path.resolve(
  scriptDir,
  '../../template/pallets/aaa/docs/specification.en.md',
);
const actual = createHash('sha256')
  .update(await readFile(specPath))
  .digest('hex');
if (actual !== ACCEPTED_SPEC_SHA256) {
  console.error(
    `AAA accepted specification hash drift: expected=${ACCEPTED_SPEC_SHA256} actual=${actual}`,
  );
  process.exit(1);
}
console.log(`AAA accepted specification hash passed: ${actual}`);
