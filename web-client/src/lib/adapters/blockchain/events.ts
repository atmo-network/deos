/*
Domain: Blockchain event formatting
Owns: Chain event labels, messages, dispatch-error text, transaction highlights, and log-entry classification.
Excludes: Log store ownership, network polling, transaction watch lifecycle, and widget rendering.
Zone: Transport formatting helper; may depend on log contracts but not stores or UI components.
*/
import { PRECISION } from '$lib/economics';
import type { LogEntry } from '$lib/log/types';

type ChainEventValue = {
  type?: string;
  value?: object;
};

type ChainEvent = {
  type?: string;
  value?: ChainEventValue;
};

function asObject(value: unknown): object | null {
  return typeof value === 'object' && value !== null ? value : null;
}

function objectProperty(
  value: object | null | undefined,
  key: string,
): unknown {
  return value == null ? undefined : Reflect.get(value, key);
}

function asChainEvent(value: unknown): ChainEvent {
  const event = asObject(value);
  const eventType = objectProperty(event, 'type');
  const eventValue = asObject(objectProperty(event, 'value'));
  const eventValueType = objectProperty(eventValue, 'type');
  return {
    type: typeof eventType === 'string' ? eventType : undefined,
    value: {
      type: typeof eventValueType === 'string' ? eventValueType : undefined,
      value: asObject(objectProperty(eventValue, 'value')) ?? undefined,
    },
  };
}

function eventPayload(event: ChainEvent): object {
  return event.value?.value ?? {};
}

function formatUnknownAmount(value: unknown): string {
  return typeof value === 'bigint' ? formatAmount(value) : 'unavailable amount';
}

function formatUnknownAsset(value: unknown): string {
  const asset = asObject(value);
  const assetType = objectProperty(asset, 'type');
  return typeof assetType === 'string' ? assetType : 'asset';
}

export function formatAmount(value: bigint): string {
  return (Number(value) / Number(PRECISION)).toLocaleString(undefined, {
    maximumFractionDigits: 6,
  });
}

export function describeDispatchError(
  dispatchError: { type?: string; value?: unknown } | null | undefined,
): string | null {
  if (!dispatchError?.type) {
    return null;
  }
  if (
    typeof dispatchError.value === 'string' &&
    dispatchError.value.length > 0
  ) {
    return `${dispatchError.type}: ${dispatchError.value}`;
  }
  if (
    typeof dispatchError.value === 'object' &&
    dispatchError.value !== null &&
    'type' in dispatchError.value &&
    typeof dispatchError.value.type === 'string'
  ) {
    return `${dispatchError.type}: ${dispatchError.value.type}`;
  }
  return dispatchError.type;
}

export function unwrapEventRecord(record: unknown): unknown {
  const candidate = asObject(record);
  if (candidate && 'event' in candidate) {
    return objectProperty(candidate, 'event');
  }
  return record;
}

export function formatChainEventLabel(eventInput: unknown): string {
  const event = asChainEvent(eventInput);
  return `${event.type ?? 'Runtime'}.${event.value?.type ?? 'Event'}`;
}

export function formatChainEventMessage(eventInput: unknown): string {
  const event = asChainEvent(eventInput);
  const payload = eventPayload(event);
  if (event.type === 'AxialRouter' && event.value?.type === 'SwapExecuted') {
    const mechanism = asObject(objectProperty(payload, 'mechanism'));
    const mechanismType = objectProperty(mechanism, 'type');
    return `Swap ${typeof mechanismType === 'string' ? mechanismType : 'route'} · in ${formatUnknownAmount(objectProperty(payload, 'amount_in'))} · out ${formatUnknownAmount(objectProperty(payload, 'amount_out'))}`;
  }
  if (event.type === 'Balances' && event.value?.type === 'Transfer') {
    return `Native transfer ${formatUnknownAmount(objectProperty(payload, 'amount'))}`;
  }
  if (event.type === 'Assets' && event.value?.type === 'Transferred') {
    return `Asset ${String(objectProperty(payload, 'asset_id') ?? 'unknown')} transfer ${formatUnknownAmount(objectProperty(payload, 'amount'))}`;
  }
  return formatChainEventLabel(event);
}

export function classifyChainEvent(eventInput: unknown): LogEntry['type'] {
  const event = asChainEvent(eventInput);
  if (event.type === 'System' && event.value?.type === 'ExtrinsicFailed') {
    return 'error';
  }
  return 'info';
}

export function buildTransactionHighlights(
  events: unknown[] | undefined,
): string[] {
  if (!events || events.length === 0) {
    return [];
  }
  const highlights: string[] = [];
  for (const eventInput of events) {
    const event = asChainEvent(eventInput);
    const payload = eventPayload(event);
    if (event.type === 'AxialRouter' && event.value?.type === 'SwapExecuted') {
      const mechanism = asObject(objectProperty(payload, 'mechanism'));
      const mechanismType = objectProperty(mechanism, 'type');
      highlights.push(
        `Swap ${typeof mechanismType === 'string' ? mechanismType : 'route'} · in ${formatUnknownAmount(objectProperty(payload, 'amount_in'))} · out ${formatUnknownAmount(objectProperty(payload, 'amount_out'))}`,
      );
      continue;
    }
    if (event.type === 'AxialRouter' && event.value?.type === 'FeeCollected') {
      highlights.push(
        `Router fee ${formatUnknownAmount(objectProperty(payload, 'amount'))} ${formatUnknownAsset(objectProperty(payload, 'asset'))}`,
      );
      continue;
    }
    if (event.type === 'Balances' && event.value?.type === 'Transfer') {
      highlights.push(
        `Native transfer ${formatUnknownAmount(objectProperty(payload, 'amount'))}`,
      );
      continue;
    }
    if (event.type === 'Assets' && event.value?.type === 'Transferred') {
      highlights.push(
        `Asset ${String(objectProperty(payload, 'asset_id') ?? 'unknown')} transfer ${formatUnknownAmount(objectProperty(payload, 'amount'))}`,
      );
      continue;
    }
    if (
      event.type === 'TransactionPayment' &&
      event.value?.type === 'TransactionFeePaid'
    ) {
      highlights.push(
        `Tx fee ${formatUnknownAmount(objectProperty(payload, 'actual_fee'))}`,
      );
    }
  }
  return highlights.slice(0, 4);
}
