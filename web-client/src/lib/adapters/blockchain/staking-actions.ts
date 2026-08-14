/*
Domain: Blockchain staking write actions
Owns: Native staking and custody extrinsic builders delegated by the blockchain adapter facade.
Excludes: Generic transaction watching, staking read-model projection, and widget composition.
Zone: Transport write adapter internals; depends on transaction submit contract and DEOS PAPI snapshots.
*/
import type { DeosPapiConnection } from './deos';
import type { BlockchainTransactionSubmitter } from './transactions';

export class BlockchainStakingActions {
  constructor(
    private readonly ensurePapi: () => Promise<DeosPapiConnection>,
    private readonly submitSigned: BlockchainTransactionSubmitter['submitSigned'],
    private readonly missingSignerMessage: () => string,
  ) {}

  async lockNativeLpForCollator(
    amount: bigint,
    operator: string,
  ): Promise<void> {
    await this.requireNativeSecurityCapability(
      'new_nominations',
      'Native LP collator nomination',
    );
    if (amount <= 0n) {
      throw new Error('LP lock amount must be greater than zero');
    }
    const normalizedOperator = operator.trim();
    if (normalizedOperator.length === 0) {
      throw new Error('Collator/operator address is required');
    }
    try {
      const lpAssetId = await this.resolveNativeStakingLpAssetId();
      await this.submitSigned(
        (snapshot, _accountId, signer) =>
          snapshot.typedApi.tx.Staking.lock_native_lp_for_collator({
            lp_asset_id: lpAssetId,
            amount,
            operator: normalizedOperator,
          }).signSubmitAndWatch(signer.signer),
        this.missingSignerMessage(),
        'Lock native staking LP',
      );
    } catch (error) {
      throw new Error(
        error instanceof Error
          ? error.message
          : 'Native LP collator lock failed',
      );
    }
  }

  async claimNativeSecurityReward(epoch: number): Promise<void> {
    await this.requireNativeSecurityCapability(
      'reward_claims',
      'Native security reward claim',
    );
    this.requireSecurityEpoch(epoch);
    await this.submitSigned(
      (snapshot, _accountId, signer) =>
        snapshot.typedApi.tx.Staking.claim_native_security_reward({
          epoch,
        }).signSubmitAndWatch(signer.signer),
      this.missingSignerMessage(),
      'Claim native security reward',
    );
  }

  async claimAndCompoundNativeSecurityReward(
    epoch: number,
    operator: string,
    minLpOut: bigint,
  ): Promise<void> {
    await this.requireNativeSecurityCapability(
      'reward_compound',
      'Native security reward compound',
    );
    this.requireSecurityEpoch(epoch);
    if (minLpOut <= 0n) {
      throw new Error('Minimum LP output must be greater than zero');
    }
    const normalizedOperator = operator.trim();
    if (normalizedOperator.length === 0) {
      throw new Error('Collator/operator address is required');
    }
    await this.submitSigned(
      (snapshot, _accountId, signer) =>
        snapshot.typedApi.tx.Staking.claim_and_compound_native_security_reward({
          epoch,
          operator: normalizedOperator,
          min_lp_out: minLpOut,
        }).signSubmitAndWatch(signer.signer),
      this.missingSignerMessage(),
      'Claim and compound native security reward',
    );
  }

  async requestUnlockNativeLp(operator: string, amount: bigint): Promise<void> {
    if (amount <= 0n) {
      throw new Error('LP unlock amount must be greater than zero');
    }
    const normalizedOperator = operator.trim();
    if (normalizedOperator.length === 0) {
      throw new Error('Collator/operator address is required');
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) =>
          snapshot.typedApi.tx.Staking.request_unlock_native_lp({
            operator: normalizedOperator,
            amount,
          }).signSubmitAndWatch(signer.signer),
        this.missingSignerMessage(),
        'Request native LP unlock',
      );
    } catch (error) {
      throw new Error(
        error instanceof Error
          ? error.message
          : 'Native LP unlock request failed',
      );
    }
  }

  async withdrawUnlockedNativeLp(operator: string): Promise<void> {
    const normalizedOperator = operator.trim();
    if (normalizedOperator.length === 0) {
      throw new Error('Collator/operator address is required');
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) =>
          snapshot.typedApi.tx.Staking.withdraw_unlocked_native_lp({
            operator: normalizedOperator,
          }).signSubmitAndWatch(signer.signer),
        this.missingSignerMessage(),
        'Withdraw unlocked native LP',
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : 'Native LP withdrawal failed',
      );
    }
  }

  async redelegateNativeLp(
    fromOperator: string,
    toOperator: string,
    amount: bigint,
  ): Promise<void> {
    await this.requireNativeSecurityCapability(
      'redelegation',
      'Native LP redelegation',
    );
    if (amount <= 0n) {
      throw new Error('LP redelegation amount must be greater than zero');
    }
    const normalizedFromOperator = fromOperator.trim();
    const normalizedToOperator = toOperator.trim();
    if (
      normalizedFromOperator.length === 0 ||
      normalizedToOperator.length === 0
    ) {
      throw new Error(
        'Both source and target collator/operator addresses are required',
      );
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) =>
          snapshot.typedApi.tx.Staking.redelegate_native_lp({
            from_operator: normalizedFromOperator,
            to_operator: normalizedToOperator,
            amount,
          }).signSubmitAndWatch(signer.signer),
        this.missingSignerMessage(),
        'Redelegate native LP',
      );
    } catch (error) {
      throw new Error(
        error instanceof Error
          ? error.message
          : 'Native LP redelegation failed',
      );
    }
  }

  async lockNativeLpForGovernance(amount: bigint): Promise<void> {
    if (amount <= 0n) {
      throw new Error('Governance LP lock amount must be greater than zero');
    }
    try {
      const lpAssetId = await this.resolveNativeStakingLpAssetId();
      await this.submitSigned(
        (snapshot, _accountId, signer) =>
          snapshot.typedApi.tx.Staking.lock_native_lp_for_governance({
            lp_asset_id: lpAssetId,
            amount,
          }).signSubmitAndWatch(signer.signer),
        this.missingSignerMessage(),
        'Lock governance native LP',
      );
    } catch (error) {
      throw new Error(
        error instanceof Error
          ? error.message
          : 'Native governance LP lock failed',
      );
    }
  }

  async requestUnlockNativeLpForGovernance(amount: bigint): Promise<void> {
    if (amount <= 0n) {
      throw new Error('Governance LP unlock amount must be greater than zero');
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) =>
          snapshot.typedApi.tx.Staking.request_unlock_native_lp_for_governance({
            amount,
          }).signSubmitAndWatch(signer.signer),
        this.missingSignerMessage(),
        'Request governance LP unlock',
      );
    } catch (error) {
      throw new Error(
        error instanceof Error
          ? error.message
          : 'Native governance LP unlock request failed',
      );
    }
  }

  async withdrawUnlockedNativeLpForGovernance(): Promise<void> {
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) =>
          snapshot.typedApi.tx.Staking.withdraw_unlocked_native_lp_for_governance().signSubmitAndWatch(
            signer.signer,
          ),
        this.missingSignerMessage(),
        'Withdraw governance LP',
      );
    } catch (error) {
      throw new Error(
        error instanceof Error
          ? error.message
          : 'Native governance LP withdrawal failed',
      );
    }
  }

  async lockNativeAssetForGovernance(
    assetId: number,
    amount: bigint,
  ): Promise<void> {
    this.requireFiniteAssetId(assetId);
    if (amount <= 0n) {
      throw new Error('Governance asset lock amount must be greater than zero');
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) =>
          snapshot.typedApi.tx.Staking.lock_native_asset_for_governance({
            asset_id: assetId,
            amount,
          }).signSubmitAndWatch(signer.signer),
        this.missingSignerMessage(),
        'Lock governance native asset',
      );
    } catch (error) {
      throw new Error(
        error instanceof Error
          ? error.message
          : 'Native governance asset lock failed',
      );
    }
  }

  async requestUnlockNativeAssetForGovernance(
    assetId: number,
    amount: bigint,
  ): Promise<void> {
    this.requireFiniteAssetId(assetId);
    if (amount <= 0n) {
      throw new Error(
        'Governance asset unlock amount must be greater than zero',
      );
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) =>
          snapshot.typedApi.tx.Staking.request_unlock_native_asset_for_governance(
            {
              asset_id: assetId,
              amount,
            },
          ).signSubmitAndWatch(signer.signer),
        this.missingSignerMessage(),
        'Request governance asset unlock',
      );
    } catch (error) {
      throw new Error(
        error instanceof Error
          ? error.message
          : 'Native governance asset unlock request failed',
      );
    }
  }

  async withdrawUnlockedNativeAssetForGovernance(
    assetId: number,
  ): Promise<void> {
    this.requireFiniteAssetId(assetId);
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) =>
          snapshot.typedApi.tx.Staking.withdraw_unlocked_native_asset_for_governance(
            {
              asset_id: assetId,
            },
          ).signSubmitAndWatch(signer.signer),
        this.missingSignerMessage(),
        'Withdraw governance asset',
      );
    } catch (error) {
      throw new Error(
        error instanceof Error
          ? error.message
          : 'Native governance asset withdrawal failed',
      );
    }
  }

  private async requireNativeSecurityCapability(
    capability:
      | 'new_nominations'
      | 'redelegation'
      | 'reward_claims'
      | 'reward_compound',
    action: string,
  ): Promise<void> {
    const snapshot = await (await this.ensurePapi()).snapshot();
    const capabilities =
      await snapshot.typedApi.view.Staking.native_security_capabilities({
        at: snapshot.at,
      });
    if (!capabilities[capability]) {
      throw new Error(`${action} is inactive in the current security mode`);
    }
  }

  private async resolveNativeStakingLpAssetId(): Promise<number> {
    const snapshot = await (await this.ensurePapi()).snapshot();
    const pool =
      await snapshot.typedApi.view.Staking.native_staking_liquidity_pool({
        at: snapshot.at,
      });
    if (!pool) {
      throw new Error('Canonical NTVE/stNTVE liquidity pool is unavailable');
    }
    return pool.lp_asset_id;
  }

  private requireSecurityEpoch(epoch: number): void {
    if (!Number.isSafeInteger(epoch) || epoch < 0) {
      throw new Error('Security epoch must be a non-negative integer');
    }
  }

  private requireFiniteAssetId(assetId: number): void {
    if (!Number.isFinite(assetId)) {
      throw new Error('Governance asset id is required');
    }
  }
}
