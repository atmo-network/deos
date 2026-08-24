/*
Domain: Actors cost projection
Owns: Fail-closed browser projection of named ActorCostApi quotes and ActionFeeCharged receipts.
Excludes: Chain transport, fee estimation, transaction submission, and historical event indexing.
Zone: Automation public contract; adapters and widgets consume these names without recombining fee owners.
*/
export const ACTORS_COST_RUNTIME_API = 'ActorCostApi_actor_cost_quote' as const;
export const ACTORS_COST_RUNTIME_API_VERSION = 1 as const;

export type ActorCostFailure =
  | 'ActorNotFound'
  | 'ActorInvariant'
  | 'ComputationOverflow'
  | 'WeightAuthorityUnavailable';

export type ActorCostWeight = {
  refTime: bigint;
  proofSize: bigint;
};

export type ActorTriggerFeeView = {
  family:
    | 'Manual'
    | 'AddressEvent'
    | 'ObservationChange'
    | 'ObservationCrossing'
    | 'AtTime'
    | 'Cadenced';
  maximumWeight: ActorCostWeight;
  fee: bigint;
  productionWeightIdentity: string;
};

export type ActorPipelineFeeView = {
  machineFee: bigint;
  cleanupFee: bigint;
  totalFee: bigint;
  strategy: 'UpfrontBounded';
  admissionIdentity: string;
  productionWeightIdentity: string;
};

export type ActorActionFeeView = {
  maximumEffectWeight: ActorCostWeight;
  maximumEffectFee: bigint;
  productionWeightIdentity: string;
};

export type ActorStateHoldView = {
  exempt: boolean;
  basePerComponent: bigint;
  perEncodedByte: bigint;
  components: {
    identity: bigint;
    contractHead: bigint;
    contractBody: bigint;
    detector: bigint;
    funding: bigint;
    run: bigint;
  };
  total: bigint;
};

export type ActorCostView = {
  actorType: 'User' | 'System';
  creationFee: bigint;
  prospectiveTriggerFee: ActorTriggerFeeView | null;
  prospectivePipelineFee: ActorPipelineFeeView | null;
  maximumNextActionFee: ActorActionFeeView;
  stateHold: ActorStateHoldView;
};

export type ActorActionFeeReceipt = {
  actorId: bigint;
  cycleNonce: bigint;
  stepIndex: number;
  actualEffectWeight: ActorCostWeight;
  fee: bigint;
};

const COST_FAILURES: ReadonlySet<string> = new Set([
  'ActorNotFound',
  'ActorInvariant',
  'ComputationOverflow',
  'WeightAuthorityUnavailable',
]);

const TRIGGER_FAMILIES: ReadonlySet<string> = new Set([
  'Manual',
  'AddressEvent',
  'ObservationChange',
  'ObservationCrossing',
  'AtTime',
  'Cadenced',
]);

function asRecord(value: unknown, field: string): Record<string, unknown> {
  if (value == null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${field} must be a runtime object`);
  }
  return value as Record<string, unknown>;
}

function asVariant(value: unknown, field: string) {
  const variant = asRecord(value, field);
  if (typeof variant.type !== 'string' || variant.type.length === 0) {
    throw new Error(`${field} must carry a runtime variant type`);
  }
  return variant as Record<string, unknown> & { type: string; value?: unknown };
}

function asUnsigned(value: unknown, field: string): bigint {
  if (typeof value === 'bigint' && value >= 0n) return value;
  if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) {
    return BigInt(value);
  }
  throw new Error(`${field} must be an unsigned runtime integer`);
}

function asStepIndex(value: unknown): number {
  const index = asUnsigned(value, 'Action fee step index');
  const projected = Number(index);
  if (!Number.isSafeInteger(projected) || index >= 1n << 32n) {
    throw new Error('Action fee step index must fit u32');
  }
  return projected;
}

function asWeight(value: unknown, field: string): ActorCostWeight {
  const weight = asRecord(value, field);
  return {
    refTime: asUnsigned(weight.ref_time, `${field}.ref_time`),
    proofSize: asUnsigned(weight.proof_size, `${field}.proof_size`),
  };
}

function bytesOf(value: unknown, field: string): Uint8Array {
  if (value instanceof Uint8Array) return value;
  if (Array.isArray(value) && value.every((byte) => Number.isInteger(byte))) {
    return Uint8Array.from(value as number[]);
  }
  if (value != null && typeof value === 'object') {
    const asBytes = (value as { asBytes?: unknown }).asBytes;
    if (typeof asBytes === 'function') {
      const bytes = asBytes.call(value);
      if (bytes instanceof Uint8Array) return bytes;
    }
  }
  throw new Error(`${field} must be runtime bytes`);
}

function asHash(value: unknown, field: string): string {
  const bytes = bytesOf(value, field);
  if (bytes.length !== 32) throw new Error(`${field} must contain 32 bytes`);
  return `0x${Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')}`;
}

function projectTrigger(value: unknown): ActorTriggerFeeView {
  const trigger = asRecord(value, 'prospective Trigger fee');
  const family = asVariant(trigger.trigger_family, 'Trigger family').type;
  if (!TRIGGER_FAMILIES.has(family)) {
    throw new Error(`Unsupported runtime Trigger family ${family}`);
  }
  return {
    family: family as ActorTriggerFeeView['family'],
    maximumWeight: asWeight(trigger.maximum_weight, 'Trigger maximum Weight'),
    fee: asUnsigned(trigger.fee, 'Trigger fee'),
    productionWeightIdentity: asHash(
      trigger.production_weight_identity,
      'Trigger production Weight identity',
    ),
  };
}

function projectPipeline(value: unknown): ActorPipelineFeeView {
  const pipeline = asRecord(value, 'prospective Pipeline fee');
  const strategy = asVariant(pipeline.strategy, 'Pipeline strategy').type;
  if (strategy !== 'UpfrontBounded') {
    throw new Error(`Unsupported runtime Pipeline strategy ${strategy}`);
  }
  const machineFee = asUnsigned(
    pipeline.pipeline_machine_fee,
    'Pipeline Machine fee',
  );
  const cleanupFee = asUnsigned(pipeline.cleanup_fee, 'Pipeline cleanup fee');
  const totalFee = asUnsigned(pipeline.total_fee, 'Pipeline total fee');
  if (machineFee + cleanupFee !== totalFee) {
    throw new Error('Pipeline total must equal Machine plus cleanup fees');
  }
  return {
    machineFee,
    cleanupFee,
    totalFee,
    strategy,
    admissionIdentity: asHash(
      pipeline.admission_identity,
      'Pipeline admission identity',
    ),
    productionWeightIdentity: asHash(
      pipeline.production_weight_identity,
      'Pipeline production Weight identity',
    ),
  };
}

function projectAction(value: unknown): ActorActionFeeView {
  const action = asRecord(value, 'maximum next Action fee');
  return {
    maximumEffectWeight: asWeight(
      action.maximum_effect_weight,
      'Action maximum effect Weight',
    ),
    maximumEffectFee: asUnsigned(
      action.maximum_effect_fee,
      'Action maximum effect fee',
    ),
    productionWeightIdentity: asHash(
      action.production_weight_identity,
      'Action production Weight identity',
    ),
  };
}

function projectStateHold(value: unknown): ActorStateHoldView {
  const hold = asRecord(value, 'Actor state hold');
  if (typeof hold.exempt !== 'boolean') {
    throw new Error('Actor state hold exemption must be boolean');
  }
  const breakdown = asRecord(hold.breakdown, 'Actor state hold breakdown');
  const components = {
    identity: asUnsigned(breakdown.identity, 'identity hold'),
    contractHead: asUnsigned(breakdown.contract_head, 'Contract head hold'),
    contractBody: asUnsigned(breakdown.contract_body, 'Contract body hold'),
    detector: asUnsigned(breakdown.detector, 'detector hold'),
    funding: asUnsigned(breakdown.funding, 'funding hold'),
    run: asUnsigned(breakdown.run, 'run hold'),
  };
  const total = asUnsigned(hold.total, 'Actor state hold total');
  const componentTotal = Object.values(components).reduce(
    (sum, component) => sum + component,
    0n,
  );
  if (componentTotal !== total) {
    throw new Error('Actor state hold total must equal its named components');
  }
  if (hold.exempt && total !== 0n) {
    throw new Error('State-hold-exempt Actor must report zero held total');
  }
  return {
    exempt: hold.exempt,
    basePerComponent: asUnsigned(
      hold.base_per_component,
      'state hold base per component',
    ),
    perEncodedByte: asUnsigned(
      hold.per_encoded_byte,
      'state hold price per encoded byte',
    ),
    components,
    total,
  };
}

export function projectActorCostQuote(value: unknown): ActorCostView {
  const result = asRecord(value, 'runtime Result');
  if (result.success === false) {
    const failure = asVariant(result.value, 'Actor cost error').type;
    if (!COST_FAILURES.has(failure)) {
      throw new Error(`Unsupported runtime Actor cost error ${failure}`);
    }
    throw new Error(
      `Runtime Actor cost quote rejected: ${failure as ActorCostFailure}`,
    );
  }
  if (result.success !== true) {
    throw new Error('Runtime Actor cost output must be a SCALE Result');
  }
  const quote = asRecord(result.value, 'Actor cost quote');
  const actorType = asVariant(quote.actor_type, 'Actor type').type;
  if (actorType !== 'User' && actorType !== 'System') {
    throw new Error(`Unsupported runtime Actor type ${actorType}`);
  }
  return {
    actorType,
    creationFee: asUnsigned(quote.creation_fee, 'Actor Creation Fee'),
    prospectiveTriggerFee:
      quote.prospective_trigger_fee === undefined
        ? null
        : projectTrigger(quote.prospective_trigger_fee),
    prospectivePipelineFee:
      quote.prospective_pipeline_fee === undefined
        ? null
        : projectPipeline(quote.prospective_pipeline_fee),
    maximumNextActionFee: projectAction(quote.maximum_next_action_fee),
    stateHold: projectStateHold(quote.actor_state_hold),
  };
}

export function projectActorActionFeeReceipt(
  value: unknown,
): ActorActionFeeReceipt {
  const receipt = asRecord(value, 'ActionFeeCharged event');
  return {
    actorId: asUnsigned(receipt.actor_id, 'Action fee actor id'),
    cycleNonce: asUnsigned(receipt.cycle_nonce, 'Action fee cycle nonce'),
    stepIndex: asStepIndex(receipt.step_index),
    actualEffectWeight: asWeight(
      receipt.actual_effect_weight,
      'Action actual effect Weight',
    ),
    fee: asUnsigned(receipt.fee, 'actual Action fee'),
  };
}
