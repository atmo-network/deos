/*
Domain: Trusted native search fixtures
Owns: Loading pinned/generated Elasticlunr assets for isolated integration assertions.
Excludes: Untrusted-script sandboxing, replacement engines and production mutation.
*/
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { createContext, runInContext } from 'node:vm';

export function nativeLibrary(directory) {
  const assets = readdirSync(directory).filter(name => /^elasticlunr-[a-f0-9]{8}\.min\.js$/.test(name));
  assert.equal(assets.length, 1);
  return readFileSync(join(directory, assets[0]), 'utf8');
}

export function loadNative(library, script) {
  // Trusted pinned/generated inputs only; VM isolation is not a security sandbox.
  const context = createContext({ console, window: { search: {} } });
  runInContext(library, context, { timeout: 5000 });
  runInContext(script, context, { timeout: 5000 });
  const engine = context.elasticlunr ?? context.window.elasticlunr;
  return { data: JSON.parse(JSON.stringify(context.window.search)), index: engine.Index.load(context.window.search.index) };
}
