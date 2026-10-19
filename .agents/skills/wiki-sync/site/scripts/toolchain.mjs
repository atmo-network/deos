#!/usr/bin/env node
/*
Domain: mdBook tool identity
Owns: Pinned release metadata and exact platform/artifact selection.
Excludes: Downloads, process control, content compilation and hosting.
*/
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

export const mdbook = JSON.parse(readFileSync(new URL('../mdbook.json', import.meta.url), 'utf8'));
export function artifactFor(platform) {
  const artifact = mdbook.artifacts[platform];
  if (!artifact || !/^[a-f0-9]{64}$/.test(artifact.sha256) || !artifact.file.startsWith(`mdbook-v${mdbook.version}-`) || !/^mdbook-v[\d.]+-[a-z0-9_-]+\.tar\.gz$/.test(artifact.file)) throw new Error(`Unsupported mdBook platform or invalid pin: ${platform}`);
  return { ...artifact, url: `https://github.com/rust-lang/mdBook/releases/download/v${mdbook.version}/${artifact.file}` };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [mode, platform] = process.argv.slice(2);
  if (mode === '--help' || mode === '-h') console.log('Usage: toolchain.mjs --version | --url PLATFORM | --sha256 PLATFORM');
  else if (mode === '--version') console.log(mdbook.version);
  else if (mode === '--url') console.log(artifactFor(platform).url);
  else if (mode === '--sha256') console.log(artifactFor(platform).sha256);
  else throw new Error('Unknown mdBook identity query');
}
