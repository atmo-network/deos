#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

let explicitRoot;
const args = process.argv.slice(2);
for (let index = 0; index < args.length; index += 1) {
  const arg = args[index];
  if (arg === "--help" || arg === "-h") {
    console.log(
      [
        "Usage: validate-context.mjs [project-root]",
        "",
        "Validates the current directory by default, VALIDATE_CONTEXT_ROOT when set,",
        "or the explicit project-root argument when provided.",
        "",
        "Markdown list items must be contiguous; blank separators fail validation.",
        "Markdown table formatting is always checked. Rows over 120 characters warn.",
      ].join("\n"),
    );
    process.exit(0);
  }
  if (arg.startsWith("--")) {
    console.error(`ERROR: Unknown option: ${arg}`);
    process.exit(1);
  }
  if (explicitRoot) {
    console.error("ERROR: Multiple project roots provided");
    process.exit(1);
  }
  explicitRoot = arg;
}
const root = path.resolve(
  explicitRoot || process.env.VALIDATE_CONTEXT_ROOT || process.cwd(),
);
if (!fs.existsSync(root) || !fs.statSync(root).isDirectory()) {
  console.error(
    `ERROR: Project root does not exist or is not a directory: ${root}`,
  );
  process.exit(1);
}
const noColor = process.env.NO_COLOR;
const tableWidthWarnThreshold = 120;
const markdownLinkScanMaxBytes = Number(
  process.env.ABCD_MARKDOWN_LINK_SCAN_MAX_BYTES || 262144,
);
if (
  !Number.isInteger(markdownLinkScanMaxBytes) ||
  markdownLinkScanMaxBytes <= 0
) {
  console.error(
    "ERROR: markdown link scan max bytes must be a positive integer",
  );
  process.exit(1);
}

const indexCandidates = [
  "AGENTS.md",
  "CLAUDE.md",
  "CODEX.md",
  "GEMINI.md",
  "CONTEXT.md",
];
const planCandidates = ["BACKLOG.md", "TODO.md", "PLAN.md", "ROADMAP.md"];
const skipDirs = new Set([
  "node_modules",
  "target",
  "build",
  "dist",
  "vendor",
  "obj",
  "out",
  "bin",
  "bower_components",
  "site-packages",
  "coverage",
  "Pods",
  ".git",
  ".cache",
  ".github",
  ".idea",
  ".next",
  ".pytest_cache",
  ".tox",
  ".venv",
  ".vscode",
  "venv",
  "temp",
  "tmp",
  "_build",
]);

let errors = 0;
let warnings = 0;
const colors = noColor
  ? { g: "", y: "", r: "", c: "", n: "" }
  : {
      g: "\x1b[0;32m",
      y: "\x1b[1;33m",
      r: "\x1b[0;31m",
      c: "\x1b[0;36m",
      n: "\x1b[0m",
    };

function add(level, msg) {
  if (level === "warn") warnings++;
  if (level === "fail") errors++;
  const tag =
    level === "pass"
      ? `${colors.g}[PASS]${colors.n}`
      : level === "warn"
        ? `${colors.y}[WARN]${colors.n}`
        : level === "fail"
          ? `${colors.r}[FAIL]${colors.n}`
          : `${colors.c}[INFO]${colors.n}`;
  console.log(`${tag} ${msg}`);
}
const pass = (msg) => add("pass", msg);
const warn = (msg) => add("warn", msg);
const fail = (msg) => add("fail", msg);
const info = (msg) => add("info", msg);
const progress = (msg) => console.log(msg);

function exists(rel) {
  return fs.existsSync(path.join(root, rel));
}
function read(file) {
  return fs.readFileSync(file, "utf8");
}
function rel(file) {
  return path.relative(root, file).replaceAll(path.sep, "/");
}

function walk(dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, ent.name);
    if (ent.isDirectory()) {
      if (!skipDirs.has(ent.name)) walk(p, out);
    } else if (ent.isFile() && ent.name.endsWith(".md")) {
      out.push(p);
    }
  }
  return out;
}

function markdownLines(text) {
  let fence = null;
  return text.split(/\r?\n/).map((line) => {
    if (fence) {
      const closing = line.match(/^ {0,3}(`{3,}|~{3,})\s*$/);
      if (
        closing &&
        closing[1][0] === fence.marker &&
        closing[1].length >= fence.length
      )
        fence = null;
      return { line, outsideFence: false };
    }
    const opening = line.match(/^ {0,3}(`{3,}|~{3,})(.*)$/);
    if (opening && (opening[1][0] === "~" || !opening[2].includes("`"))) {
      fence = { marker: opening[1][0], length: opening[1].length };
      return { line, outsideFence: false };
    }
    return { line, outsideFence: true };
  });
}

function markdownIndent(line) {
  const leading = line.match(/^[ \t]*/)?.[0] || "";
  return [...leading].reduce((width, char) => width + (char === "\t" ? 4 : 1), 0);
}

function findLooseListBlankLines(text) {
  const lines = markdownLines(text);
  const listIndents = [];
  let pendingBlankLine;
  const violations = [];
  for (let index = 0; index < lines.length; index += 1) {
    const { line, outsideFence } = lines[index];
    if (!outsideFence) {
      listIndents.length = 0;
      pendingBlankLine = undefined;
      continue;
    }
    if (/^\s*$/.test(line)) {
      if (listIndents.length > 0 && pendingBlankLine === undefined) {
        pendingBlankLine = index + 1;
      }
      continue;
    }
    const item = line.match(/^([ \t]*)(?:[-+*]|\d+[.)])[ \t]+\S/);
    if (item) {
      const indent = markdownIndent(item[1]);
      if (pendingBlankLine !== undefined) violations.push(pendingBlankLine);
      while (listIndents.length > 0 && listIndents.at(-1) > indent) listIndents.pop();
      if (listIndents.at(-1) !== indent) listIndents.push(indent);
      pendingBlankLine = undefined;
      continue;
    }
    if (pendingBlankLine !== undefined) {
      if (markdownIndent(line) > (listIndents.at(-1) ?? 0)) {
        violations.push(pendingBlankLine);
        pendingBlankLine = undefined;
        continue;
      }
      listIndents.length = 0;
      pendingBlankLine = undefined;
      continue;
    }
    if (/^ {0,3}(?:#{1,6}\s|>|\|)/.test(line)) listIndents.length = 0;
  }
  return violations;
}

function stripFences(text) {
  return markdownLines(text)
    .filter(({ outsideFence }) => outsideFence)
    .map(({ line }) => line)
    .join("\n");
}

function headingAnchor(text) {
  return text
    .toLowerCase()
    .replace(/[^\p{L}\p{N} _-]/gu, "")
    .trim()
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-");
}

progress("--- ABCd CONTEXT PROTOCOL VALIDATOR ---\n");

let contextName = "";
let contextFile = "";
for (const c of indexCandidates)
  if (exists(c)) {
    contextName = c;
    contextFile = path.join(root, c);
    break;
  }
contextFile
  ? pass(`Index file detected: ${contextName}`)
  : fail(`No index file found (checked: ${indexCandidates.join(" ")})`);

let planName = "";
let planFile = "";
for (const c of planCandidates)
  if (exists(c)) {
    planName = c;
    planFile = path.join(root, c);
    break;
  }
const changelogFile = path.join(root, "CHANGELOG.md");

const readme = path.join(root, "README.md");
if (fs.existsSync(readme)) {
  const txt = read(readme);
  txt.includes("docs/README.md")
    ? pass("README links to docs/README.md")
    : warn("README missing link to docs/README.md");
  contextName && txt.includes(contextName)
    ? pass(`README references index file (${contextName})`)
    : warn(`README missing reference to index file (${contextName})`);
  if (planName)
    txt.includes(planName)
      ? pass(`README references open-work file (${planName})`)
      : warn(`README missing reference to open-work file (${planName})`);
  if (fs.existsSync(changelogFile))
    txt.includes("CHANGELOG.md")
      ? pass("README references CHANGELOG.md")
      : warn("README missing reference to CHANGELOG.md");
} else warn("README.md missing");

if (contextFile) {
  const txt = read(contextFile);
  if (
    /## 1\./.test(txt) ||
    (txt.includes("Meta-Protocol Principles") &&
      /Discovered Constraints|Operating Principles/.test(txt))
  )
    pass("Core structure sections present");
  else warn("Core structure sections missing");
}

if (planName)
  planName === "BACKLOG.md"
    ? pass("Canonical open-work file detected: BACKLOG.md")
    : warn(
        `Canonical open-work file uses fallback alias: ${planName} (BACKLOG.md preferred)`,
      );
else
  warn(
    `No canonical open-work file found (checked: ${planCandidates.join(" ")})`,
  );
fs.existsSync(changelogFile)
  ? pass("CHANGELOG.md detected")
  : warn("CHANGELOG.md missing");

if (
  contextFile &&
  fs.existsSync(changelogFile) &&
  read(contextFile).includes("Change History")
)
  warn(
    "Index file contains Change History while CHANGELOG.md exists — consider separating durable rules from delivery history",
  );
else pass("Root state split looks coherent");

if (planFile && fs.existsSync(changelogFile)) {
  const backlog = read(planFile);
  const changelog = read(changelogFile).toLowerCase();
  let drift = 0;
  for (const m of backlog.matchAll(/^- \[ \] `([^`]+)`/gm)) {
    const label = m[1];
    const search = label.replace(/:$/, "").toLowerCase();
    if (search && changelog.includes(search)) {
      warn(
        `Possible root-state drift: open backlog slice also appears in CHANGELOG.md: ${label}`,
      );
      drift++;
    }
  }
  if (!drift) pass("No obvious BACKLOG/CHANGELOG drift detected");
}

progress("Checking documentation links...");
const mdFiles = walk(root);
let broken = false;
const targets = new Set();
for (const file of mdFiles) {
  const sourceSize = fs.statSync(file).size;
  if (sourceSize > markdownLinkScanMaxBytes) {
    info(
      `Skipped link validation for large Markdown file: ${rel(file)} (${sourceSize} bytes > ${markdownLinkScanMaxBytes})`,
    );
    continue;
  }
  const base = path.dirname(file);
  const text = stripFences(read(file));
  for (const m of text.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
    const raw = m[1];
    if (/^(https?:|mailto:)/.test(raw)) continue;
    let linkFile = raw;
    let anchor = "";
    if (raw.startsWith("#")) {
      linkFile = "";
      anchor = raw.slice(1);
    } else if (raw.includes("#")) [linkFile, anchor] = raw.split("#", 2);
    const target = linkFile ? path.resolve(base, linkFile) : file;
    if (!fs.existsSync(target)) {
      fail(`Broken link: ${raw} in ${rel(file)}`);
      broken = true;
      continue;
    }
    if (target !== file) targets.add(path.resolve(target));
    if (anchor && fs.statSync(target).isFile()) {
      const lineRef = anchor.match(/^L([1-9]\d*)(?:-L([1-9]\d*))?$/);
      if (lineRef) {
        const startLine = Number(lineRef[1]);
        const endLine = Number(lineRef[2] || lineRef[1]);
        const targetText = read(target);
        const totalLines = targetText
          ? targetText.split(/\r?\n/).length - (targetText.endsWith("\n") ? 1 : 0)
          : 0;
        if (startLine > totalLines || endLine > totalLines || endLine < startLine) {
          fail(
            `Line ref out of range: #${anchor} (file has ${totalLines} lines) in ${rel(target)} (from ${rel(file)})`,
          );
          broken = true;
        }
        continue;
      }
      const anchors = read(target)
        .split(/\r?\n/)
        .filter((l) => /^#{1,6} /.test(l))
        .map((l) => headingAnchor(l.replace(/^#{1,6} /, "")));
      if (!anchors.includes(anchor)) {
        fail(`Broken anchor: #${anchor} in ${rel(file)}`);
        broken = true;
      }
    }
  }
}
if (!broken) pass("Documentation links validated");

let readmeOrphans = 0;
for (const file of mdFiles.filter(
  (f) =>
    path.basename(f) === "README.md" &&
    path.resolve(f) !== path.resolve(readme),
)) {
  if (!targets.has(path.resolve(file))) {
    warn(
      `Unreachable README entrypoint: ${rel(file)} (no inbound markdown links)`,
    );
    readmeOrphans++;
  }
}
if (!readmeOrphans) pass("README entrypoints are reachable");

if (contextFile)
  read(contextFile).includes("Meta-Protocol Principles")
    ? pass("Meta-Protocol Principles found")
    : warn("Meta-Protocol Principles missing");

if (contextFile) {
  progress("Analyzing index file health...");
  const lines = read(contextFile).split(/\r?\n/);
  const total = lines.length;
  const headings = lines.filter((l) => /^#{1,6} /.test(l)).length;
  const listItems = lines.filter((l) => /^\s*[-*] /.test(l)).length;
  const blanks = lines.filter((l) => /^\s*$/.test(l)).length;
  const content = Math.max(1, total - blanks);
  const density = Math.floor(((headings + listItems) * 100) / content);
  info(
    `Index metrics: ${total} lines, ${headings} headings, density ${density}%`,
  );
  let signals = 0;
  if (density < 40) {
    warn(
      `Low information density (${density}%): index may contain verbose prose`,
    );
    signals++;
  }
  if (headings && Math.floor(total / headings) > 15) {
    warn(
      `Sparse structure (${Math.floor(total / headings)} lines/heading): consider splitting or consolidating`,
    );
    signals++;
  }
  signals === 0
    ? pass("Index file health: no bloat signals")
    : warn(
        `Index file health: ${signals} bloat signal(s) — consolidation recommended`,
      );
}

const docsDir = path.join(root, "docs");

progress("Checking Markdown lists...");
let listIssue = false;
for (const file of mdFiles) {
  const sourceSize = fs.statSync(file).size;
  if (sourceSize > markdownLinkScanMaxBytes) {
    info(
      `Skipped list validation for large Markdown file: ${rel(file)} (${sourceSize} bytes > ${markdownLinkScanMaxBytes})`,
    );
    continue;
  }
  for (const line of findLooseListBlankLines(read(file))) {
    fail(
      `Blank line inside Markdown list: ${rel(file)}:${line} (keep adjacent list items contiguous)`,
    );
    listIssue = true;
  }
}
if (!listIssue) pass("Markdown list checks passed");

progress("Checking Markdown tables...");
let tableIssue = false;
for (const file of mdFiles) {
  const lines = markdownLines(read(file));
  let table = null;
  const flushTable = () => {
    if (!table) return;
    if (table.maxLen > tableWidthWarnThreshold) {
      warn(
        `Wide Markdown table: ${rel(file)}:${table.start}-${table.end} (max ${table.maxLen} chars, ${table.wideRows}/${table.rows} rows > ${tableWidthWarnThreshold})`,
      );
      tableIssue = true;
    }
    table = null;
  };
  for (let i = 0; i < lines.length; i++) {
    const { line, outsideFence } = lines[i];
    if (!outsideFence || !/^ *\|.*\| *$/.test(line)) {
      flushTable();
      continue;
    }
    if (!table)
      table = {
        start: i + 1,
        end: i + 1,
        rows: 0,
        maxLen: 0,
        wideRows: 0,
      };
    table.end = i + 1;
    table.rows += 1;
    table.maxLen = Math.max(table.maxLen, line.length);
    if (line.length > tableWidthWarnThreshold) table.wideRows += 1;

    const cells = line
      .slice(line.indexOf("|") + 1, line.lastIndexOf("|"))
      .split("|");
    if (
      cells.length &&
      cells.every((cell) => /^\s*:?-{3,}:?\s*$/.test(cell))
    ) {
      const compact = cells.every((cell) =>
        /^ (?:---|---:|:---|:---:) $/.test(cell),
      );
      if (!compact) {
        fail(
          `Non-compact Markdown table delimiter: ${rel(file)}:${i + 1} (use exactly three hyphens and one space inside each pipe)`,
        );
        tableIssue = true;
      }
    }
  }
  flushTable();
}
if (!tableIssue) pass("Markdown table checks passed");

if (contextFile) {
  const age = Math.floor(
    (Date.now() - fs.statSync(contextFile).mtimeMs) / 86400000,
  );
  age < 30
    ? pass(`Context is fresh (${age} days old)`)
    : warn(`Context is stale (${age} days old)`);
}
fs.existsSync(docsDir)
  ? pass("Documentation directory exists")
  : warn("Documentation directory missing");

const docsIndex = path.join(docsDir, "README.md");
if (fs.existsSync(docsDir) && fs.existsSync(docsIndex)) {
  const idx = read(docsIndex);
  let orphan = 0,
    phantom = 0;
  for (const file of walk(docsDir).filter(
    (f) => path.basename(f) !== "README.md",
  )) {
    const r = path.relative(docsDir, file).replaceAll(path.sep, "/");
    if (!idx.includes(r)) {
      warn(`Orphaned doc: docs/${r} (not in docs/README.md)`);
      orphan++;
    }
  }
  for (const m of idx.matchAll(/\]\(\.\/([^)]+\.md)\)/g)) {
    if (!fs.existsSync(path.join(docsDir, m[1]))) {
      warn(`Phantom link in docs/README.md: ./${m[1]}`);
      phantom++;
    }
  }
  if (!orphan && !phantom) pass("Docs index coverage: 0 orphans, 0 phantoms");
}

console.log("\n--- VALIDATION SUMMARY ---\n");
console.log(`Warnings: ${warnings}`);
console.log(`Errors: ${errors}\n`);
if (!errors)
  console.log(
    `${colors.g}✓ Context validation PASSED${colors.n}`,
  );
else
  console.log(
    `${colors.r}✗ Context validation FAILED${colors.n}\nManual intervention required`,
  );
process.exit(errors > 0 ? 1 : 0);
