#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";

const scriptDir = path.dirname(new URL(import.meta.url).pathname);
const skillDir = path.resolve(scriptDir, "..");
const validator = path.join(scriptDir, "validate-context.mjs");
const fixtureRoot = path.join(skillDir, "fixtures/abcd-project");

let pass = 0;
let fail = 0;

const defaultOutput = run([]);
const pathOutput = run([fixtureRoot], { withoutRootEnv: true });
const selfOutput = run([skillDir], { withoutRootEnv: true });
const missingPathOutput = run([path.join(fixtureRoot, "missing")], {
  withoutRootEnv: true,
});
const jsonOptionOutput = run(["--json"]);
const invalidLineRefRoot = createInvalidLineRefFixture();
const tableRoot = createTableFixture();
const listRoot = createListFixture();
try {
  const invalidLineRefOutput = run([invalidLineRefRoot], {
    withoutRootEnv: true,
  });
  const compactTableOutput = run([tableRoot], { withoutRootEnv: true });
  fs.writeFileSync(
    path.join(tableRoot, "docs/table.md"),
    [
      "# Table",
      "",
      `| ${"Wide header ".repeat(12)} | Value |`,
      "|------------|---:|",
      "| Content | 1 |",
      "",
      "LaTeX is allowed: $\\frac{a}{b}$.",
      "",
    ].join("\n"),
  );
  const nonCompactTableOutput = run([tableRoot], { withoutRootEnv: true });
  fs.writeFileSync(
    path.join(tableRoot, "docs/table.md"),
    fs
      .readFileSync(path.join(tableRoot, "docs/table.md"), "utf8")
      .replace("|------------|---:|", "| --- | ---: |"),
  );
  const wideTableOutput = run([tableRoot], { withoutRootEnv: true });
  const tightListOutput = run([listRoot], { withoutRootEnv: true });
  fs.appendFileSync(
    path.join(listRoot, "docs/lists.md"),
    "\n- First loose item.\n  Continued detail.\n\n- Second loose item.\n  - Nested item.\n\n  - Nested sibling.\n",
  );
  const looseListOutput = run([listRoot], { withoutRootEnv: true });

  checkSuccess("default fixture", defaultOutput);
  checkSuccess("fixture path arg", pathOutput);
  checkSuccess("self", selfOutput);
  checkMissingPath(missingPathOutput);
  checkRejectedJsonOption(jsonOptionOutput);
  checkInvalidLineReference(invalidLineRefOutput);
  checkCompactTableAndLatex(compactTableOutput);
  checkNonCompactTable(nonCompactTableOutput);
  checkWideTable(wideTableOutput);
  checkTightLists(tightListOutput);
  checkLooseLists(looseListOutput);
} finally {
  fs.rmSync(invalidLineRefRoot, { recursive: true, force: true });
  fs.rmSync(tableRoot, { recursive: true, force: true });
  fs.rmSync(listRoot, { recursive: true, force: true });
}

console.log("PASS: validate-context fixture + self-reference regression");
console.log(`Self-test assertions: ${pass} passed, ${fail} failed`);
process.exit(fail ? 1 : 0);

function run(argv, options = {}) {
  const env = {
    ...process.env,
    NO_COLOR: "1",
  };
  if (!options.withoutRootEnv) env.VALIDATE_CONTEXT_ROOT = fixtureRoot;
  const result = spawnSync(process.execPath, [validator, ...argv], {
    encoding: "utf8",
    env,
  });
  if (result.error) {
    return { status: 1, stdout: "", stderr: String(result.error) };
  }
  return result;
}

function checkSuccess(scope, result) {
  assert(result.status === 0, `${scope} exits 0`);
  assert(result.stdout.includes("[PASS]"), `${scope} prints readable checks`);
  assert(
    result.stdout.includes("Context validation PASSED"),
    `${scope} prints a readable summary`,
  );
}

function checkMissingPath(result) {
  assert(result.status !== 0, "missing path is rejected");
  assert(
    result.stderr.includes("Project root does not exist"),
    "missing path is explained",
  );
}

function checkRejectedJsonOption(result) {
  assert(result.status !== 0, "JSON option is rejected");
  assert(
    result.stderr.includes("Unknown option: --json"),
    "JSON option removal is explained",
  );
}

function checkInvalidLineReference(result) {
  assert(result.status !== 0, "out-of-range line reference is rejected");
  assert(
    result.stdout.includes("Line ref out of range"),
    "out-of-range line reference is explained",
  );
}

function checkCompactTableAndLatex(result) {
  assert(result.status === 0, "compact table and LaTeX are accepted");
  assert(
    result.stdout.includes("Markdown table checks passed"),
    "compact table delimiter passes",
  );
  assert(!result.stdout.includes("LaTeX syntax"), "LaTeX is not checked");
  assert(
    result.stdout.includes("Documentation links validated"),
    "links inside indented backtick and tilde fences are ignored",
  );
}

function checkNonCompactTable(result) {
  assert(result.status !== 0, "non-compact table delimiter fails validation");
  assert(
    result.stdout.includes("Non-compact Markdown table delimiter"),
    "non-compact table delimiter is explained",
  );
}

function checkWideTable(result) {
  assert(result.status === 0, "wide table remains valid");
  assert(
    result.stdout.includes("Wide Markdown table"),
    "table row over 120 chars warns",
  );
}

function checkTightLists(result) {
  assert(result.status === 0, "contiguous lists and fenced examples pass");
  assert(
    result.stdout.includes("Markdown list checks passed"),
    "contiguous list check reports success",
  );
}

function checkLooseLists(result) {
  assert(result.status !== 0, "blank lines inside lists fail validation");
  assert(
    result.stdout.includes("Blank line inside Markdown list: docs/lists.md"),
    "loose list failure identifies its file",
  );
}

function createListFixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "abcd-context-list-"));
  fs.mkdirSync(path.join(root, "docs"));
  fs.writeFileSync(
    path.join(root, "README.md"),
    "# Probe\n\n[Context](./AGENTS.md) [Backlog](./BACKLOG.md) [History](./CHANGELOG.md) [Docs](./docs/README.md)\n",
  );
  fs.writeFileSync(
    path.join(root, "AGENTS.md"),
    "# Context\n\n## Meta-Protocol Principles\n\n- First rule.\n- Second rule.\n\n## Operating Principles\n\n- Rule.\n",
  );
  fs.writeFileSync(path.join(root, "BACKLOG.md"), "# Backlog\n\nNo open work.\n");
  fs.writeFileSync(path.join(root, "CHANGELOG.md"), "# Changelog\n\nNo releases.\n");
  fs.writeFileSync(path.join(root, "docs/README.md"), "# Docs\n\n- [Lists](./lists.md)\n");
  fs.writeFileSync(
    path.join(root, "docs/lists.md"),
    [
      "# Lists",
      "",
      "1. First item.",
      "2. Second item.",
      "   - Nested item.",
      "   - Nested sibling.",
      "",
      "```markdown",
      "- Fenced item.",
      "",
      "- Fenced loose item.",
      "```",
      "",
    ].join("\n"),
  );
  return root;
}

function createTableFixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "abcd-context-table-"));
  fs.mkdirSync(path.join(root, "docs"));
  fs.writeFileSync(
    path.join(root, "README.md"),
    [
      "# Probe",
      "",
      "- [Context](./AGENTS.md)",
      "- [Backlog](./BACKLOG.md)",
      "- [History](./CHANGELOG.md)",
      "- [Docs](./docs/README.md)",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    path.join(root, "AGENTS.md"),
    "# Context\n\n## Meta-Protocol Principles\n\n- Rule.\n\n## Operating Principles\n\n- Rule.\n",
  );
  fs.writeFileSync(path.join(root, "BACKLOG.md"), "# Backlog\n\nNo open work.\n");
  fs.writeFileSync(path.join(root, "CHANGELOG.md"), "# Changelog\n\nNo releases.\n");
  fs.writeFileSync(
    path.join(root, "docs/README.md"),
    "# Docs\n\n- [Table](./table.md)\n",
  );
  fs.writeFileSync(
    path.join(root, "docs/table.md"),
    [
      "# Table",
      "",
      "| Left | Right | Center |",
      "| --- | ---: | :---: |",
      "| A | B | C |",
      "",
      "   ````markdown",
      "|------------|---:|",
      "[Ignored missing link](./missing-backtick.md)",
      "```",
      "````",
      "",
      "~~~markdown",
      "|------------|---:|",
      "[Ignored missing link](./missing-tilde.md)",
      "~~~",
      "",
      "LaTeX is allowed: $\\frac{a}{b}$.",
      "",
    ].join("\n"),
  );
  return root;
}

function createInvalidLineRefFixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "abcd-context-line-ref-"));
  fs.mkdirSync(path.join(root, "docs"));
  fs.writeFileSync(
    path.join(root, "README.md"),
    [
      "# Probe",
      "",
      "- [Context](./AGENTS.md)",
      "- [Backlog](./BACKLOG.md)",
      "- [History](./CHANGELOG.md)",
      "- [Docs](./docs/README.md)",
      "- [Invalid line](./AGENTS.md#L999)",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    path.join(root, "AGENTS.md"),
    [
      "# Context",
      "",
      "## Meta-Protocol Principles",
      "",
      "- Rule.",
      "",
      "## Operating Principles",
      "",
      "- Rule.",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(path.join(root, "BACKLOG.md"), "# Backlog\n\nNo open work.\n");
  fs.writeFileSync(path.join(root, "CHANGELOG.md"), "# Changelog\n\nNo releases.\n");
  fs.writeFileSync(path.join(root, "docs/README.md"), "# Docs\n\nNo docs.\n");
  return root;
}

function assert(ok, label) {
  if (ok) pass++;
  else {
    fail++;
    console.error(`FAIL: ${label}`);
  }
}
