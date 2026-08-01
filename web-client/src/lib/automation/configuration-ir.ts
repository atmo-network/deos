/*
Domain: AAA configuration IR and syntax adapters
Owns: Format-neutral actor configuration structure, deterministic normalization, structural diagnostics/diff, and JSON/TOML/Markdown interchange.
Excludes: Runtime metadata decoding, SCALE semantics, signing, submission, comments, presentation prose, and a second execution language.
Zone: Automation domain capability; lowers only through the canonical authoring contract.
*/
import {
  type AaaAuthoringLimits,
  type AaaAuthoringProgram,
  type AaaAuthoringStep,
  DEOS_AAA_AUTHORING_LIMITS,
  lowerAaaAuthoringProgram,
  validateAaaAuthoringProgram,
} from './authoring.ts';

export const AAA_CONFIGURATION_IR_FORMAT = 'deos.aaa.configuration-ir' as const;
export const AAA_CONFIGURATION_IR_VERSION = 1 as const;

export type AaaConfigurationIrStep = Omit<AaaAuthoringStep, 'key'>;

export type AaaConfigurationIr = Omit<
  AaaAuthoringProgram,
  'steps' | 'autoCloseAtCycleNonce'
> & {
  format: typeof AAA_CONFIGURATION_IR_FORMAT;
  formatVersion: typeof AAA_CONFIGURATION_IR_VERSION;
  autoCloseAtCycleNonce: string | null;
  steps: AaaConfigurationIrStep[];
};

export type AaaConfigurationIrDiagnostic = {
  path: string;
  severity: 'Error';
  message: string;
};

export type AaaConfigurationIrDiff = {
  path: string;
  kind: 'Added' | 'Removed' | 'Replaced';
  before?: unknown;
  after?: unknown;
};

const FIELD_NAMES = [
  'aaaType',
  'mutability',
  'completionPolicy',
  'autoCloseAtCycleNonce',
  'trigger',
  'cooldownBlocks',
  'scheduleWindow',
  'fundingPolicy',
  'steps',
] as const satisfies readonly (keyof AaaConfigurationIr)[];

function stableValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(stableValue);
  if (value != null && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, entry]) => [key, stableValue(entry)]),
    );
  }
  return value;
}

export function stableAaaConfigurationIrJson(value: unknown, space?: number) {
  return JSON.stringify(stableValue(value), null, space);
}

function requireRecord(value: unknown, label: string): Record<string, unknown> {
  if (value == null || Array.isArray(value) || typeof value !== 'object') {
    throw new Error(`${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

export function normalizeAaaConfigurationIr(
  value: unknown,
): AaaConfigurationIr {
  const record = requireRecord(value, 'AAA configuration IR');
  if (record.format !== AAA_CONFIGURATION_IR_FORMAT) {
    throw new Error(
      `Unsupported AAA configuration IR format: ${String(record.format)}`,
    );
  }
  if (record.formatVersion !== AAA_CONFIGURATION_IR_VERSION) {
    throw new Error(
      `Unsupported AAA configuration IR version: ${String(record.formatVersion)}`,
    );
  }
  const allowedFields = new Set<string>([
    'format',
    'formatVersion',
    ...FIELD_NAMES,
  ]);
  for (const field of FIELD_NAMES) {
    if (!(field in record))
      throw new Error(`Missing AAA configuration IR field: ${field}`);
  }
  for (const field of Object.keys(record)) {
    if (!allowedFields.has(field))
      throw new Error(`Unknown AAA configuration IR field: ${field}`);
  }
  if (
    record.autoCloseAtCycleNonce !== null &&
    (typeof record.autoCloseAtCycleNonce !== 'string' ||
      !/^[1-9][0-9]*$/.test(record.autoCloseAtCycleNonce))
  ) {
    throw new Error(
      'autoCloseAtCycleNonce must be null or a canonical positive integer string',
    );
  }
  const normalized = stableValue(record) as AaaConfigurationIr;
  return JSON.parse(
    stableAaaConfigurationIrJson(normalized),
  ) as AaaConfigurationIr;
}

export function authoringProgramToConfigurationIr(
  program: AaaAuthoringProgram,
): AaaConfigurationIr {
  return normalizeAaaConfigurationIr({
    format: AAA_CONFIGURATION_IR_FORMAT,
    formatVersion: AAA_CONFIGURATION_IR_VERSION,
    aaaType: program.aaaType,
    mutability: program.mutability,
    completionPolicy: program.completionPolicy,
    autoCloseAtCycleNonce: program.autoCloseAtCycleNonce?.toString() ?? null,
    trigger: program.trigger,
    cooldownBlocks: program.cooldownBlocks,
    scheduleWindow: program.scheduleWindow,
    fundingPolicy: program.fundingPolicy,
    steps: program.steps.map(({ conditionSet, task, errorPolicy }) => ({
      conditionSet,
      task,
      errorPolicy,
    })),
  });
}

export function configurationIrToAuthoringProgram(
  value: unknown,
): AaaAuthoringProgram {
  const ir = normalizeAaaConfigurationIr(value);
  return {
    aaaType: ir.aaaType,
    mutability: ir.mutability,
    completionPolicy: ir.completionPolicy,
    autoCloseAtCycleNonce:
      ir.autoCloseAtCycleNonce == null
        ? null
        : BigInt(ir.autoCloseAtCycleNonce),
    trigger: ir.trigger,
    cooldownBlocks: ir.cooldownBlocks,
    scheduleWindow: ir.scheduleWindow,
    fundingPolicy: ir.fundingPolicy,
    steps: ir.steps.map((step, index) => ({
      key: `genome-step-${String(index).padStart(3, '0')}`,
      conditionSet: step.conditionSet,
      task: step.task,
      errorPolicy: step.errorPolicy,
    })),
  };
}

export function diagnoseAaaConfigurationIr(
  value: unknown,
  limits: AaaAuthoringLimits = DEOS_AAA_AUTHORING_LIMITS,
): AaaConfigurationIrDiagnostic[] {
  try {
    const validation = validateAaaAuthoringProgram(
      configurationIrToAuthoringProgram(value),
      limits,
    );
    return validation.valid
      ? []
      : validation.issues.map((issue) => ({
          path: issue.path,
          severity: 'Error',
          message: issue.message,
        }));
  } catch (error) {
    return [
      {
        path: '/',
        severity: 'Error',
        message: error instanceof Error ? error.message : String(error),
      },
    ];
  }
}

export function lowerAaaConfigurationIr(
  value: unknown,
  limits: AaaAuthoringLimits = DEOS_AAA_AUTHORING_LIMITS,
) {
  return lowerAaaAuthoringProgram(
    configurationIrToAuthoringProgram(value),
    limits,
  );
}

function diffValues(
  before: unknown,
  after: unknown,
  path: string,
  output: AaaConfigurationIrDiff[],
) {
  if (
    stableAaaConfigurationIrJson(before) === stableAaaConfigurationIrJson(after)
  )
    return;
  if (Array.isArray(before) && Array.isArray(after)) {
    const length = Math.max(before.length, after.length);
    for (let index = 0; index < length; index += 1) {
      const nextPath = `${path}/${index}`;
      if (index >= before.length)
        output.push({ path: nextPath, kind: 'Added', after: after[index] });
      else if (index >= after.length)
        output.push({ path: nextPath, kind: 'Removed', before: before[index] });
      else diffValues(before[index], after[index], nextPath, output);
    }
    return;
  }
  if (
    before != null &&
    after != null &&
    typeof before === 'object' &&
    typeof after === 'object' &&
    !Array.isArray(before) &&
    !Array.isArray(after)
  ) {
    const left = before as Record<string, unknown>;
    const right = after as Record<string, unknown>;
    const keys = [
      ...new Set([...Object.keys(left), ...Object.keys(right)]),
    ].sort();
    for (const key of keys) {
      const nextPath = `${path}/${key}`;
      if (!(key in left))
        output.push({ path: nextPath, kind: 'Added', after: right[key] });
      else if (!(key in right))
        output.push({ path: nextPath, kind: 'Removed', before: left[key] });
      else diffValues(left[key], right[key], nextPath, output);
    }
    return;
  }
  output.push({ path: path || '/', kind: 'Replaced', before, after });
}

export function diffAaaConfigurationIr(
  before: unknown,
  after: unknown,
): AaaConfigurationIrDiff[] {
  const output: AaaConfigurationIrDiff[] = [];
  diffValues(
    normalizeAaaConfigurationIr(before),
    normalizeAaaConfigurationIr(after),
    '',
    output,
  );
  return output;
}

export function serializeAaaConfigurationJson(value: unknown) {
  return `${stableAaaConfigurationIrJson(normalizeAaaConfigurationIr(value), 2)}\n`;
}

export function parseAaaConfigurationJson(source: string) {
  return normalizeAaaConfigurationIr(JSON.parse(source));
}

export function serializeAaaConfigurationToml(value: unknown) {
  const ir = normalizeAaaConfigurationIr(value);
  const lines = [
    `format = ${JSON.stringify(AAA_CONFIGURATION_IR_FORMAT)}`,
    `format_version = ${AAA_CONFIGURATION_IR_VERSION}`,
    '',
    '[genome]',
  ];
  for (const field of FIELD_NAMES) {
    lines.push(
      `${field} = ${JSON.stringify(stableAaaConfigurationIrJson(ir[field]))}`,
    );
  }
  return `${lines.join('\n')}\n`;
}

export function parseAaaConfigurationToml(source: string) {
  const format = source.match(/^format\s*=\s*("(?:[^"\\]|\\.)*")\s*$/m);
  const version = source.match(/^format_version\s*=\s*(\d+)\s*$/m);
  if (format == null || version == null)
    throw new Error('Invalid AAA configuration TOML header');
  const record: Record<string, unknown> = {
    format: JSON.parse(format[1]),
    formatVersion: Number(version[1]),
  };
  for (const field of FIELD_NAMES) {
    const match = source.match(
      new RegExp(`^${field}\\s*=\\s*("(?:[^"\\\\]|\\\\.)*")\\s*$`, 'm'),
    );
    if (match == null)
      throw new Error(`Missing AAA configuration TOML field: ${field}`);
    record[field] = JSON.parse(JSON.parse(match[1]));
  }
  return normalizeAaaConfigurationIr(record);
}

export function serializeAaaConfigurationMarkdown(value: unknown) {
  const ir = normalizeAaaConfigurationIr(value);
  const sections = FIELD_NAMES.map(
    (field) =>
      `## ${field}\n\n\`\`\`json\n${stableAaaConfigurationIrJson(ir[field], 2)}\n\`\`\``,
  );
  return `# AAA Configuration Genome\n\n<!-- ${AAA_CONFIGURATION_IR_FORMAT}@${AAA_CONFIGURATION_IR_VERSION} -->\n\n${sections.join('\n\n')}\n`;
}

export function parseAaaConfigurationMarkdown(source: string) {
  const marker = source.match(/<!--\s*([^@\s]+)@(\d+)\s*-->/);
  if (marker == null)
    throw new Error('Invalid AAA configuration Markdown header');
  const record: Record<string, unknown> = {
    format: marker[1],
    formatVersion: Number(marker[2]),
  };
  for (const field of FIELD_NAMES) {
    const match = source.match(
      new RegExp(
        '^## ' + field + '\\s*\\n+```json\\n([\\s\\S]*?)\\n```\\s*$',
        'm',
      ),
    );
    if (match == null)
      throw new Error(`Missing AAA configuration Markdown field: ${field}`);
    record[field] = JSON.parse(match[1]);
  }
  return normalizeAaaConfigurationIr(record);
}
