/*
Domain: Execution log contracts
Owns: Log entry, live event, and typed activity-feedback shapes.
Excludes: Event subscription lifecycle, wallet state, adapter transport, and visual log rendering.
Zone: Log public contract; safe for stores, adapters, and widgets to import.
*/
export type LogType = 'info' | 'buy' | 'sell' | 'error';

export type LogEntry = {
  id: number | string;
  step: number;
  blockNumber: number | null;
  message: string;
  type: LogType;
  label?: string;
  accountId?: string | null;
};

export type TransactionProgress =
  | {
      kind: 'idle';
      message: string;
      actionLabel?: undefined;
      highlights?: undefined;
    }
  | {
      kind: 'signed';
      txHash: string;
      message: string;
      actionLabel: string;
      highlights?: undefined;
    }
  | {
      kind: 'broadcasted';
      txHash: string;
      message: string;
      actionLabel: string;
      highlights?: undefined;
    }
  | {
      kind: 'best';
      txHash: string;
      blockNumber: number;
      ok: boolean;
      eventsCount: number;
      message: string;
      actionLabel: string;
      highlights?: string[];
    }
  | {
      kind: 'finalized';
      txHash: string;
      blockNumber: number;
      ok: boolean;
      eventsCount: number;
      dispatchError: string | null;
      message: string;
      actionLabel: string;
      highlights?: string[];
    }
  | {
      kind: 'error';
      txHash: string | null;
      message: string;
      actionLabel?: string;
      highlights?: undefined;
    };
