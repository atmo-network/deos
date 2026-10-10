/*
Domain: Governance Invoice transport projection
Owns: Strict current-schema PAPI Invoice receipt projection; unsupported shapes become unavailable.
Excludes: SCALE decoding, metadata issuance, ledger policy and legacy schema aliases.
Zone: Governance adapter transport leaf; consumes domain contracts only.
*/
import type { GovernanceProposalExecutionSuccessDetail } from '$lib/governance/types';

type InvoiceReceipt = Extract<
  GovernanceProposalExecutionSuccessDetail,
  { kind: 'TreasurySpendExecuted' }
>;
function record(value: unknown): value is Record<string, unknown> {
  return value != null && typeof value === 'object';
}

export function mapInvoiceExecutionReceipt(
  value: unknown,
): InvoiceReceipt | null {
  if (
    !record(value) ||
    !(value.invoice_cid instanceof Uint8Array) ||
    typeof value.treasury_account !== 'string' ||
    typeof value.beneficiary !== 'string' ||
    typeof value.payout_asset !== 'number' ||
    !Number.isInteger(value.payout_asset) ||
    value.payout_asset < 0 ||
    value.payout_asset > 0xffff_ffff ||
    typeof value.base_amount !== 'bigint' ||
    typeof value.target_amount !== 'bigint' ||
    typeof value.actual_amount !== 'bigint' ||
    typeof value.capacity_limited !== 'boolean' ||
    !record(value.scalar) ||
    !record(value.settlement_kind)
  )
    return null;
  const scalar = value.scalar.type;
  if (
    (scalar !== 'Amplify' && scalar !== 'Approve' && scalar !== 'Reduce') ||
    value.settlement_kind.type !== 'InvoiceScalarTransfer'
  )
    return null;
  try {
    const invoiceCid = new TextDecoder('utf-8', { fatal: true }).decode(
      value.invoice_cid,
    );
    if (invoiceCid.length === 0) return null;
    return {
      kind: 'TreasurySpendExecuted',
      invoiceCid,
      treasuryAccount: value.treasury_account,
      beneficiary: value.beneficiary,
      payoutAsset: value.payout_asset,
      baseAmount: value.base_amount,
      scalar,
      targetAmount: value.target_amount,
      actualAmount: value.actual_amount,
      capacityLimited: value.capacity_limited,
      settlementKind: 'InvoiceScalarTransfer',
    };
  } catch {
    return null;
  }
}
