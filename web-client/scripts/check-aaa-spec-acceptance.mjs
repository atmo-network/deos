/*
Domain: AAA accepted specification identity
Owns: Fail-closed binding of the 0.7.11 normative specification bytes.
Excludes: Implementation conformance, metadata identity, and release evidence approval.
Zone: Release validation entrypoint; update only through an explicit specification acceptance.
*/
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ACCEPTED_SPEC_SHA256 =
  '1bb87c6338d746ca7ab268fa72e2154bbd5fda2e51ed8d245dff955140cb6852';
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
