#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { readConcept } from "./okf-frontmatter.mjs";

const [
  wikiArgument = "wiki",
  minLinesArgument = "18",
  confidenceArgument = "0.85",
  rootArgument = ".",
] = process.argv.slice(2);
const wikiDir = resolve(wikiArgument);
const projectRoot = resolve(rootArgument);
const minimumBodyLines = Number(minLinesArgument);
const lowConfidenceThreshold = Number(confidenceArgument);
const failures = [];
const warnings = [];
const pages = new Map();
const localesById = new Map();
const confidenceByLocale = new Map();
const sourceDates = new Map();

function sourceCommitDate(path) {
  if (sourceDates.has(path)) return sourceDates.get(path);
  let value = null;
  try {
    const rel = relative(projectRoot, path);
    if (!rel.startsWith("..")) {
      const raw = execFileSync(
        "git",
        ["-C", projectRoot, "log", "-1", "--format=%cs", "--", rel],
        { encoding: "utf8" },
      ).trim();
      if (/^\d{4}-\d{2}-\d{2}$/.test(raw)) value = raw;
    }
  } catch {}
  sourceDates.set(path, value);
  return value;
}

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
      "last_compiled",
      "confidence",
    ]) {
      if (meta[field] === undefined || meta[field] === "")
        failures.push(`${rel}: missing required metadata ${field}`);
    }
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

    const compiled = /^\d{4}-\d{2}-\d{2}$/.test(String(meta.last_compiled))
      ? String(meta.last_compiled)
      : null;
    if (!compiled)
      failures.push(`${rel}: invalid last_compiled date ${meta.last_compiled}`);
    let latest = null;
    let latestResource = null;
    for (const source of meta.sources ?? []) {
      const resource = source?.resource;
      if (typeof resource !== "string") continue;
      const sourcePath = isAbsolute(resource)
        ? resource
        : resolve(dirname(path), resource);
      if (!existsSync(sourcePath)) {
        failures.push(`${rel}: missing source resource ${resource}`);
        continue;
      }
      const date = sourceCommitDate(sourcePath);
      if (date && (!latest || date > latest)) {
        latest = date;
        latestResource = resource;
      }
    }
    if (compiled && latest && latest > compiled)
      warnings.push(
        `${rel}: source-newer-than-page candidate (${latestResource} committed ${latest} > ${compiled})`,
      );
    const bodyLines = body.split("\n").filter((line) => line.trim()).length;
    if (id !== "index" && bodyLines < minimumBodyLines)
      warnings.push(`${rel}: short-page candidate (${bodyLines} body lines)`);

    const confidence = Number(meta.confidence);
    if (!Number.isFinite(confidence) || confidence < 0 || confidence > 1)
      failures.push(`${rel}: invalid confidence ${meta.confidence}`);
    else {
      if (Math.abs(confidence * 20 - Math.round(confidence * 20)) > 1e-9)
        failures.push(`${rel}: confidence must use 0.05 bands (${confidence})`);
      if (!confidenceByLocale.has(id)) confidenceByLocale.set(id, new Map());
      confidenceByLocale.get(id).set(locale, confidence);
      if (id !== "index" && confidence <= lowConfidenceThreshold)
        warnings.push(
          `${rel}: low-confidence candidate (${confidence.toFixed(2)})`,
        );
    }
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
for (const [id, confidences] of confidenceByLocale) {
  const statePage = state.pages[id];
  if (!statePage) failures.push(`_meta/state.json: missing page ${id}`);
  else {
    const expected = Math.min(...confidences.values());
    if (Number(statePage.confidence) !== expected)
      failures.push(`_meta/state.json: confidence drift for ${id}`);
  }
}
for (const id of Object.keys(state.pages))
  if (!localesById.has(id)) failures.push(`_meta/state.json: stale page ${id}`);

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
