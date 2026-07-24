#!/usr/bin/env node

import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const MAX_CHARS_PER_PAGE = 12_000;
const scriptDir = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(scriptDir, "../../../..");
const defaultWikiDir = join(projectRoot, "wiki");

function usage() {
  process.stdout.write(
    `Usage: build-search-index.mjs [OPTIONS]\n\nGenerate the bounded bilingual plain-text manifest used by Wiki client search.\n\nOptions:\n  --wiki-dir <path>  Override the wiki directory (default: ./wiki)\n  --check            Fail when the committed manifest differs from generated output\n  -h, --help         Show this help message\n`,
  );
}

function parseArgs(argv) {
  let wikiDir = defaultWikiDir;
  let check = false;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--check") {
      check = true;
    } else if (argument === "--wiki-dir") {
      const value = argv[index + 1];
      if (!value) throw new Error("Missing value for --wiki-dir");
      wikiDir = resolve(value);
      index += 1;
    } else if (argument.startsWith("--wiki-dir=")) {
      const value = argument.slice("--wiki-dir=".length);
      if (!value) throw new Error("Missing value for --wiki-dir");
      wikiDir = resolve(value);
    } else if (argument === "--help" || argument === "-h") {
      usage();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${argument}`);
    }
  }
  return { check, wikiDir };
}

function stripMarkdown(markdown) {
  let body = markdown.replace(/^---\n[\s\S]*?\n---\n?/, "");
  body = body
    .replace(/```[^\n]*\n?/g, " ")
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/^\s*[-*+]\s+/gm, "")
    .replace(/^\s*\d+\.\s+/gm, "")
    .replace(/[>*_`~|]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  return Array.from(body).slice(0, MAX_CHARS_PER_PAGE).join("");
}

function resolvePageFiles(wikiDir) {
  const files = [];
  const walk = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {
        if (entry.name !== "_meta") walk(path);
      } else if (entry.isFile() && /\.(?:en|ru)\.md$/.test(entry.name)) {
        files.push(path);
      }
    }
  };
  walk(wikiDir);
  return files.sort();
}

function parsePage(path) {
  const markdown = readFileSync(path, "utf8");
  const frontmatter = markdown.match(/^---\n([\s\S]*?)\n---/)?.[1] ?? "";
  const id = frontmatter.match(/^canonical_page_id:\s*(.+)$/m)?.[1]?.trim();
  const locale = frontmatter.match(/^locale:\s*(.+)$/m)?.[1]?.trim();
  if (!id || !locale) {
    throw new Error(`Missing canonical_page_id or locale: ${path}`);
  }
  return { id, locale, text: stripMarkdown(markdown) };
}

function buildManifest(wikiDir) {
  const state = JSON.parse(
    readFileSync(join(wikiDir, "_meta/state.json"), "utf8"),
  );
  const pages = new Map();
  for (const path of resolvePageFiles(wikiDir)) {
    const page = parsePage(path);
    const entry = pages.get(page.id) ?? { id: page.id, text: {} };
    entry.text[page.locale] = page.text;
    pages.set(page.id, entry);
  }
  const expectedLocales = [...state.available_locales].sort();
  for (const [id, page] of pages) {
    const locales = Object.keys(page.text).sort();
    if (JSON.stringify(locales) !== JSON.stringify(expectedLocales)) {
      throw new Error(
        `Locale coverage mismatch for ${id}: ${locales.join(", ")}`,
      );
    }
  }
  const searchPageIds = [...pages.keys()].sort();
  const statePageIds = Object.keys(state.pages).sort();
  if (JSON.stringify(searchPageIds) !== JSON.stringify(statePageIds)) {
    throw new Error(
      `Page coverage mismatch: search=${searchPageIds.join(",")} state=${statePageIds.join(",")}`,
    );
  }
  return {
    generated_at: state.generated_at,
    max_chars_per_page: MAX_CHARS_PER_PAGE,
    default_locale: state.default_locale,
    available_locales: state.available_locales,
    pages: [...pages.values()].sort((left, right) =>
      left.id.localeCompare(right.id),
    ),
  };
}

try {
  const { check, wikiDir } = parseArgs(process.argv.slice(2));
  const outputPath = join(wikiDir, "_meta/search.json");
  const output = `${JSON.stringify(buildManifest(wikiDir), null, 2)}\n`;
  if (check) {
    if (readFileSync(outputPath, "utf8") !== output) {
      process.stderr.write(
        `Wiki search manifest is stale. Run ${basename(import.meta.filename)}.\n`,
      );
      process.exit(1);
    }
    process.stdout.write("Wiki search manifest is current.\n");
  } else {
    writeFileSync(outputPath, output);
    process.stdout.write(`Wrote ${outputPath}.\n`);
  }
} catch (error) {
  process.stderr.write(
    `${error instanceof Error ? error.message : String(error)}\n`,
  );
  process.exit(1);
}
