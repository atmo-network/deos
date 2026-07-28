#!/usr/bin/env node
/*
Domain: AAA reactive operations corpus validation
Owns: Machine-readable fixture contract, runtime identity binding, evidence anchors, and deterministic failure artifacts.
Excludes: Runtime test execution, benchmark generation, release publication, and Router route semantics.
Zone: Shared human/CI validator implementation; invoked through reactive-operations-corpus.sh.
*/
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const corpusPath = resolve(
  root,
  "template/runtime/src/tests/fixtures/aaa-reactive-operations.v1.json",
);
const args = process.argv.slice(2);

function usage() {
  console.log(`Usage: validate-reactive-operations-corpus.mjs [OPTIONS]

Validates the deterministic AAA reactive-operations scenario contract and its
runtime-test evidence anchors. It does not execute runtime tests.

Options:
  --family <name>  Validate one scenario family
  --list           List scenario ids and families after validation
  --anchors        Emit package and Rust test symbol for selected scenarios
  -h, --help       Show this help`);
}

let family = null;
let list = false;
let anchors = false;
for (let index = 0; index < args.length; index += 1) {
  const arg = args[index];
  if (arg === "-h" || arg === "--help") {
    usage();
    process.exit(0);
  }
  if (arg === "--list") {
    list = true;
    continue;
  }
  if (arg === "--anchors") {
    anchors = true;
    continue;
  }
  if (arg === "--family") {
    family = args[index + 1] ?? null;
    if (family == null || family.startsWith("-")) {
      console.error("--family requires a value");
      process.exit(2);
    }
    index += 1;
    continue;
  }
  console.error(`Unknown argument: ${arg}`);
  process.exit(2);
}

const source = readFileSync(corpusPath, "utf8");
const corpus = JSON.parse(source);
const failures = [];
const requiredScenarioKeys = [
  "id",
  "family",
  "initialState",
  "actions",
  "checkpoints",
  "terminalState",
  "invariants",
  "expectedWeightClass",
  "rollbackBoundary",
  "expectedEvidenceIdentity",
  "executionAnchor",
];

function fail(scope, message) {
  failures.push({ scope, message });
}

function nonEmptyString(value) {
  return typeof value === "string" && value.trim().length > 0;
}

if (corpus.schemaVersion !== "deos.aaa-reactive-operations/1") {
  fail("corpus", "unsupported schemaVersion");
}
const runtime = corpus.runtimeIdentity;
if (runtime == null || typeof runtime !== "object") {
  fail("corpus", "runtimeIdentity is required");
} else {
  const expectedId = `${runtime.specName}@spec-${runtime.specVersion}:tx-${runtime.transactionVersion}:system-${runtime.systemVersion}`;
  if (runtime.id !== expectedId)
    fail("corpus", "runtimeIdentity.id is not canonical");
  const generated = readFileSync(
    resolve(
      root,
      "web-client/src/lib/observation/runtime-evidence.generated.ts",
    ),
    "utf8",
  );
  for (const [field, pattern, expected] of [
    ["specName", /specName: '([^']+)'/, runtime.specName],
    ["specVersion", /specVersion: (\d+)/, runtime.specVersion],
    ["systemVersion", /systemVersion: (\d+)/, runtime.systemVersion],
    [
      "transactionVersion",
      /transactionVersion: (\d+)/,
      runtime.transactionVersion,
    ],
  ]) {
    const match = generated.match(pattern);
    const actual = field === "specName" ? match?.[1] : Number(match?.[1]);
    if (actual !== expected) {
      fail("corpus", `${field} differs from generated runtime evidence`);
    }
  }
}

if (corpus.invariants == null || typeof corpus.invariants !== "object") {
  fail("corpus", "invariants registry is required");
}
if (!Array.isArray(corpus.scenarios) || corpus.scenarios.length === 0) {
  fail("corpus", "scenarios must be a non-empty array");
}

const ids = new Set();
const selected = [];
for (const scenario of corpus.scenarios ?? []) {
  const scope = nonEmptyString(scenario?.id) ? scenario.id : "unknown-scenario";
  for (const key of requiredScenarioKeys) {
    if (!(key in (scenario ?? {}))) fail(scope, `missing ${key}`);
  }
  if (!nonEmptyString(scenario?.id)) fail(scope, "id must be non-empty");
  if (ids.has(scenario?.id)) fail(scope, "scenario id must be unique");
  ids.add(scenario?.id);
  if (!nonEmptyString(scenario?.family))
    fail(scope, "family must be non-empty");
  if (family != null && scenario?.family !== family) continue;
  selected.push(scenario);
  if (
    scenario.initialState == null ||
    Array.isArray(scenario.initialState) ||
    typeof scenario.initialState !== "object"
  ) {
    fail(scope, "initialState must be an object");
  }
  if (!Array.isArray(scenario.actions) || scenario.actions.length === 0) {
    fail(scope, "actions must be a non-empty array");
  }
  if (
    (scenario.actions ?? []).some((action) =>
      String(action?.kind ?? "").startsWith("Seeded"),
    ) &&
    !nonEmptyString(scenario.seed)
  ) {
    fail(scope, "seeded actions require an explicit seed");
  }
  if (
    !Array.isArray(scenario.checkpoints) ||
    scenario.checkpoints.length === 0
  ) {
    fail(scope, "checkpoints must be a non-empty array");
  }
  for (const checkpoint of scenario.checkpoints ?? []) {
    if (
      !Number.isSafeInteger(checkpoint.afterAction) ||
      checkpoint.afterAction < 1 ||
      checkpoint.afterAction > (scenario.actions?.length ?? 0)
    ) {
      fail(scope, "checkpoint afterAction must identify an ordered action");
    }
    if (
      checkpoint.expect == null ||
      Array.isArray(checkpoint.expect) ||
      typeof checkpoint.expect !== "object"
    ) {
      fail(scope, "checkpoint expect must be an object");
    }
  }
  if (
    scenario.terminalState == null ||
    Array.isArray(scenario.terminalState) ||
    typeof scenario.terminalState !== "object"
  ) {
    fail(scope, "terminalState must be an object");
  }
  if (!Array.isArray(scenario.invariants) || scenario.invariants.length === 0) {
    fail(scope, "invariants must be a non-empty array");
  }
  for (const invariant of scenario.invariants ?? []) {
    if (!nonEmptyString(corpus.invariants?.[invariant])) {
      fail(scope, `unknown invariant: ${String(invariant)}`);
    }
  }
  if (!nonEmptyString(scenario.expectedWeightClass)) {
    fail(scope, "expectedWeightClass must be non-empty");
  }
  if (!nonEmptyString(scenario.rollbackBoundary)) {
    fail(scope, "rollbackBoundary must be non-empty");
  }
  if (scenario.expectedEvidenceIdentity !== runtime?.id) {
    fail(
      scope,
      "expectedEvidenceIdentity differs from corpus runtime identity",
    );
  }
  const anchor = scenario.executionAnchor;
  if (
    anchor == null ||
    !nonEmptyString(anchor.path) ||
    !nonEmptyString(anchor.symbol) ||
    !/^[a-z][a-z0-9_]*$/.test(anchor.symbol) ||
    (!anchor.path.startsWith("template/pallets/aaa/") &&
      !anchor.path.startsWith("template/runtime/"))
  ) {
    fail(scope, "executionAnchor must name an AAA pallet/runtime test");
    continue;
  }
  try {
    const anchorSource = readFileSync(resolve(root, anchor.path), "utf8");
    const escaped = anchor.symbol.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    if (!new RegExp(`\\bfn\\s+${escaped}\\s*\\(`).test(anchorSource)) {
      fail(scope, `executionAnchor symbol not found: ${anchor.symbol}`);
    }
  } catch {
    fail(scope, `executionAnchor path not found: ${anchor.path}`);
  }
}

if (family != null && selected.length === 0) {
  fail("corpus", `unknown or empty family: ${family}`);
}
if (family == null) {
  for (const invariant of [
    "oracleRevisionLinked",
    "dirtyFeedUnique",
    "subscriberPageReachable",
    "admissionExclusive",
    "atomicFailureRestoresOwnedState",
    "boundedWork",
  ]) {
    if (!selected.some((scenario) => scenario.invariants?.includes(invariant))) {
      fail("corpus", `global invariant lacks a scenario: ${invariant}`);
    }
  }
}

if (failures.length > 0) {
  const digest = createHash("sha256").update(source).digest("hex").slice(0, 16);
  const artifactPath = resolve(
    process.env.TMPDIR ?? "/tmp",
    `deos-reactive-corpus-failure-${digest}.json`,
  );
  writeFileSync(
    artifactPath,
    `${JSON.stringify(
      {
        corpusPath,
        corpusSha256: createHash("sha256").update(source).digest("hex"),
        selectedFamily: family,
        selectedScenarios: selected.map((scenario) => ({
          id: scenario.id,
          seed: scenario.seed ?? null,
          initialState: scenario.initialState,
        })),
        failures,
      },
      null,
      2,
    )}\n`,
  );
  for (const failure of failures) {
    console.error(`[FAIL] ${failure.scope}: ${failure.message}`);
  }
  console.error(`Failure artifact: ${artifactPath}`);
  process.exit(1);
}

if (list) {
  for (const scenario of selected)
    console.log(`${scenario.family}\t${scenario.id}`);
}
if (anchors) {
  for (const scenario of selected) {
    const packageName = scenario.executionAnchor.path.startsWith(
      "template/pallets/aaa/",
    )
      ? "pallet-deos-aaa"
      : "deos-runtime";
    console.log(`${packageName}\t${scenario.executionAnchor.symbol}`);
  }
}
console.log(
  `Reactive operations corpus valid: ${selected.length}/${corpus.scenarios.length} scenario(s)${family == null ? "" : ` in ${family}`}`,
);
