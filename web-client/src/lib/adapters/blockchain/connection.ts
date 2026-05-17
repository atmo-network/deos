/*
Domain: Blockchain connection lifecycle
Owns: PAPI connection reuse, endpoint switching, finalized-block refresh subscription, and teardown.
Excludes: Adapter business methods, transaction submission, signer selection, and UI state.
Zone: Transport adapter internals; depends only on the DEOS PAPI connection wrapper.
*/
import type { DeosPapiConnection } from './deos';

export class BlockchainConnectionSession {
  private papi: DeosPapiConnection | null = null;
  private papiLoading: Promise<DeosPapiConnection> | null = null;
  private currentEndpoint: string | null = null;
  private loadingEndpoint: string | null = null;
  private connectionGeneration = 0;
  private cancelFinalizedBlockSub: (() => void) | null = null;

  reset(): void {
    if (this.cancelFinalizedBlockSub) {
      this.cancelFinalizedBlockSub();
      this.cancelFinalizedBlockSub = null;
    }
    if (this.papi) {
      this.papi.destroy();
      this.papi = null;
    }
    this.papiLoading = null;
    this.currentEndpoint = null;
    this.loadingEndpoint = null;
  }

  destroy(): void {
    this.connectionGeneration += 1;
    this.reset();
  }

  get loading(): boolean {
    return this.papiLoading !== null;
  }

  connectionState() {
    return this.papi?.connectionState() ?? null;
  }

  ensure(
    endpoint: string,
    onRefresh: (() => void) | null,
    initialized: boolean,
  ): Promise<DeosPapiConnection> {
    if (this.papi && this.currentEndpoint === endpoint) {
      return Promise.resolve(this.papi);
    }
    if (this.papiLoading && this.loadingEndpoint === endpoint) {
      return this.papiLoading;
    }
    if (!initialized && !this.papi && !this.papiLoading) {
      throw new Error('Adapter not initialized');
    }
    this.reset();
    return this.start(endpoint, onRefresh);
  }

  start(
    endpoint: string,
    onRefresh: (() => void) | null,
  ): Promise<DeosPapiConnection> {
    const generation = ++this.connectionGeneration;
    this.loadingEndpoint = endpoint;
    this.papiLoading = import('./deos')
      .then(({ DeosPapiConnection }) => {
        if (generation !== this.connectionGeneration) {
          throw new Error('Adapter initialization superseded');
        }
        const papi = new DeosPapiConnection(endpoint);
        this.cancelFinalizedBlockSub = papi.subscribeToFinalizedBlocks(() => {
          onRefresh?.();
        });
        this.papi = papi;
        this.currentEndpoint = endpoint;
        this.loadingEndpoint = null;
        return papi;
      })
      .catch((error) => {
        if (generation === this.connectionGeneration) {
          this.papiLoading = null;
          this.loadingEndpoint = null;
        }
        throw error;
      });
    void this.papiLoading
      .then(() => {
        if (generation === this.connectionGeneration) {
          onRefresh?.();
        }
      })
      .catch(() => {});
    return this.papiLoading;
  }
}
