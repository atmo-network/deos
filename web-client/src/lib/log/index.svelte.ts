import type { Adapter } from "$lib/adapters/types";
import {
  fromSessionDerivedChain,
  type ReadModelValue,
} from "$lib/shared/read-model";
import { walletStore } from "$lib/wallet/index.svelte";
import type {
  LogEntry,
  LogType,
  TransactionProgress,
} from "$lib/shared/types";

const IDLE_PROGRESS: TransactionProgress = {
  kind: "idle",
  message: "No transaction submitted yet",
};

class LogStore {
  log: LogEntry[] = $state([]);
  networkLog: LogEntry[] = $state([]);
  networkLogView: ReadModelValue<LogEntry[]> | null = $state(null);
  txProgress: TransactionProgress = $state(IDLE_PROGRESS);
  private logCounter = 0;

  reset() {
    this.log = [];
    this.networkLog = [];
    this.networkLogView = null;
    this.txProgress = IDLE_PROGRESS;
  }

  setTransactionProgress(progress: TransactionProgress) {
    this.txProgress = progress;
  }

  async refreshNetworkLog(adapter: Adapter) {
    if (!adapter.getRecentNetworkLog) {
      return;
    }
    try {
      const recentEntries = await adapter.getRecentNetworkLog(24);
      if (recentEntries.length === 0) {
        return;
      }
      const seen = new Set<string>();
      const merged = [...recentEntries, ...this.networkLog].filter((entry) => {
        const key = String(entry.id);
        if (seen.has(key)) {
          return false;
        }
        seen.add(key);
        return true;
      });
      const nextNetworkLog = merged
        .sort((left, right) => {
          const leftBlock = left.blockNumber ?? left.step;
          const rightBlock = right.blockNumber ?? right.step;
          if (leftBlock !== rightBlock) {
            return rightBlock - leftBlock;
          }
          return String(right.id).localeCompare(String(left.id));
        })
        .slice(0, 80);
      this.networkLog = nextNetworkLog;
      this.networkLogView = fromSessionDerivedChain(
        nextNetworkLog,
        "finalized-events",
        "System.Events",
        "session",
        {
          asOfBlock: nextNetworkLog[0]?.blockNumber ?? undefined,
        },
      );
    } catch {
      // Keep the last successful live network feed instead of blanking the panel
    }
  }

  add(
    message: string,
    type: LogType = "info",
    context?: {
      blockNumber?: number | null;
      step?: number;
      accountId?: string | null;
    },
  ) {
    const entry: LogEntry = {
      id: this.logCounter++,
      step: context?.step ?? this.logCounter,
      blockNumber: context?.blockNumber ?? null,
      message,
      type,
      accountId: context?.accountId ?? (walletStore.state.selectedAddress || null),
    };
    this.log = [entry, ...this.log.slice(0, 199)];
  }

  clear() {
    this.log = [];
  }
}

export const logStore = new LogStore();
