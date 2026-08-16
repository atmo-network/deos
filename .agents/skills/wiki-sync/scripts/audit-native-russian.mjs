#!/usr/bin/env node

import { readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const skillDir = resolve(scriptDir, "..");
const projectRoot = resolve(skillDir, "../../..");
const WORD = String.raw`\p{L}\p{N}_`;

function maskRange(chars, start, end) {
  for (
    let index = Math.max(0, start);
    index < Math.min(chars.length, end);
    index += 1
  ) {
    if (chars[index] !== "\n" && chars[index] !== "\r") chars[index] = " ";
  }
}

function maskMatches(chars, expression) {
  const value = chars.join("");
  for (const match of value.matchAll(expression))
    maskRange(chars, match.index, match.index + match[0].length);
}

function maskFences(chars) {
  const text = chars.join("");
  const opening = /^ {0,3}(`{3,}|~{3,})[^\n]*(?:\n|$)/gm;
  let match;
  while ((match = opening.exec(text)) !== null) {
    const marker = match[1];
    const closing = new RegExp(
      `^ {0,3}${marker[0]}{${marker.length},}[^\\n]*(?:\\n|$)`,
      "gm",
    );
    closing.lastIndex = match.index + match[0].length;
    const endMatch = closing.exec(text);
    const end = endMatch ? endMatch.index + endMatch[0].length : text.length;
    maskRange(chars, match.index, end);
    opening.lastIndex = end;
  }
}

function maskInlineCode(chars) {
  const text = chars.join("");
  for (let index = 0; index < text.length;) {
    if (text[index] !== "`" || chars[index] === " ") {
      index += 1;
      continue;
    }
    let width = 1;
    while (text[index + width] === "`") width += 1;
    const marker = "`".repeat(width);
    const end = text.indexOf(marker, index + width);
    if (end >= 0 && !text.slice(index + width, end).includes("\n")) {
      maskRange(chars, index, end + width);
      index = end + width;
    } else index += width;
  }
}

function maskExactLinkLabels(chars, exactFieldTerms) {
  const text = chars.join("");
  for (const term of exactFieldTerms) {
    const escaped = term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    for (const match of text.matchAll(
      new RegExp(`\\[${escaped}\\]\\(`, "gu"),
    )) {
      maskRange(chars, match.index + 1, match.index + 1 + term.length);
    }
  }
}

function maskLinkDestinations(chars) {
  const text = chars.join("");
  for (let index = 0; index < text.length - 1; index += 1) {
    if (text[index] !== "]" || text[index + 1] !== "(" || chars[index] === " ")
      continue;
    let depth = 1;
    let cursor = index + 2;
    let quote = null;
    for (; cursor < text.length && depth > 0; cursor += 1) {
      const character = text[cursor];
      if (character === "\n" || character === "\r") break;
      if (quote) {
        if (character === quote && text[cursor - 1] !== "\\") quote = null;
      } else if (character === '"' || character === "'") quote = character;
      else if (character === "(") depth += 1;
      else if (character === ")") depth -= 1;
    }
    if (depth === 0) maskRange(chars, index + 2, cursor - 1);
  }
}

function maskFrontmatterNonDisplayFields(chars, exactFieldTerms = []) {
  const text = chars.join("");
  if (!text.startsWith("---\n") && !text.startsWith("---\r\n")) return;
  const endMatch = /^---\s*$/gm;
  endMatch.lastIndex = text.indexOf("\n") + 1;
  const end = endMatch.exec(text);
  if (!end) return;
  const keepFields = new Set(["title", "description", "related"]);
  let currentField = null;
  let offset = 0;
  for (const line of text
    .slice(0, end.index + end[0].length)
    .matchAll(/.*(?:\n|$)/g)) {
    const value = line[0];
    const field = /^([A-Za-z_][\w-]*):/.exec(value)?.[1] ?? null;
    if (field) currentField = field;
    const scalar = field
      ? value
          .slice(value.indexOf(":") + 1)
          .trim()
          .replace(/^(['"])(.*)\1$/, "$2")
      : null;
    const listItem =
      /^\s*-\s+(.+?)\s*$/.exec(value)?.[1]?.replace(/^(['"])(.*)\1$/, "$2") ??
      null;
    const exactDisplayField =
      exactFieldTerms.includes(scalar) ||
      (currentField === "related" && exactFieldTerms.includes(listItem));
    if (
      !keepFields.has(currentField) ||
      exactDisplayField ||
      /^---\s*$/.test(value.trimEnd())
    ) {
      maskRange(chars, offset, offset + value.length);
    }
    offset += value.length;
  }
}

function maskSourceFragments(chars) {
  maskMatches(
    chars,
    /^\s*(?:const|let|var|type|interface|function|class|import|export)\b[^\n]*$/gmu,
  );
  maskMatches(
    chars,
    /^\s*[\p{L}_$][\p{L}\p{N}_$]*\s*:\s*(?:true|false|null|undefined|\d+(?:\.\d+)?|['"`{\[])[^\n]*$/gmu,
  );
}

function exactTermExpression(term) {
  const escaped = term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`(?<![${WORD}])${escaped}(?![${WORD}])`, "gu");
}

export function stripNonProse(text, canonicalTerms = [], options = {}) {
  const { frontmatter = false } = options;
  const chars = text.split("");
  if (
    options.exactField &&
    (options.exactFieldTerms ?? []).includes(text.trim())
  )
    return text.replace(/[^\r\n]/g, " ");
  if (frontmatter)
    maskFrontmatterNonDisplayFields(chars, options.exactFieldTerms ?? []);
  maskFences(chars);
  maskInlineCode(chars);
  maskExactLinkLabels(chars, options.exactFieldTerms ?? []);
  maskLinkDestinations(chars);
  maskMatches(chars, /\b(?:https?|file):\/\/[^\s<>"')\]]+/giu);
  maskMatches(
    chars,
    new RegExp(
      String.raw`(?<![${WORD}.-])(?:(?:\.{0,2}\/|\/)[^\s<>"'\x60)\]]+|(?:[A-Za-z0-9._-]+\/)+[A-Za-z0-9._-]+\.[A-Za-z0-9_-]+)`,
      "gu",
    ),
  );
  maskSourceFragments(chars);
  for (const term of [...canonicalTerms].sort(
    (left, right) => right.length - left.length,
  )) {
    maskMatches(chars, exactTermExpression(term));
  }
  return chars.join("");
}

function deduplicateOverlaps(findings) {
  const selected = [];
  for (const finding of [...findings].sort(
    (left, right) =>
      right.end - right.start - (left.end - left.start) ||
      left.ruleIndex - right.ruleIndex,
  )) {
    if (
      !selected.some(
        (item) => finding.start < item.end && item.start < finding.end,
      )
    )
      selected.push(finding);
  }
  return selected.sort(
    (left, right) =>
      left.start - right.start || left.ruleIndex - right.ruleIndex,
  );
}

export function auditTextDetailed(
  text,
  config,
  source = "<text>",
  options = {},
) {
  const prose = stripNonProse(text, config.canonical_terms ?? [], {
    ...options,
    exactFieldTerms: config.exact_field_terms ?? [],
  });
  const raw = [];
  for (const [ruleIndex, rule] of config.forbidden.entries()) {
    const expression = new RegExp(rule.pattern, "giu");
    for (const match of prose.matchAll(expression)) {
      const before = prose.slice(0, match.index);
      const line = (options.baseLine ?? 1) + (before.match(/\n/g)?.length ?? 0);
      const lastNewline = before.lastIndexOf("\n");
      const column =
        lastNewline < 0
          ? (options.baseColumn ?? 1) + before.length
          : before.length - lastNewline;
      raw.push({
        source,
        pointer: options.pointer,
        evidenceClass: options.evidenceClass ?? "display-prose",
        cohort: options.cohort ?? "fixture",
        line,
        column,
        start: match.index,
        end: match.index + match[0].length,
        match: match[0],
        ruleIndex,
        ...rule,
      });
    }
  }
  return { raw, findings: deduplicateOverlaps(raw), masked: prose };
}

export function auditText(text, config, source = "<text>", options = {}) {
  return auditTextDetailed(text, config, source, options).findings;
}

function escapePointer(value) {
  return value.replaceAll("~", "~0").replaceAll("/", "~1");
}

function collectRussianManifestRecords(
  value,
  path = [],
  records = [],
  inRussian = false,
) {
  if (typeof value === "string") {
    if (inRussian)
      records.push({
        value,
        pointer: `/${path.map(escapePointer).join("/")}`,
        evidenceClass: "display-prose",
      });
    return records;
  }
  if (Array.isArray(value)) {
    value.forEach((item, index) =>
      collectRussianManifestRecords(
        item,
        [...path, String(index)],
        records,
        inRussian,
      ),
    );
    return records;
  }
  if (!value || typeof value !== "object") return records;
  for (const [key, child] of Object.entries(value)) {
    collectRussianManifestRecords(
      child,
      [...path, key],
      records,
      inRussian || key === "ru",
    );
  }
  return records;
}

export function extractManifestEvidence(value, name) {
  if (name === "aliases.json") {
    const aliases = value?.aliases?.ru ?? {};
    return {
      displayRecords: [],
      searchAliases: Object.keys(aliases).map((key) => ({
        value: key,
        pointer: `/aliases/ru/${escapePointer(key)}`,
        evidenceClass: "search-alias",
      })),
      identifiers: Object.values(aliases).map((value, index) => ({
        value,
        pointer: `/aliases/ru/${index}`,
        evidenceClass: "identifier",
      })),
    };
  }
  return {
    displayRecords: collectRussianManifestRecords(value),
    searchAliases: [],
    identifiers: [],
  };
}

function findBalancedObject(text, start) {
  let depth = 0;
  let quote = null;
  for (let index = start; index < text.length; index += 1) {
    const character = text[index];
    if (quote) {
      if (character === "\\") index += 1;
      else if (character === quote) quote = null;
    } else if (character === "'" || character === '"' || character === "`")
      quote = character;
    else if (character === "{") depth += 1;
    else if (character === "}" && --depth === 0) return index + 1;
  }
  return -1;
}

function stringRecords(
  source,
  rangeStart = 0,
  rangeEnd = source.length,
  valuesOnly = false,
) {
  const records = [];
  for (let index = rangeStart; index < rangeEnd;) {
    const quote = source[index];
    if (quote !== "'" && quote !== '"' && quote !== "`") {
      index += 1;
      continue;
    }
    const start = index;
    let value = "";
    for (index += 1; index < rangeEnd; index += 1) {
      if (source[index] === "\\") {
        value += source[index] + (source[index + 1] ?? "");
        index += 1;
        continue;
      }
      if (source[index] === quote) break;
      value += source[index];
    }
    const isValue = source.slice(rangeStart, start).trimEnd().endsWith(":");
    if (!valuesOnly || isValue) {
      const prefix = source.slice(0, start + 1);
      records.push({
        value,
        start,
        line: (prefix.match(/\n/g)?.length ?? 0) + 1,
        column: start - prefix.lastIndexOf("\n") + 1,
      });
    }
    index += 1;
  }
  return records;
}

export function extractRussianWidgetStrings(source) {
  const records = stringRecords(source).filter(({ value }) =>
    /\p{Script=Cyrillic}/u.test(value),
  );
  const widgetStart = source.indexOf("const widgetText");
  const localeStart = source.indexOf("currentLocale === 'ru'", widgetStart);
  const objectStart = source.indexOf("{", localeStart);
  const objectEnd = findBalancedObject(source, objectStart);
  if (widgetStart < 0 || localeStart < 0 || objectStart < 0 || objectEnd < 0)
    throw new Error("Russian WikiWidget copy object not found");
  for (const record of stringRecords(source, objectStart, objectEnd, true)) {
    if (!records.some(({ start }) => start === record.start))
      records.push(record);
  }
  return records
    .sort((left, right) => left.start - right.start)
    .map(({ start: _start, ...record }) => record);
}

function walkRuPages(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) files.push(...walkRuPages(path));
    else if (entry.name.endsWith(".ru.md")) files.push(path);
  }
  return files.sort();
}

function parseArgs(argv) {
  const options = {
    wikiDir: join(projectRoot, "wiki"),
    configPath: join(skillDir, "native-russian-style.json"),
    frontendPath: join(
      projectRoot,
      "web-client/src/lib/widgets/WikiWidget.svelte",
    ),
    relationPath: join(
      projectRoot,
      "web-client/src/lib/wiki/relation-phrases.ts",
    ),
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--wiki-dir") options.wikiDir = resolve(argv[++index] ?? "");
    else if (arg.startsWith("--wiki-dir="))
      options.wikiDir = resolve(arg.slice(11));
    else if (arg === "--config")
      options.configPath = resolve(argv[++index] ?? "");
    else if (arg.startsWith("--config="))
      options.configPath = resolve(arg.slice(9));
    else if (arg === "--frontend")
      options.frontendPath = resolve(argv[++index] ?? "");
    else if (arg.startsWith("--frontend="))
      options.frontendPath = resolve(arg.slice(11));
    else if (arg === "--relations")
      options.relationPath = resolve(argv[++index] ?? "");
    else if (arg.startsWith("--relations="))
      options.relationPath = resolve(arg.slice(12));
    else if (arg === "--help" || arg === "-h") {
      process.stdout.write(
        "Usage: node audit-native-russian.mjs [--wiki-dir PATH] [--config PATH] [--frontend PATH] [--relations PATH]\n",
      );
      process.exit(0);
    } else throw new Error(`Unknown argument: ${arg}`);
  }
  return options;
}

function countBy(items, key) {
  const counts = {};
  for (const item of items) counts[item[key]] = (counts[item[key]] ?? 0) + 1;
  return counts;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const config = JSON.parse(readFileSync(options.configPath, "utf8"));
  const results = [];
  const pages = walkRuPages(options.wikiDir);
  for (const path of pages) {
    results.push(
      auditTextDetailed(readFileSync(path, "utf8"), config, path, {
        frontmatter: true,
        cohort: "pages",
        evidenceClass: "display-prose",
      }),
    );
  }

  const evidenceInventory = {
    "display-prose": 0,
    "search-alias": 0,
    identifier: 0,
  };
  const metaDir = join(options.wikiDir, "_meta");
  const locales = JSON.parse(
    readFileSync(join(metaDir, "locales.json"), "utf8"),
  );
  const expectedPages = Object.values(locales.pages ?? {})
    .map((localePaths) => localePaths?.ru)
    .filter((path) => typeof path === "string")
    .sort();
  const actualPages = pages
    .map((path) => relative(options.wikiDir, path).replaceAll("\\", "/"))
    .sort();
  if (new Set(expectedPages).size !== expectedPages.length)
    throw new Error("Locale manifest contains duplicate Russian concept paths");
  const missingPages = expectedPages.filter((path) => !actualPages.includes(path));
  const unexpectedPages = actualPages.filter((path) => !expectedPages.includes(path));
  if (missingPages.length > 0 || unexpectedPages.length > 0)
    throw new Error(
      `Russian concept inventory differs from canonical locale metadata: missing [${missingPages.join(", ")}], unexpected [${unexpectedPages.join(", ")}]`,
    );
  for (const name of [
    "aliases.json",
    "graph.json",
    "navigation.json",
    "state.json",
  ]) {
    const path = join(metaDir, name);
    const evidence = extractManifestEvidence(
      JSON.parse(readFileSync(path, "utf8")),
      name,
    );
    evidenceInventory["search-alias"] += evidence.searchAliases.length;
    evidenceInventory.identifier += evidence.identifiers.length;
    for (const record of evidence.displayRecords) {
      evidenceInventory["display-prose"] += 1;
      results.push(
        auditTextDetailed(record.value, config, path, {
          pointer: record.pointer,
          cohort: "manifests",
          evidenceClass: record.evidenceClass,
          exactField: true,
        }),
      );
    }
  }

  const frontend = readFileSync(options.frontendPath, "utf8");
  const widgetRecords = extractRussianWidgetStrings(frontend);
  evidenceInventory["display-prose"] += widgetRecords.length;
  for (const record of widgetRecords) {
    results.push(
      auditTextDetailed(record.value, config, options.frontendPath, {
        baseLine: record.line,
        baseColumn: record.column,
        cohort: "frontend",
        evidenceClass: "display-prose",
      }),
    );
  }

  const relationSource = readFileSync(options.relationPath, "utf8");
  const relationRecords = stringRecords(relationSource).filter(({ value }) =>
    /\p{Script=Cyrillic}/u.test(value),
  );
  evidenceInventory["display-prose"] += relationRecords.length;
  for (const record of relationRecords) {
    results.push(
      auditTextDetailed(record.value, config, options.relationPath, {
        baseLine: record.line,
        baseColumn: record.column,
        cohort: "frontend-relations",
        evidenceClass: "display-prose",
      }),
    );
  }

  const raw = results.flatMap((result) => result.raw);
  const findings = results.flatMap((result) => result.findings);
  const locationKey = (finding) =>
    `${finding.source}#${finding.pointer ?? `${finding.line}:${finding.column}`}`;
  const uniqueLocations = new Set(findings.map(locationKey));
  const affectedFiles = new Set(findings.map((finding) => finding.source));

  for (const finding of findings) {
    const location = finding.pointer
      ? `${finding.source}#${finding.pointer}`
      : `${finding.source}:${finding.line}:${finding.column}`;
    process.stderr.write(
      `${location}: ${finding.evidenceClass}/${finding.class}: “${finding.match}” — ${finding.guidance}\n`,
    );
  }
  const summary = {
    heuristic_raw_occurrences: raw.length,
    unique_source_locations: uniqueLocations.size,
    affected_files: affectedFiles.size,
    deduplicated_occurrences: findings.length,
    by_evidence_class: countBy(findings, "evidenceClass"),
    by_rule_class: countBy(findings, "class"),
    by_cohort: countBy(findings, "cohort"),
    evidence_inventory: evidenceInventory,
    russian_concepts: pages.length,
    rules: config.forbidden.length,
    canonical_term_exclusions: (config.canonical_terms ?? []).length,
    exact_field_exclusions: (config.exact_field_terms ?? []).length,
  };
  const output = `Native Russian audit ${findings.length ? "found heuristic debt" : "passed"}: ${JSON.stringify(summary)}\n`;
  if (findings.length) {
    process.stderr.write(output);
    process.exit(1);
  }
  process.stdout.write(
    `${output}Independent bilingual review remains required for native fluency and semantic parity.\n`,
  );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
