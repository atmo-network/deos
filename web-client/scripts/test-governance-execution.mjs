/*
Domain: Governance execution presentation validation
Owns: Invoice source-codec vectors, capability refusal, typed failures and finalized receipt projection.
Excludes: Metadata issuance, chain SCALE decoding, RPC, submission and rendered UI evidence.
*/
import { base32Encode } from '@polkadot/util-crypto';
import assert from 'node:assert/strict';
import test from 'node:test';
import { Binary } from 'polkadot-api';

import { mapInvoiceExecutionReceipt } from '../src/lib/adapters/governance/invoice-receipt.ts';
import {
  executionFailureReasonLabel,
  finalizedExecutionDetailRows,
} from '../src/lib/governance/labels.ts';
import { deriveGovernanceTreasuryPayloadDraftState } from '../src/lib/governance/treasury-payload.ts';

test('treasury minimum failure is explicit and does not disguise generic dispatch refusal', () => {
  const reason = 'InsufficientTreasuryCapacity';
  const label = 'Insufficient treasury capacity · no payout';
  assert.equal(executionFailureReasonLabel(reason), label);
  const rows = finalizedExecutionDetailRows({ kind: 'Failed', reason });
  assert.equal(
    rows.find((row) => row.label === 'Failure reason')?.value,
    label,
  );
  assert.ok(
    rows
      .find((row) => row.label === 'Execution receipt')
      ?.value.includes(label),
  );
  assert.equal(
    executionFailureReasonLabel('DispatchFailed'),
    'Dispatch failed',
  );
  assert.equal(
    executionFailureReasonLabel('MissingWinningPrimaryOption'),
    'Missing winning primary option',
  );
  assert.equal(
    executionFailureReasonLabel('FutureUnknownFailure'),
    'Unavailable',
  );
  assert.deepEqual(finalizedExecutionDetailRows(null), []);
});

const cid = 'bafkreiaha4dqobyha4dqobyha4dqobyha4dqobyha4dqobyha4dqobyha4';
const draft = {
  invoiceCid: cid,
  treasuryAccount: '0x' + '11'.repeat(32),
  beneficiary: '0x' + '22'.repeat(32),
  payoutAsset: '42',
  baseAmount: '100',
};

test('Invoice SCALE source vector agrees with native CID/account/beneficiary/asset/amount order', () => {
  const encoded = deriveGovernanceTreasuryPayloadDraftState(draft, 96).encoding;
  assert.ok(encoded);
  assert.equal(
    encoded.payloadHex,
    '0xec' +
      Buffer.from(cid).toString('hex') +
      '11'.repeat(32) +
      '22'.repeat(32) +
      '2a000000' +
      '64' +
      '00'.repeat(15),
  );
  assert.equal(encoded.payloadByteLength, 144);
  assert.ok(!('fundingSource' in encoded));
  const longCid = base32Encode(
    Uint8Array.from([1, 85, 0, 36, ...Array(36).fill(7)]),
    true,
  );
  const long = deriveGovernanceTreasuryPayloadDraftState(
    { ...draft, invoiceCid: longCid },
    96,
  ).encoding;
  assert.equal(longCid.length, 65);
  assert.deepEqual([...long.payloadBytes.slice(0, 2)], [5, 1]);
  const maxCid = base32Encode(
    Uint8Array.from([1, 85, 0, 55, ...Array(55).fill(7)]),
    true,
  );
  assert.equal(maxCid.length, 96);
  assert.equal(
    deriveGovernanceTreasuryPayloadDraftState(
      { ...draft, invoiceCid: maxCid },
      96,
    ).encoding.payloadByteLength,
    182,
  );
});

test('Invoice authoring fails closed without runtime CID capability and rejects malformed drafts', () => {
  for (const bound of [null, 0, -1, Infinity, NaN, 2 ** 32]) {
    const state = deriveGovernanceTreasuryPayloadDraftState(draft, bound);
    assert.equal(state.capabilityReady, false);
    assert.equal(state.encoding, null);
  }
  assert.ok(
    deriveGovernanceTreasuryPayloadDraftState(draft, cid.length).encoding,
  );
  assert.equal(
    deriveGovernanceTreasuryPayloadDraftState(draft, cid.length - 1).encoding,
    null,
  );
  for (const invoiceCid of [
    '',
    'ipfs://' + cid,
    cid.toUpperCase(),
    cid + '=',
    ' ' + cid,
    'baf',
  ]) {
    assert.equal(
      deriveGovernanceTreasuryPayloadDraftState({ ...draft, invoiceCid }, 96)
        .encoding,
      null,
    );
  }
  for (const change of [
    { treasuryAccount: 'bad' },
    { beneficiary: '0x01' },
    { payoutAsset: '4294967296' },
    { baseAmount: '0' },
    { baseAmount: (1n << 128n).toString() },
  ]) {
    assert.equal(
      deriveGovernanceTreasuryPayloadDraftState({ ...draft, ...change }, 96)
        .encoding,
      null,
    );
  }
});

function rawReceipt() {
  return {
    invoice_cid: Binary.fromText(cid),
    treasury_account: draft.treasuryAccount,
    beneficiary: draft.beneficiary,
    payout_asset: 42,
    base_amount: 100n,
    scalar: { type: 'Amplify' },
    target_amount: 200n,
    actual_amount: 150n,
    capacity_limited: true,
    settlement_kind: { type: 'InvoiceScalarTransfer' },
  };
}

test('Invoice transport and rows preserve target, actual, capacity status and source identity', () => {
  const detail = mapInvoiceExecutionReceipt(rawReceipt());
  assert.deepEqual(detail, {
    kind: 'TreasurySpendExecuted',
    invoiceCid: cid,
    treasuryAccount: draft.treasuryAccount,
    beneficiary: draft.beneficiary,
    payoutAsset: 42,
    baseAmount: 100n,
    scalar: 'Amplify',
    targetAmount: 200n,
    actualAmount: 150n,
    capacityLimited: true,
    settlementKind: 'InvoiceScalarTransfer',
  });
  const rows = finalizedExecutionDetailRows({ kind: 'Succeeded', detail });
  assert.equal(rows.find((row) => row.label === 'Target amount')?.value, '200');
  assert.equal(rows.find((row) => row.label === 'Actual payout')?.value, '150');
  assert.equal(rows.find((row) => row.label === 'Invoice CID')?.value, cid);
  assert.match(
    rows.find((row) => row.label === 'Capacity limited')?.value,
    /^Yes/,
  );
  assert.ok(!('finalAmount' in detail));
  const full = mapInvoiceExecutionReceipt({
    ...rawReceipt(),
    actual_amount: 200n,
    capacity_limited: false,
  });
  assert.equal(full.capacityLimited, false);
  assert.equal(full.actualAmount, 200n);
});

test('Old or incomplete receipt schemas are unavailable, never relabelled as current payouts', () => {
  assert.equal(
    mapInvoiceExecutionReceipt({
      funding_source: draft.treasuryAccount,
      final_amount: 150n,
    }),
    null,
  );
  for (const field of [
    'invoice_cid',
    'treasury_account',
    'target_amount',
    'actual_amount',
    'capacity_limited',
  ]) {
    const value = rawReceipt();
    delete value[field];
    assert.equal(mapInvoiceExecutionReceipt(value), null);
  }
  assert.equal(
    mapInvoiceExecutionReceipt({ ...rawReceipt(), capacity_limited: 'false' }),
    null,
  );
  assert.equal(
    mapInvoiceExecutionReceipt({
      ...rawReceipt(),
      scalar: { type: 'FutureScalar' },
    }),
    null,
  );
  assert.equal(
    mapInvoiceExecutionReceipt({
      ...rawReceipt(),
      invoice_cid: Uint8Array.of(0xff),
    }),
    null,
  );
});
