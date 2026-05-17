/*
Domain: Blockchain transaction submission
Owns: Signer acquisition, sign-submit-watch lifecycle, transaction progress events, and finalized result mapping.
Excludes: Concrete pallet action selection, widget state, and chain read-model assembly.
Zone: Transport write adapter internals; depends on signer and log contracts without importing stores/widgets.
*/
import type { TransactionProgress } from '$lib/log/types';
import { connectDeosSigner } from '$lib/wallet/signer';

import type { DeosChainSnapshot, DeosPapiConnection } from './deos';
import { buildTransactionHighlights, describeDispatchError } from './events';

type DeosSigner = NonNullable<Awaited<ReturnType<typeof connectDeosSigner>>>;

type TransactionWatcherEvent =
  | { type: 'signed'; txHash: string }
  | { type: 'broadcasted'; txHash: string }
  | {
      type: 'txBestBlocksState';
      txHash: string;
      found: false;
      isValid: boolean;
    }
  | {
      type: 'txBestBlocksState';
      txHash: string;
      found: true;
      isValid?: boolean;
      block: { number: number };
      ok: boolean;
      events: unknown[];
    }
  | {
      type: 'finalized';
      txHash: string;
      block: { number: number };
      ok: boolean;
      events: unknown[];
      dispatchError?: { type?: string; value?: unknown } | null;
    };

type TransactionWatcher = {
  subscribe: (observer: {
    next: (event: TransactionWatcherEvent) => void;
    error: (error: unknown) => void;
  }) => { unsubscribe(): void };
};

export type SignedTransactionSubmitter = (
  snapshot: DeosChainSnapshot,
  accountId: string,
  signer: DeosSigner,
) => TransactionWatcher;

export type SubmittedTransactionResult = {
  txHash: string;
  blockNumber: number;
  ok: boolean;
  dispatchError: string | null;
};

export class BlockchainTransactionSubmitter {
  constructor(
    private readonly ensurePapi: () => Promise<DeosPapiConnection>,
    private readonly selectedAddress: () => string | null,
    private readonly dappName: () => string,
    private readonly emitTransactionProgress: (
      progress: TransactionProgress,
    ) => void,
  ) {}

  async submitSigned(
    submitter: SignedTransactionSubmitter,
    missingSignerMessage: string,
    actionLabel: string,
  ): Promise<SubmittedTransactionResult> {
    const papi = await this.ensurePapi();
    const accountId = this.selectedAddress();
    if (!accountId) {
      throw new Error('Select an account before submitting a live transaction');
    }
    const signer = await connectDeosSigner(accountId, this.dappName());
    if (!signer) {
      throw new Error(missingSignerMessage);
    }
    try {
      const snapshot = await papi.snapshot();
      const watcher = submitter(snapshot, accountId, signer);
      const result = await this.watchSubmittedTransaction(watcher, actionLabel);
      await papi.syncConnectionState();
      return result;
    } finally {
      signer.disconnect();
    }
  }

  private async watchSubmittedTransaction(
    watcher: TransactionWatcher,
    actionLabel: string,
  ): Promise<SubmittedTransactionResult> {
    return await new Promise((resolve, reject) => {
      const subscription = watcher.subscribe({
        next: (event) => {
          switch (event.type) {
            case 'signed':
              this.emitTransactionProgress({
                kind: 'signed',
                txHash: event.txHash,
                message: this.formatActionLabel(
                  actionLabel,
                  `Signed ${event.txHash}`,
                ),
                actionLabel,
              });
              return;
            case 'broadcasted':
              this.emitTransactionProgress({
                kind: 'broadcasted',
                txHash: event.txHash,
                message: this.formatActionLabel(
                  actionLabel,
                  `Broadcasted ${event.txHash}`,
                ),
                actionLabel,
              });
              return;
            case 'txBestBlocksState':
              if (!event.found) {
                this.emitTransactionProgress({
                  kind: 'broadcasted',
                  txHash: event.txHash,
                  message: event.isValid
                    ? this.formatActionLabel(
                        actionLabel,
                        `Broadcasted ${event.txHash}; waiting for inclusion`,
                      )
                    : this.formatActionLabel(
                        actionLabel,
                        `Transaction ${event.txHash} became invalid before inclusion`,
                      ),
                  actionLabel,
                });
                if (!event.isValid) {
                  subscription.unsubscribe();
                  reject(
                    new Error(
                      `Transaction ${event.txHash} became invalid before inclusion`,
                    ),
                  );
                }
                return;
              }
              this.emitTransactionProgress({
                kind: 'best',
                txHash: event.txHash,
                blockNumber: event.block.number,
                ok: event.ok,
                eventsCount: event.events.length,
                message: event.ok
                  ? this.formatActionLabel(
                      actionLabel,
                      `Included in best block #${event.block.number}`,
                    )
                  : this.formatActionLabel(
                      actionLabel,
                      `Included in best block #${event.block.number} with dispatch error`,
                    ),
                actionLabel,
                highlights: buildTransactionHighlights(event.events),
              });
              return;
            case 'finalized':
              this.emitTransactionProgress({
                kind: 'finalized',
                txHash: event.txHash,
                blockNumber: event.block.number,
                ok: event.ok,
                eventsCount: event.events.length,
                dispatchError: describeDispatchError(event.dispatchError),
                message: event.ok
                  ? this.formatActionLabel(
                      actionLabel,
                      `Finalized in block #${event.block.number}`,
                    )
                  : this.formatActionLabel(
                      actionLabel,
                      `Finalized with dispatch error in block #${event.block.number}`,
                    ),
                actionLabel,
                highlights: buildTransactionHighlights(event.events),
              });
              subscription.unsubscribe();
              resolve({
                txHash: event.txHash,
                blockNumber: event.block.number,
                ok: event.ok,
                dispatchError: describeDispatchError(event.dispatchError),
              });
              return;
          }
        },
        error: (error) => {
          subscription.unsubscribe();
          const message =
            error instanceof Error ? error.message : 'Live transaction failed';
          this.emitTransactionProgress({
            kind: 'error',
            txHash: null,
            message: this.formatActionLabel(actionLabel, message),
            actionLabel,
          });
          reject(error);
        },
      });
    });
  }

  private formatActionLabel(actionLabel: string, message: string): string {
    return `${actionLabel} · ${message}`;
  }
}
