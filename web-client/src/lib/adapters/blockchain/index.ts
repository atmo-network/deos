/*
Domain: Concrete blockchain adapter facade
Owns: Public chain-backed Adapter implementation and delegation across focused blockchain adapter helpers.
Excludes: Wallet store ownership, system composition wiring, UI Kit presentation, and domain store state.
Zone: Transport adapter boundary; consumes adapter contracts and runtime helpers without importing widgets.
*/
import type { Adapter, AdapterRuntimeContext } from '$lib/adapters/contract';
import type {
  AutomationActorSnapshot,
  AutomationAuthoringContext,
  AutomationContinuationSnapshot,
  AutomationRunState,
} from '$lib/automation/types';
import { PRECISION } from '$lib/economics';
import type { LogEntry, TransactionProgress } from '$lib/log/types';
import type { PricePoint, Quote, SwapResult } from '$lib/market/types';
import { compareObservationRuntimeEvidence } from '$lib/observation/runtime-evidence';
import type {
  ObservationFanoutEvidence,
  ObservationFeedIdentity,
} from '$lib/observation/types';
import type {
  AssetBalanceProjection,
  TransferAssetKey,
} from '$lib/portfolio/types';
import type {
  NativeCollatorLpPositionProjection,
  NativeGovernanceCustodyPositionProjection,
} from '$lib/staking/types';
import type { SystemConfig, SystemSnapshot } from '$lib/system/types';
import { DEFAULT_DEOS_DAPP_NAME } from '$lib/wallet/signer';

import { readActorEligibility } from './actor-eligibility';
import { getDeosActorFinalizedAuthoringContext } from './actor-simulation';
import { BlockchainConnectionSession } from './connection';
import type {
  DeosChainConnectionState,
  DeosChainSnapshot,
  DeosPapiConnection,
} from './deos';
import {
  classifyChainEvent,
  formatChainEventLabel,
  formatChainEventMessage,
  unwrapEventRecord,
} from './events';
import { BlockchainObservationReader } from './observations';
import {
  quoteBuyAtSnapshot,
  quoteSellAtSnapshot,
  tmcMintQuote,
} from './quotes';
import {
  KNOWN_SYSTEM_ACTORS,
  deriveSystemActorSovereignAccount,
} from './runtime-accounts';
import {
  NATIVE_ASSET,
  type RuntimeAssetKind,
  TYPE_FOREIGN,
  accountIdRecipient,
  isAssetWithId,
} from './runtime-assets';
import { BlockchainSnapshotBuilder } from './snapshot';
import { BlockchainStakingActions } from './staking-actions';
import { BlockchainTransactionSubmitter } from './transactions';

export {
  DEFAULT_DEOS_DAPP_NAME,
  connectDevSigner,
  connectInjectedSigner,
  connectDeosSigner,
  discoverInjectedSignerAccounts,
  hasBuiltInDevSigner,
  injectedSignerAvailability,
  isValidDeosAddress,
  injectedSignerExtensionNames,
  type DeosDevSignerPreset,
  type DeosInjectedSignerAccount,
  type DeosInjectedSignerAvailability,
  type DeosInjectedSignerMatch,
  type DeosSignerMatch,
  DEOS_DEV_SIGNER_PRESETS,
} from '$lib/wallet/signer';

function triggerRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null
    ? Object.fromEntries(
        Object.entries(value).filter(([, property]) => property !== undefined),
      )
    : null;
}

function automationActorPaused(instance: unknown): boolean {
  const actor = triggerRecord(instance);
  const lifecycle = triggerRecord(actor?.lifecycle);
  if (typeof lifecycle?.type === 'string') {
    return lifecycle.type === 'Paused';
  }
  return actor?.is_paused === true;
}

function automationActorRunState(instance: unknown): AutomationRunState {
  const actor = triggerRecord(instance);
  const runState = triggerRecord(actor?.cycle_state);
  return runState?.type === 'Suspended' ? 'suspended' : 'idle';
}

function automationContinuationSnapshot(
  continuation: unknown,
): AutomationContinuationSnapshot | null {
  const value = triggerRecord(continuation);
  if (
    typeof value?.cursor !== 'number' ||
    typeof value.attempt !== 'number' ||
    typeof value.unsuccessful_attempts_at_cursor !== 'number' ||
    typeof value.last_attempt_block !== 'number'
  ) {
    return null;
  }
  return {
    cursor: value.cursor,
    attempt: value.attempt,
    unsuccessfulAttemptsAtCursor: value.unsuccessful_attempts_at_cursor,
    lastAttemptBlock: value.last_attempt_block,
  };
}

function automationQueueTicket(hot: unknown): bigint | null {
  const actor = triggerRecord(hot);
  const ticket = actor?.queue_ticket;
  return typeof ticket === 'bigint' ? ticket : null;
}

function automationFundingAccumulated(
  funding: unknown,
): ReadonlyArray<[string, bigint]> {
  const value = triggerRecord(funding);
  const accumulated = value?.funding_accumulated;
  if (!Array.isArray(accumulated)) {
    return [];
  }
  return accumulated.flatMap((entry) => {
    if (!Array.isArray(entry) || entry.length !== 2) return [];
    const [asset, amount] = entry;
    if (typeof amount !== 'bigint') return [];
    return [[String(asset), amount]];
  });
}

function automationFundingSourcePolicy(funding: unknown): string | null {
  const value = triggerRecord(funding);
  const policy = triggerRecord(value?.funding_source_policy);
  return typeof policy?.type === 'string' ? policy.type : null;
}

function triggerSourceLabel(source: unknown): string {
  const variant = triggerRecord(source);
  switch (variant?.type) {
    case 'Manual':
      return 'Manual';
    case 'OnAddressEvent':
      return 'Address event';
    default:
      return typeof variant?.type === 'string'
        ? variant.type
        : 'Unknown source';
  }
}

function triggerSourcesLabel(value: unknown): string {
  const sources = Array.isArray(value) ? value : [];
  return sources.length === 0
    ? 'No sources'
    : sources.map(triggerSourceLabel).join(' + ');
}

function automationTriggerLabel(trigger?: {
  type: string;
  value?: unknown;
}): string {
  if (!trigger) {
    return 'Unavailable';
  }
  const value = triggerRecord(trigger.value);
  switch (trigger.type) {
    case 'Immediate':
      return `Immediate · ${triggerSourcesLabel(value?.sources)}`;
    case 'Cadenced': {
      const everyBlocks = value?.every_blocks;
      const cadence = typeof everyBlocks === 'number' ? everyBlocks : 'unknown';
      const mode = triggerRecord(value?.mode);
      return mode?.type === 'WhenSignalled'
        ? `Cadenced/${cadence} · ${triggerSourcesLabel(mode.value)}`
        : `Cadenced/${cadence} · Always`;
    }
    default:
      return trigger.type;
  }
}

export class BlockchainAdapter implements Adapter {
  private connection = new BlockchainConnectionSession();
  private context: AdapterRuntimeContext | null = null;
  private onRefresh: (() => void) | null = null;
  private onTransactionProgress:
    | ((progress: TransactionProgress) => void)
    | null = null;
  private readonly snapshotBuilder = new BlockchainSnapshotBuilder(() =>
    this.selectedAddress(),
  );
  private readonly observationReader = new BlockchainObservationReader();

  private async resolvePrimaryForeignAsset(
    snapshot: DeosChainSnapshot,
  ): Promise<RuntimeAssetKind> {
    return await this.snapshotBuilder.resolvePrimaryForeignAsset(snapshot);
  }

  private async xykReserves(
    snapshot: DeosChainSnapshot,
    foreignAsset: RuntimeAssetKind,
  ): Promise<{ native: bigint; foreign: bigint } | null> {
    return await this.snapshotBuilder.xykReserves(snapshot, foreignAsset);
  }

  async getNativeCollatorLpPosition(
    operator: string,
  ): Promise<NativeCollatorLpPositionProjection | null> {
    const accountAddress = this.selectedAddress() || null;
    const normalizedOperator = operator.trim();
    if (!accountAddress || normalizedOperator.length === 0) {
      return null;
    }
    const snapshot = await (await this.ensurePapi()).snapshot();
    const position =
      await snapshot.typedApi.view.Staking.native_collator_lp_position(
        accountAddress,
        normalizedOperator,
        { at: snapshot.at },
      );
    return {
      lpAssetId: position.lp_asset_id ?? null,
      lockedLp: position.locked_lp,
      pendingUnlockLp: position.pending_unlock_lp,
      pendingUnlockBlock: position.pending_unlock_block ?? null,
      conservativeNativeValue: position.conservative_native_value ?? null,
    };
  }

  async getNativeGovernanceCustodyPosition(
    assetId: number,
  ): Promise<NativeGovernanceCustodyPositionProjection | null> {
    const accountAddress = this.selectedAddress() || null;
    if (!accountAddress || !Number.isFinite(assetId)) {
      return null;
    }
    const snapshot = await (await this.ensurePapi()).snapshot();
    const position =
      await snapshot.typedApi.view.Staking.native_governance_custody_position(
        accountAddress,
        assetId,
        { at: snapshot.at },
      );
    return {
      lpAssetId: position.lp_asset_id ?? null,
      governanceLockedLp: position.governance_locked_lp,
      pendingGovernanceLpUnlock: position.pending_governance_lp_unlock,
      pendingGovernanceLpUnlockBlock:
        position.pending_governance_lp_unlock_block ?? null,
      assetId: position.asset_id,
      assetLocked: position.asset_locked,
      pendingAssetUnlock: position.pending_asset_unlock,
      pendingAssetUnlockBlock: position.pending_asset_unlock_block ?? null,
    };
  }

  async getNativeNominationRewardClaimable(
    epoch: number,
  ): Promise<bigint | null> {
    const accountAddress = this.selectedAddress() || null;
    if (!accountAddress || !Number.isInteger(epoch) || epoch < 0) {
      return null;
    }
    const snapshot = await (await this.ensurePapi()).snapshot();
    return (
      (await snapshot.typedApi.view.Staking.native_nomination_reward_claimable(
        epoch,
        accountAddress,
        { at: snapshot.at },
      )) ?? null
    );
  }

  private endpoint(): string {
    if (!this.context) {
      throw new Error('Adapter not initialized');
    }
    return this.context.getEndpoint();
  }

  private selectedAddress(): string {
    return this.context?.getSelectedAddress().trim() ?? '';
  }

  private dappName(): string {
    return this.context?.dappName ?? DEFAULT_DEOS_DAPP_NAME;
  }

  private missingSignerMessage(): string {
    return `No signer is available for ${this.selectedAddress()}. Use an injected wallet account or a built-in Zombienet dev identity.`;
  }

  private async ensurePapi(): Promise<DeosPapiConnection> {
    return await this.connection.ensure(
      this.endpoint(),
      this.onRefresh,
      Boolean(this.context || this.onRefresh || this.onTransactionProgress),
    );
  }

  init(
    _overrides: Partial<SystemConfig>,
    _initialForeign: number,
    context: AdapterRuntimeContext,
    onRefresh?: () => void,
    onTransactionProgress?: (progress: TransactionProgress) => void,
  ): void {
    this.connection.reset();
    this.context = context;
    this.onRefresh = onRefresh || null;
    this.onTransactionProgress = onTransactionProgress || null;
    void this.connection.start(this.endpoint(), this.onRefresh);
  }

  destroy(): void {
    this.context = null;
    this.onRefresh = null;
    this.onTransactionProgress = null;
    this.connection.destroy();
  }

  async getSnapshot(): Promise<SystemSnapshot> {
    try {
      const snapshot = await (await this.ensurePapi()).snapshot();
      return await this.snapshotBuilder.buildSystemSnapshot(snapshot);
    } catch {
      return {
        blockNumber: null,
        supply: 0n,
        priceTmc: 0n,
        priceXyk: null,
        reserveNative: 0n,
        reserveForeign: 0n,
        totalBurned: null,
        supplyLp: 0n,
        hasPool: false,
        hasNativeCurve: false,
        registeredForeignAssetCount: 0,
        minForeignSwapAmount: PRECISION,
        gravityWellRatio: 0,
        buckets: new Map(),
        bufferNative: 0n,
        bufferForeign: 0n,
        nativeAsset: {
          kind: 'Native',
          assetId: null,
          symbol: 'NTVE',
          isCanonical: true,
        },
        foreignAsset: {
          kind: 'Foreign',
          assetId: TYPE_FOREIGN,
          symbol: 'FOREIGN',
          isCanonical: false,
        },
        nativeStaking: {
          isAvailable: false,
          accountAddress: this.selectedAddress() || null,
          exchangeRate: null,
          pool: null,
          accountPosition: null,
        },
      };
    }
  }

  /**
   * ChartWidget now prefers live bounded sampling from the active session.
   * Historical/archive backfill must come from an explicit indexed provider,
   * not opportunistic RPC probing against a standard local node.
   */
  async getHistoricalPricePoints(
    _limit: number,
    _probeAmount: bigint,
  ): Promise<PricePoint[]> {
    return [];
  }

  getConnectionState(): DeosChainConnectionState {
    return (
      this.connection.connectionState() ?? {
        status: 'unconfigured',
        label: 'DEOS blockchain provider',
        endpoint: this.endpoint() || null,
        chainName: null,
        nodeName: null,
        nodeVersion: null,
        genesisHash: null,
        finalizedBlockHash: null,
        finalizedBlockNumber: null,
        message: this.connection.loading
          ? 'Connecting to websocket endpoint'
          : 'PAPI connection not checked yet',
      }
    );
  }

  async getKnownAssetBalances(): Promise<AssetBalanceProjection[]> {
    const address = this.selectedAddress();
    if (!address) {
      return [];
    }
    try {
      const snapshot = await (await this.ensurePapi()).snapshot();
      return await this.snapshotBuilder.knownAssetBalancesAtSnapshot(
        snapshot,
        address,
      );
    } catch {
      return [];
    }
  }

  async getAutomationAuthoringContext(): Promise<AutomationAuthoringContext> {
    return await getDeosActorFinalizedAuthoringContext(await this.ensurePapi());
  }

  async getObservationFeeds() {
    const snapshot = await (await this.ensurePapi()).snapshot();
    return await this.observationReader.feeds(snapshot);
  }

  async getObservationInspection(
    feed: ObservationFeedIdentity,
    maxAgeBlocks: number,
    actorId?: number,
  ) {
    const connection = await this.ensurePapi();
    const snapshot = await connection.snapshot();
    let evidence: ObservationFanoutEvidence;
    try {
      evidence = compareObservationRuntimeEvidence(
        await connection.finalizedRuntimeEvidence(snapshot),
      );
    } catch (error) {
      evidence = {
        status: 'EvidenceMismatch',
        observedIdentity: `Unavailable at ${snapshot.at}`,
        reasons: [
          error instanceof Error
            ? error.message
            : 'Finalized runtime evidence is unavailable',
        ],
      };
    }
    return await this.observationReader.inspection(
      snapshot,
      evidence,
      feed,
      maxAgeBlocks,
      actorId,
    );
  }

  async getAutomationActors(): Promise<AutomationActorSnapshot[]> {
    try {
      const snapshot = await (await this.ensurePapi()).snapshot();
      return await Promise.all(
        KNOWN_SYSTEM_ACTORS.map(async (actor) => {
          const [identity, hot, program, continuation, funding] =
            await Promise.all([
              snapshot.typedApi.query.Actors.ActorIdentities.getValue(
                BigInt(actor.actorId),
                { at: snapshot.at },
              ),
              snapshot.typedApi.query.Actors.ActorHot.getValue(
                BigInt(actor.actorId),
                {
                  at: snapshot.at,
                },
              ),
              snapshot.typedApi.query.Actors.ActorProgram.getValue(
                BigInt(actor.actorId),
                { at: snapshot.at },
              ),
              snapshot.typedApi.query.Actors.ContinuationState.getValue(
                BigInt(actor.actorId),
                { at: snapshot.at },
              ),
              snapshot.typedApi.query.Actors.ActorFunding.getValue(
                BigInt(actor.actorId),
                { at: snapshot.at },
              ),
            ]);
          const eligibility = await readActorEligibility(
            snapshot.typedApi,
            snapshot.at,
            actor.actorId,
          );
          const exists = identity != null && hot != null && program != null;
          const sovereignAccount =
            identity?.sovereign_account ??
            deriveSystemActorSovereignAccount(actor.actorId);
          const account = await snapshot.typedApi.query.System.Account.getValue(
            sovereignAccount,
            { at: snapshot.at },
          );
          return {
            actorId: actor.actorId,
            label: actor.label,
            role: actor.role,
            exists,
            actorClass:
              identity?.actor_class?.type === 'System'
                ? 'System'
                : identity?.actor_class?.type === 'User'
                  ? 'User'
                  : null,
            paused: automationActorPaused(hot),
            runState: automationActorRunState(hot),
            cycleNonce: identity?.cycle_nonce ?? 0n,
            continuation: automationContinuationSnapshot(continuation),
            lastCycleBlock: hot?.last_cycle_block ?? null,
            completionPolicy: program?.completion_policy.type ?? null,
            triggerLabel: automationTriggerLabel(program?.schedule.trigger),
            nativeBalance: account?.data?.free ?? 0n,
            queueTicket: automationQueueTicket(hot),
            fundingAccumulated: automationFundingAccumulated(funding),
            fundingSourcePolicy: automationFundingSourcePolicy(funding),
            eligibility: eligibility.projection,
          } satisfies AutomationActorSnapshot;
        }),
      );
    } catch {
      return [];
    }
  }

  async getUserBalance(): Promise<{ native: bigint; foreign: bigint }> {
    const knownAssets = await this.getKnownAssetBalances();
    return {
      native:
        knownAssets.find((asset) => asset.presentation.kind === 'Native')
          ?.balance ?? 0n,
      foreign:
        knownAssets.find((asset) => asset.isPrimaryRouteAsset)?.balance ?? 0n,
    };
  }

  private selectedQuoteAccountId(): string | null {
    const accountId = this.selectedAddress();
    return accountId.length > 0 ? accountId : null;
  }

  private routeSnapshotPrice(
    snapshot: SystemSnapshot,
    route: 'TMC' | 'XYK',
  ): bigint {
    return route === 'TMC' ? snapshot.priceTmc : (snapshot.priceXyk ?? 0n);
  }

  private accountRecipient(accountId: string) {
    return accountIdRecipient(accountId);
  }

  private readonly transactionSubmitter = new BlockchainTransactionSubmitter(
    () => this.ensurePapi(),
    () => this.selectedAddress(),
    () => this.dappName(),
    (progress) => this.onTransactionProgress?.(progress),
  );

  private async submitSigned(
    ...args: Parameters<BlockchainTransactionSubmitter['submitSigned']>
  ): ReturnType<BlockchainTransactionSubmitter['submitSigned']> {
    return await this.transactionSubmitter.submitSigned(...args);
  }

  private async submitRouterSwap(
    from: RuntimeAssetKind,
    to: RuntimeAssetKind,
    amountIn: bigint,
    minAmountOut: bigint,
  ): Promise<SystemSnapshot> {
    try {
      await this.submitSigned(
        (snapshot, accountId, signer) => {
          return snapshot.typedApi.tx.DeosRouter.swap({
            from,
            to,
            amount_in: amountIn,
            min_amount_out: minAmountOut,
            recipient: accountId,
            deadline: snapshot.finalizedBlockNumber + 50,
          }).signSubmitAndWatch(signer.signer);
        },
        this.missingSignerMessage(),
        `${from.type}->${to.type} swap`,
      );
      return await this.getSnapshot();
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : 'Live swap submission failed',
      );
    }
  }

  async buyNative(
    foreignAmount: bigint,
    slippageBps = 50,
  ): Promise<SwapResult> {
    if (foreignAmount <= 0n) {
      throw new Error('Buy amount must be greater than zero');
    }
    const snapshotBefore = await this.getSnapshot();
    if (foreignAmount < snapshotBefore.minForeignSwapAmount) {
      throw new Error(
        `Buy amount is below runtime minimum ${snapshotBefore.minForeignSwapAmount.toString()}`,
      );
    }
    const quote = await this.getQuoteBuy(foreignAmount);
    if (!quote) {
      throw new Error('No live buy route is available for this size right now');
    }
    const priceBefore = this.routeSnapshotPrice(snapshotBefore, quote.route);
    const minAmountOut =
      quote.out > 0n
        ? (quote.out * BigInt(Math.max(0, 10_000 - slippageBps))) / 10_000n
        : 0n;
    const snapshotAfter = await this.submitRouterSwap(
      await this.resolvePrimaryForeignAsset(
        await (await this.ensurePapi()).snapshot(),
      ),
      NATIVE_ASSET,
      foreignAmount,
      minAmountOut,
    );
    const priceAfter = this.routeSnapshotPrice(snapshotAfter, quote.route);
    return {
      route: quote.route,
      native_out: quote.out,
      foreign_in: foreignAmount,
      foreign_router_fee: quote.fee,
      price_before: priceBefore,
      price_after: priceAfter,
    };
  }

  async sellNative(
    nativeAmount: bigint,
    slippageBps = 50,
  ): Promise<SwapResult> {
    if (nativeAmount <= 0n) {
      throw new Error('Sell amount must be greater than zero');
    }
    const quote = await this.getQuoteSell(nativeAmount);
    if (!quote) {
      throw new Error(
        'No live sell route is available for this size right now',
      );
    }
    const snapshotBefore = await this.getSnapshot();
    const priceBefore = this.routeSnapshotPrice(snapshotBefore, quote.route);
    const minAmountOut =
      quote.out > 0n
        ? (quote.out * BigInt(Math.max(0, 10_000 - slippageBps))) / 10_000n
        : 0n;
    const foreignAsset = await this.resolvePrimaryForeignAsset(
      await (await this.ensurePapi()).snapshot(),
    );
    const snapshotAfter = await this.submitRouterSwap(
      NATIVE_ASSET,
      foreignAsset,
      nativeAmount,
      minAmountOut,
    );
    const priceAfter = this.routeSnapshotPrice(snapshotAfter, quote.route);
    return {
      route: quote.route,
      foreign_out: quote.out,
      native_in: nativeAmount,
      native_router_fee: quote.fee,
      price_before: priceBefore,
      price_after: priceAfter,
    };
  }

  async depositForeign(_amount: bigint): Promise<void> {
    return;
  }

  private readonly stakingActions = new BlockchainStakingActions(
    () => this.ensurePapi(),
    (...args) => this.submitSigned(...args),
    () => this.missingSignerMessage(),
  );

  async claimNominationReward(epoch: number): Promise<void> {
    return await this.stakingActions.claimNominationReward(epoch);
  }

  async claimAndCompoundNominationReward(
    epoch: number,
    operator: string,
  ): Promise<void> {
    return await this.stakingActions.claimAndCompoundNominationReward(
      epoch,
      operator,
    );
  }

  async lockNativeLpForCollator(
    amount: bigint,
    operator: string,
  ): Promise<void> {
    return await this.stakingActions.lockNativeLpForCollator(amount, operator);
  }

  async requestUnlockNativeLp(operator: string, amount: bigint): Promise<void> {
    return await this.stakingActions.requestUnlockNativeLp(operator, amount);
  }

  async withdrawUnlockedNativeLp(operator: string): Promise<void> {
    return await this.stakingActions.withdrawUnlockedNativeLp(operator);
  }

  async redelegateNativeLp(
    fromOperator: string,
    toOperator: string,
    amount: bigint,
  ): Promise<void> {
    return await this.stakingActions.redelegateNativeLp(
      fromOperator,
      toOperator,
      amount,
    );
  }

  async lockNativeLpForGovernance(amount: bigint): Promise<void> {
    return await this.stakingActions.lockNativeLpForGovernance(amount);
  }

  async requestUnlockNativeLpForGovernance(amount: bigint): Promise<void> {
    return await this.stakingActions.requestUnlockNativeLpForGovernance(amount);
  }

  async withdrawUnlockedNativeLpForGovernance(): Promise<void> {
    return await this.stakingActions.withdrawUnlockedNativeLpForGovernance();
  }

  async lockNativeAssetForGovernance(
    assetId: number,
    amount: bigint,
  ): Promise<void> {
    return await this.stakingActions.lockNativeAssetForGovernance(
      assetId,
      amount,
    );
  }

  async requestUnlockNativeAssetForGovernance(
    assetId: number,
    amount: bigint,
  ): Promise<void> {
    return await this.stakingActions.requestUnlockNativeAssetForGovernance(
      assetId,
      amount,
    );
  }

  async withdrawUnlockedNativeAssetForGovernance(
    assetId: number,
  ): Promise<void> {
    return await this.stakingActions.withdrawUnlockedNativeAssetForGovernance(
      assetId,
    );
  }

  transferAssetId(asset: TransferAssetKey): number {
    const match = /^asset:(\d+)$/.exec(asset);
    if (!match) {
      throw new Error('Selected asset is not transferable');
    }
    const assetId = Number(match[1]);
    if (!Number.isSafeInteger(assetId) || assetId < 0 || assetId > 0xffffffff) {
      throw new Error('Selected asset id is outside the local u32 range');
    }
    return assetId;
  }

  async transferAsset(
    asset: TransferAssetKey,
    recipient: string,
    amount: bigint,
  ): Promise<void> {
    if (amount <= 0n) {
      throw new Error('Transfer amount must be greater than zero');
    }
    const normalizedRecipient = recipient.trim();
    if (normalizedRecipient.length === 0) {
      throw new Error('Recipient address is required');
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) => {
          if (asset === 'native') {
            return snapshot.typedApi.tx.Balances.transfer_keep_alive({
              dest: this.accountRecipient(normalizedRecipient),
              value: amount,
            }).signSubmitAndWatch(signer.signer);
          }
          const assetIdPromise =
            asset === 'foreign'
              ? this.resolvePrimaryForeignAsset(snapshot).then(
                  (resolvedForeignAsset) => {
                    if (!isAssetWithId(resolvedForeignAsset)) {
                      throw new Error(
                        'No transferable primary route asset is registered yet',
                      );
                    }
                    return resolvedForeignAsset.value;
                  },
                )
              : Promise.resolve(this.transferAssetId(asset));
          return {
            subscribe: (observer) => {
              let nestedSubscription: { unsubscribe(): void } | null = null;
              let cancelled = false;
              void assetIdPromise
                .then((resolvedAssetId) => {
                  if (!Number.isFinite(resolvedAssetId)) {
                    throw new Error('Selected asset is not transferable');
                  }
                  if (cancelled) {
                    return;
                  }
                  nestedSubscription =
                    snapshot.typedApi.tx.Assets.transfer_keep_alive({
                      id: resolvedAssetId,
                      target: this.accountRecipient(normalizedRecipient),
                      amount,
                    })
                      .signSubmitAndWatch(signer.signer)
                      .subscribe(observer);
                })
                .catch((error) => {
                  observer.error(error);
                });
              return {
                unsubscribe: () => {
                  cancelled = true;
                  nestedSubscription?.unsubscribe();
                },
              };
            },
          };
        },
        this.missingSignerMessage(),
        `${asset} transfer`,
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : 'Live transfer failed',
      );
    }
  }

  async getQuoteBuy(foreignAmount: bigint): Promise<Quote | null> {
    if (foreignAmount <= 0n) {
      return null;
    }
    try {
      const snapshot = await (await this.ensurePapi()).snapshot();
      const foreignAsset = await this.resolvePrimaryForeignAsset(snapshot);
      return await quoteBuyAtSnapshot(
        snapshot,
        this.selectedQuoteAccountId(),
        foreignAsset,
        foreignAmount,
        await this.xykReserves(snapshot, foreignAsset),
      );
    } catch {
      return null;
    }
  }

  async getQuoteSell(nativeAmount: bigint): Promise<Quote | null> {
    if (nativeAmount <= 0n) {
      return null;
    }
    try {
      const snapshot = await (await this.ensurePapi()).snapshot();
      return await quoteSellAtSnapshot(
        snapshot,
        this.selectedQuoteAccountId(),
        await this.resolvePrimaryForeignAsset(snapshot),
        nativeAmount,
      );
    } catch {
      return null;
    }
  }

  async estimateSwapNetworkFee(
    direction: 'buy' | 'sell',
    amountIn: bigint,
    minAmountOut: bigint,
  ): Promise<bigint | null> {
    const accountId = this.selectedQuoteAccountId();
    if (!accountId || amountIn <= 0n || minAmountOut < 0n) {
      return null;
    }
    try {
      const snapshot = await (await this.ensurePapi()).snapshot();
      const foreignAsset = await this.resolvePrimaryForeignAsset(snapshot);
      const from = direction === 'buy' ? foreignAsset : NATIVE_ASSET;
      const to = direction === 'buy' ? NATIVE_ASSET : foreignAsset;
      return await snapshot.typedApi.tx.DeosRouter.swap({
        from,
        to,
        amount_in: amountIn,
        min_amount_out: minAmountOut,
        recipient: accountId,
        deadline: snapshot.finalizedBlockNumber + 50,
      }).getEstimatedFees(accountId);
    } catch {
      return null;
    }
  }

  async getRecentNetworkLog(limit: number): Promise<LogEntry[]> {
    if (limit <= 0) {
      return [];
    }
    const snapshot = await (await this.ensurePapi()).snapshot();
    const records = await snapshot.typedApi.query.System.Events.getValue({
      at: snapshot.at,
    });
    if (!records || records.length === 0) {
      return [];
    }
    const entries: LogEntry[] = [];
    for (
      let index = records.length - 1;
      index >= 0 && entries.length < limit;
      index -= 1
    ) {
      const event = unwrapEventRecord(records[index]);
      if (!event) {
        continue;
      }
      const label = formatChainEventLabel(event);
      entries.push({
        id: `${snapshot.finalizedBlockNumber}-${index}-${label}`,
        step: snapshot.finalizedBlockNumber,
        blockNumber: snapshot.finalizedBlockNumber,
        message: formatChainEventMessage(event),
        type: classifyChainEvent(event),
        label,
        accountId: null,
      });
    }
    return entries;
  }

  async getEffectiveMintPrice(probeAmount: bigint): Promise<number> {
    if (probeAmount <= 0n) {
      return 0;
    }
    try {
      const snapshot = await (await this.ensurePapi()).snapshot();
      const out = await tmcMintQuote(snapshot, probeAmount);
      return out > 0n ? Number(probeAmount) / Number(out) : 0;
    } catch {
      return 0;
    }
  }
}
