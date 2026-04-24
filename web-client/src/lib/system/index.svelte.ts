import type { Adapter } from "$lib/adapters/types";
import { BlockchainAdapter } from "$lib/adapters/blockchain";
import { logStore } from "$lib/log/index.svelte";
import { marketStore } from "$lib/market/index.svelte";
import { portfolioStore } from "$lib/portfolio/index.svelte";
import type {
  SystemConfig,
  SystemSnapshot,
  DeosChainConnectionState,
} from "./types";

const DEFAULT_INITIAL_FOREIGN_BALANCE = 100000;

class SystemStore {
  adapter: Adapter = $state(new BlockchainAdapter());
  snapshot: SystemSnapshot | null = $state(null);
  connectionState: DeosChainConnectionState | null = $state(null);

  async init(overrides: Partial<SystemConfig> = {}, initialForeign?: number): Promise<void> {
    const foreign = initialForeign ?? DEFAULT_INITIAL_FOREIGN_BALANCE;
    if (this.adapter.destroy) {
      this.adapter.destroy();
    }
    portfolioStore.bind({
      getAdapter: () => this.adapter,
      getSnapshot: () => this.snapshot,
      refreshSystem: () => this.refresh(),
    });
    marketStore.bind({
      getAdapter: () => this.adapter,
      getSnapshot: () => this.snapshot,
      refreshSystem: () => this.refresh(),
    });
    portfolioStore.reset();
    marketStore.reset();
    logStore.reset();
    this.adapter.init(
      overrides,
      foreign,
      () => this.refresh(),
      (progress) => {
        logStore.setTransactionProgress(progress);
      },
    );
    await this.refresh();
  }

  async refresh() {
    try {
      this.snapshot = await this.adapter.getSnapshot();
      if (this.adapter.getConnectionState) {
        this.connectionState = await this.adapter.getConnectionState();
      }
      if (this.adapter.getKnownAssetBalances) {
        portfolioStore.setKnownAssetBalances(
          await this.adapter.getKnownAssetBalances(),
        );
      } else {
        portfolioStore.setKnownAssetBalances([]);
        portfolioStore.setUserBalance(await this.adapter.getUserBalance());
      }
      await logStore.refreshNetworkLog(this.adapter);
      await marketStore.syncHistory();
    } catch (error) {
      if (this.adapter.getConnectionState) {
        this.connectionState = await this.adapter.getConnectionState();
      }
      console.warn("Blockchain adapter refresh failed", error);
    }
  }

  async claimNominationReward(epoch: number): Promise<void> {
    if (!this.adapter.claimNominationReward) {
      throw new Error("Native nomination reward claim is unavailable for this adapter");
    }
    await this.adapter.claimNominationReward(epoch);
    await this.refresh();
  }

  async claimAndCompoundNominationReward(epoch: number, operator: string): Promise<void> {
    if (!this.adapter.claimAndCompoundNominationReward) {
      throw new Error("Native nomination reward compound is unavailable for this adapter");
    }
    await this.adapter.claimAndCompoundNominationReward(epoch, operator);
    await this.refresh();
  }

  async lockNativeLpForCollator(amount: bigint, operator: string): Promise<void> {
    if (!this.adapter.lockNativeLpForCollator) {
      throw new Error("Native LP collator lock is unavailable for this adapter");
    }
    await this.adapter.lockNativeLpForCollator(amount, operator);
    await this.refresh();
  }

  async requestUnlockNativeLp(operator: string, amount: bigint): Promise<void> {
    if (!this.adapter.requestUnlockNativeLp) {
      throw new Error("Native LP unlock request is unavailable for this adapter");
    }
    await this.adapter.requestUnlockNativeLp(operator, amount);
    await this.refresh();
  }

  async withdrawUnlockedNativeLp(operator: string): Promise<void> {
    if (!this.adapter.withdrawUnlockedNativeLp) {
      throw new Error("Native LP withdrawal is unavailable for this adapter");
    }
    await this.adapter.withdrawUnlockedNativeLp(operator);
    await this.refresh();
  }

  async redelegateNativeLp(fromOperator: string, toOperator: string, amount: bigint): Promise<void> {
    if (!this.adapter.redelegateNativeLp) {
      throw new Error("Native LP redelegation is unavailable for this adapter");
    }
    await this.adapter.redelegateNativeLp(fromOperator, toOperator, amount);
    await this.refresh();
  }

  async lockNativeLpForGovernance(amount: bigint): Promise<void> {
    if (!this.adapter.lockNativeLpForGovernance) {
      throw new Error("Native governance LP lock is unavailable for this adapter");
    }
    await this.adapter.lockNativeLpForGovernance(amount);
    await this.refresh();
  }

  async requestUnlockNativeLpForGovernance(amount: bigint): Promise<void> {
    if (!this.adapter.requestUnlockNativeLpForGovernance) {
      throw new Error("Native governance LP unlock request is unavailable for this adapter");
    }
    await this.adapter.requestUnlockNativeLpForGovernance(amount);
    await this.refresh();
  }

  async withdrawUnlockedNativeLpForGovernance(): Promise<void> {
    if (!this.adapter.withdrawUnlockedNativeLpForGovernance) {
      throw new Error("Native governance LP withdrawal is unavailable for this adapter");
    }
    await this.adapter.withdrawUnlockedNativeLpForGovernance();
    await this.refresh();
  }

  async lockNativeAssetForGovernance(assetId: number, amount: bigint): Promise<void> {
    if (!this.adapter.lockNativeAssetForGovernance) {
      throw new Error("Native governance asset lock is unavailable for this adapter");
    }
    await this.adapter.lockNativeAssetForGovernance(assetId, amount);
    await this.refresh();
  }

  async requestUnlockNativeAssetForGovernance(assetId: number, amount: bigint): Promise<void> {
    if (!this.adapter.requestUnlockNativeAssetForGovernance) {
      throw new Error("Native governance asset unlock request is unavailable for this adapter");
    }
    await this.adapter.requestUnlockNativeAssetForGovernance(assetId, amount);
    await this.refresh();
  }

  async withdrawUnlockedNativeAssetForGovernance(assetId: number): Promise<void> {
    if (!this.adapter.withdrawUnlockedNativeAssetForGovernance) {
      throw new Error("Native governance asset withdrawal is unavailable for this adapter");
    }
    await this.adapter.withdrawUnlockedNativeAssetForGovernance(assetId);
    await this.refresh();
  }

  async getNativeCollatorLpPosition(operator: string) {
    if (!this.adapter.getNativeCollatorLpPosition) {
      throw new Error("Native collator LP position detail is unavailable for this adapter");
    }
    return await this.adapter.getNativeCollatorLpPosition(operator);
  }

  async getNativeGovernanceCustodyPosition(assetId: number) {
    if (!this.adapter.getNativeGovernanceCustodyPosition) {
      throw new Error("Native governance custody detail is unavailable for this adapter");
    }
    return await this.adapter.getNativeGovernanceCustodyPosition(assetId);
  }

  async getNativeNominationRewardClaimable(epoch: number) {
    if (!this.adapter.getNativeNominationRewardClaimable) {
      throw new Error("Native nomination reward claimability is unavailable for this adapter");
    }
    return await this.adapter.getNativeNominationRewardClaimable(epoch);
  }

}

export const systemStore = new SystemStore();
