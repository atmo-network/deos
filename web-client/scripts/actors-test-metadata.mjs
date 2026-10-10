/*
Domain: Actors client conformance inputs
Owns: One explicit metadata fixture for source preparation and committed binding checks.
Excludes: Metadata generation, runtime admission, and production artifact acceptance.
*/
import { readFile } from 'node:fs/promises';

export async function readActorsTestMetadata() {
  return new Uint8Array(
    await readFile(
      process.env.DEOS_ACTORS_TEST_METADATA_PATH ??
        new URL('../.papi/metadata/deos.scale', import.meta.url),
    ),
  );
}
