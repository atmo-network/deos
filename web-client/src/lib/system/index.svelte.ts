import type { Adapter } from "$lib/adapters/types";
import { BlockchainAdapter } from "$lib/adapters/blockchain";
import { logStore } from "$lib/log/index.svelte";
import { marketStore } from "$lib/market/index.svelte";
import { portfolioStore } from "$lib/portfolio/index.svelte";
import type {
  SystemConfig,
  SystemSnapshot,
  TmctolChainConnectionState,
} from "./types";

const DEFAULT_INITIAL_FOREIGN_BALANCE = 100000;

class SystemStore {
  adapter: Adapter = $state(new BlockchainAdapter());
  snapshot: SystemSnapshot | null = $state(null);
  connectionState: TmctolChainConnectionState | null = $state(null);

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

}

export const systemStore = new SystemStore();
