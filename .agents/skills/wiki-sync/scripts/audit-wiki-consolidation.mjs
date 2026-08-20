#!/usr/bin/env node

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { readConcept } from "./okf-frontmatter.mjs";

const [wikiArgument = "wiki", minLinesArgument = "18"] = process.argv.slice(2);
const wikiDir = resolve(wikiArgument);
const minimumBodyLines = Number(minLinesArgument);
const failures = [];
const warnings = [];
const pages = new Map();
const localesById = new Map();

const paths = [];
function visit(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) visit(path);
    else if (entry.isFile() && /\.(en|ru)\.md$/.test(entry.name))
      paths.push(path);
  }
}
visit(wikiDir);

for (const path of paths.sort()) {
  const rel = relative(wikiDir, path).replaceAll("\\", "/");
  try {
    const { body, meta } = readConcept(path);
    for (const field of [
      "type",
      "title",
      "description",
      "locale",
      "canonical_page_id",
    ]) {
      if (meta[field] === undefined || meta[field] === "")
        failures.push(`${rel}: missing required metadata ${field}`);
    }
    if (meta.last_compiled !== undefined)
      failures.push(`${rel}: unsupported freshness signal last_compiled`);
    if (meta.confidence !== undefined)
      failures.push(`${rel}: unsupported subjective signal confidence`);

    const id = meta.canonical_page_id;
    const locale = meta.locale;
    if (!id || !locale) continue;
    const key = `${id}/${locale}`;
    if (pages.has(key)) failures.push(`${rel}: duplicate page identity ${key}`);
    pages.set(key, rel);
    if (!localesById.has(id)) localesById.set(id, new Set());
    localesById.get(id).add(locale);
    if (!Array.isArray(meta.sources) || meta.sources.length === 0)
      failures.push(`${rel}: missing structured provenance`);
    if (id !== "index" && !Array.isArray(meta.related))
      failures.push(`${rel}: missing related block`);

    for (const source of meta.sources ?? []) {
      const resource = source?.resource;
      if (typeof resource !== "string") continue;
      const sourcePath = isAbsolute(resource)
        ? resource
        : resolve(dirname(path), resource);
      if (!existsSync(sourcePath))
        failures.push(`${rel}: missing source resource ${resource}`);
    }

    const bodyLines = body.split("\n").filter((line) => line.trim()).length;
    if (id !== "index" && bodyLines < minimumBodyLines)
      warnings.push(`${rel}: short-page candidate (${bodyLines} body lines)`);
  } catch (error) {
    failures.push(error.message);
  }
}

for (const [id, locales] of localesById) {
  for (const locale of ["en", "ru"])
    if (!locales.has(locale)) failures.push(`${id}: missing ${locale} mirror`);
}

const state = JSON.parse(
  readFileSync(join(wikiDir, "_meta/state.json"), "utf8"),
);
for (const id of localesById.keys())
  if (!state.pages[id]) failures.push(`_meta/state.json: missing page ${id}`);
for (const [id, page] of Object.entries(state.pages)) {
  if (!localesById.has(id)) failures.push(`_meta/state.json: stale page ${id}`);
  if (page.confidence !== undefined)
    failures.push(`_meta/state.json: unsupported subjective signal for ${id}`);
}

const navigation = JSON.parse(
  readFileSync(join(wikiDir, "_meta/navigation.json"), "utf8"),
);
const navigationIds = new Set();
function scanNavigation(value) {
  if (Array.isArray(value)) for (const child of value) scanNavigation(child);
  else if (value && typeof value === "object") {
    if ("id" in value && "path" in value) navigationIds.add(value.id);
    for (const child of Object.values(value)) scanNavigation(child);
  }
}
scanNavigation(navigation);
const graph = JSON.parse(
  readFileSync(join(wikiDir, "_meta/graph.json"), "utf8"),
);
const inbound = new Set(graph.edges.map((edge) => edge.to));
const outbound = new Set(graph.edges.map((edge) => edge.from));
for (const id of localesById.keys()) {
  if (id === "index") continue;
  if (!navigationIds.has(id) && !inbound.has(id))
    failures.push(`${id}: no navigation item or graph inbound edge`);
  if (!outbound.has(id))
    warnings.push(`${id}: graph leaf candidate with no outbound edge`);
}

if (warnings.length) {
  console.log("Consolidation candidates:");
  for (const warning of warnings) console.log(`[WARN] ${warning}`);
}
if (failures.length) {
  console.error("Structural wiki consolidation failures:");
  for (const failure of failures) console.error(`[FAIL] ${failure}`);
  process.exit(1);
}
console.log(
  `Wiki consolidation guard passed: ${pages.size} locale pages across ${localesById.size} page IDs.`,
);
