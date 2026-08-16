#!/usr/bin/env node

import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import {
  parseFrontmatter,
  splitFrontmatter,
  stringifyFrontmatter,
} from "./okf-frontmatter.mjs";

function usage() {
  console.log(`Usage: migrate-okf-wiki.mjs [--wiki-dir PATH] [--write]

Deterministically project legacy DEOS wiki frontmatter onto the canonical OKF
v0.2 fields. Without --write, fail when any page is not normalized.`);
}

let wikiDir = resolve(process.cwd(), "wiki");
let write = false;
for (let index = 2; index < process.argv.length; index += 1) {
  const argument = process.argv[index];
  if (argument === "--write") write = true;
  else if (argument === "-h" || argument === "--help") {
    usage();
    process.exit(0);
  } else if (argument === "--wiki-dir")
    wikiDir = resolve(process.argv[++index] ?? "");
  else if (argument.startsWith("--wiki-dir="))
    wikiDir = resolve(argument.slice("--wiki-dir=".length));
  else throw new Error(`Unknown argument: ${argument}`);
}

const paths = [];
function walk(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) walk(path);
    else if (entry.isFile() && /\.(en|ru)\.md$/.test(entry.name))
      paths.push(path);
  }
}
walk(wikiDir);

function normalizeMetadata(meta, path) {
  if ("page_type" in meta && "type" in meta)
    throw new Error(`${path}: both page_type and type are present`);
  if ("summary" in meta && "description" in meta)
    throw new Error(`${path}: both summary and description are present`);
  let changed = false;
  const normalized = {};
  for (const [key, value] of Object.entries(meta)) {
    const canonicalKey =
      key === "page_type" ? "type" : key === "summary" ? "description" : key;
    if (canonicalKey !== key) changed = true;
    if (canonicalKey === "status" && value === "active") {
      normalized[canonicalKey] = "stable";
      changed = true;
    } else if (canonicalKey === "sources" && Array.isArray(value)) {
      normalized[canonicalKey] = value.map((source) => {
        if (typeof source !== "string") return source;
        changed = true;
        return { resource: source };
      });
    } else normalized[canonicalKey] = value;
  }
  return { changed, normalized };
}

let changed = 0;
for (const path of paths.sort()) {
  const original = readFileSync(path, "utf8");
  const { frontmatter, body } = splitFrontmatter(original, path);
  const meta = parseFrontmatter(frontmatter, path);
  const result = normalizeMetadata(meta, path);
  if (!result.changed) continue;
  changed += 1;
  if (write) {
    const normalized = `---\n${stringifyFrontmatter(result.normalized)}\n---\n${body}`;
    writeFileSync(path, normalized);
  } else console.error(`[DRIFT] ${relative(wikiDir, path)}`);
}

if (!write && changed) process.exit(1);
console.log(
  `${write ? "Migrated" : "Canonical OKF projection confirmed for"} ${paths.length} localized concepts${write ? ` (${changed} changed)` : ""}.`,
);
