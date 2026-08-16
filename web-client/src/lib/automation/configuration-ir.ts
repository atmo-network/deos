/*
Domain: Actors configuration IR and syntax adapters
Owns: Format-neutral actor configuration structure, deterministic normalization, structural diagnostics/diff, and JSON/TOML/Markdown interchange.
Excludes: Runtime metadata decoding, SCALE semantics, signing, submission, comments, presentation prose, and a second execution language.
Zone: Automation domain capability; lowers only through the canonical authoring contract.
*/
import {
  type ActorAuthoringContract,
  type ActorAuthoringLimits,
  type ActorAuthoringStep,
  DEOS_ACTORS_AUTHORING_LIMITS,
  lowerActorAuthoringContract,
  validateActorAuthoringContract,
} from './authoring.ts';

export const ACTORS_CONFIGURATION_IR_FORMAT =
  'deos.actor.configuration-ir' as const;
export const ACTORS_CONFIGURATION_IR_VERSION = 1 as const;

export type ActorConfigurationIrStep = Omit<ActorAuthoringStep, 'key'>;

export type ActorConfigurationIr = Omit<
  ActorAuthoringContract,
  'steps' | 'autoCloseAtCycleNonce'
> & {
  format: typeof ACTORS_CONFIGURATION_IR_FORMAT;
  formatVersion: typeof ACTORS_CONFIGURATION_IR_VERSION;
  autoCloseAtCycleNonce: string | null;
  steps: ActorConfigurationIrStep[];
};

export type ActorConfigurationIrDiagnostic = {
  path: string;
  severity: 'Error';
  message: string;
};

export type ActorConfigurationIrDiff = {
  path: string;
  kind: 'Added' | 'Removed' | 'Replaced';
  before?: unknown;
  after?: unknown;
};

const FIELD_NAMES = [
  'actorType',
  'mutability',
  'completionPolicy',
  'autoCloseAtCycleNonce',
  'trigger',
  'cooldownBlocks',
  'scheduleWindow',
  'fundingPolicy',
  'steps',
] as const satisfies readonly (keyof ActorConfigurationIr)[];

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

export function stableActorConfigurationIrJson(value: unknown, space?: number) {
  return JSON.stringify(stableValue(value), null, space);
}

function requireRecord(value: unknown, label: string): Record<string, unknown> {
  if (value == null || Array.isArray(value) || typeof value !== 'object') {
    throw new Error(`${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

export function normalizeActorConfigurationIr(
  value: unknown,
): ActorConfigurationIr {
  const record = requireRecord(value, 'Actors configuration IR');
  if (record.format !== ACTORS_CONFIGURATION_IR_FORMAT) {
    throw new Error(
      `Unsupported Actors configuration IR format: ${String(record.format)}`,
    );
  }
  if (record.formatVersion !== ACTORS_CONFIGURATION_IR_VERSION) {
    throw new Error(
      `Unsupported Actors configuration IR version: ${String(record.formatVersion)}`,
    );
  }
  const allowedFields = new Set<string>([
    'format',
    'formatVersion',
    ...FIELD_NAMES,
  ]);
  for (const field of FIELD_NAMES) {
    if (!(field in record))
      throw new Error(`Missing Actors configuration IR field: ${field}`);
  }
  for (const field of Object.keys(record)) {
    if (!allowedFields.has(field))
      throw new Error(`Unknown Actors configuration IR field: ${field}`);
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
  const normalized = stableValue(record) as ActorConfigurationIr;
  return JSON.parse(
    stableActorConfigurationIrJson(normalized),
  ) as ActorConfigurationIr;
}

export function authoringContractToConfigurationIr(
  contract: ActorAuthoringContract,
): ActorConfigurationIr {
  return normalizeActorConfigurationIr({
    format: ACTORS_CONFIGURATION_IR_FORMAT,
    formatVersion: ACTORS_CONFIGURATION_IR_VERSION,
    actorType: contract.actorType,
    mutability: contract.mutability,
    completionPolicy: contract.completionPolicy,
    autoCloseAtCycleNonce: contract.autoCloseAtCycleNonce?.toString() ?? null,
    trigger: contract.trigger,
    cooldownBlocks: contract.cooldownBlocks,
    scheduleWindow: contract.scheduleWindow,
    fundingPolicy: contract.fundingPolicy,
    steps: contract.steps.map(({ precondition, task, errorPolicy }) => ({
      precondition,
      task,
      errorPolicy,
    })),
  });
}

export function configurationIrToAuthoringContract(
  value: unknown,
): ActorAuthoringContract {
  const ir = normalizeActorConfigurationIr(value);
  return {
    actorType: ir.actorType,
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
      precondition: step.precondition,
      task: step.task,
      errorPolicy: step.errorPolicy,
    })),
  };
}

export function diagnoseActorConfigurationIr(
  value: unknown,
  limits: ActorAuthoringLimits = DEOS_ACTORS_AUTHORING_LIMITS,
): ActorConfigurationIrDiagnostic[] {
  try {
    const validation = validateActorAuthoringContract(
      configurationIrToAuthoringContract(value),
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

export function lowerActorConfigurationIr(
  value: unknown,
  limits: ActorAuthoringLimits = DEOS_ACTORS_AUTHORING_LIMITS,
) {
  return lowerActorAuthoringContract(
    configurationIrToAuthoringContract(value),
    limits,
  );
}

function diffValues(
  before: unknown,
  after: unknown,
  path: string,
  output: ActorConfigurationIrDiff[],
) {
  if (
    stableActorConfigurationIrJson(before) ===
    stableActorConfigurationIrJson(after)
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

export function diffActorConfigurationIr(
  before: unknown,
  after: unknown,
): ActorConfigurationIrDiff[] {
  const output: ActorConfigurationIrDiff[] = [];
  diffValues(
    normalizeActorConfigurationIr(before),
    normalizeActorConfigurationIr(after),
    '',
    output,
  );
  return output;
}

export function serializeActorConfigurationJson(value: unknown) {
  return `${stableActorConfigurationIrJson(normalizeActorConfigurationIr(value), 2)}\n`;
}

export function parseActorConfigurationJson(source: string) {
  return normalizeActorConfigurationIr(JSON.parse(source));
}

export function serializeActorConfigurationToml(value: unknown) {
  const ir = normalizeActorConfigurationIr(value);
  const lines = [
    `format = ${JSON.stringify(ACTORS_CONFIGURATION_IR_FORMAT)}`,
    `format_version = ${ACTORS_CONFIGURATION_IR_VERSION}`,
    '',
    '[genome]',
  ];
  for (const field of FIELD_NAMES) {
    lines.push(
      `${field} = ${JSON.stringify(stableActorConfigurationIrJson(ir[field]))}`,
    );
  }
  return `${lines.join('\n')}\n`;
}

export function parseActorConfigurationToml(source: string) {
  const format = source.match(/^format\s*=\s*("(?:[^"\\]|\\.)*")\s*$/m);
  const version = source.match(/^format_version\s*=\s*(\d+)\s*$/m);
  if (format == null || version == null)
    throw new Error('Invalid Actors configuration TOML header');
  const record: Record<string, unknown> = {
    format: JSON.parse(format[1]),
    formatVersion: Number(version[1]),
  };
  for (const field of FIELD_NAMES) {
    const match = source.match(
      new RegExp(`^${field}\\s*=\\s*("(?:[^"\\\\]|\\\\.)*")\\s*$`, 'm'),
    );
    if (match == null)
      throw new Error(`Missing Actors configuration TOML field: ${field}`);
    record[field] = JSON.parse(JSON.parse(match[1]));
  }
  return normalizeActorConfigurationIr(record);
}

export function serializeActorConfigurationMarkdown(value: unknown) {
  const ir = normalizeActorConfigurationIr(value);
  const sections = FIELD_NAMES.map(
    (field) =>
      `## ${field}\n\n\`\`\`json\n${stableActorConfigurationIrJson(ir[field], 2)}\n\`\`\``,
  );
  return `# Actors Configuration Genome\n\n<!-- ${ACTORS_CONFIGURATION_IR_FORMAT}@${ACTORS_CONFIGURATION_IR_VERSION} -->\n\n${sections.join('\n\n')}\n`;
}

export function parseActorConfigurationMarkdown(source: string) {
  const marker = source.match(/<!--\s*([^@\s]+)@(\d+)\s*-->/);
  if (marker == null)
    throw new Error('Invalid Actors configuration Markdown header');
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
      throw new Error(`Missing Actors configuration Markdown field: ${field}`);
    record[field] = JSON.parse(match[1]);
  }
  return normalizeActorConfigurationIr(record);
}
