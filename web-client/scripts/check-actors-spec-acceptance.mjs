/*
Domain: Actors accepted specification identity
Owns: Fail-closed binding of the 0.7.12 normative specification bytes.
Excludes: Implementation conformance, metadata identity, and release evidence approval.
Zone: Release validation entrypoint; update only through an explicit specification acceptance.
*/
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ACCEPTED_SPEC_SHA256 =
  '6f74c0b70d8fda08c141c27a45b3c243cc80e77cfecdcf611279c1c2961c8da6';
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
