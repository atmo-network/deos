/*
Domain: DEOS blockchain transport
Owns: PAPI client creation, descriptor binding, chain connection state, and low-level runtime helpers.
Excludes: Facade orchestration, domain stores, widget presentation, and governance-specific provider policy.
Zone: Blockchain adapter internals; consumed by focused adapter capability modules only.
*/
import { deos } from '@polkadot-api/descriptors';
import {
  type HexString,
  type PolkadotClient,
  type TypedApi,
} from 'polkadot-api';
import { type StatusChange, WsEvent, createWsClient } from 'polkadot-api/ws';

export const DEFAULT_DEOS_WS_ENDPOINT = 'ws://127.0.0.1:9988';

export type DeosChainConnectionState = {
  status: 'connected' | 'unconfigured' | 'error';
  label: string;
  endpoint: string | null;
  chainName: string | null;
  nodeName: string | null;
  nodeVersion: string | null;
  genesisHash: string | null;
  finalizedBlockHash: string | null;
  finalizedBlockNumber: number | null;
  message: string | null;
};

export type DeosClient = PolkadotClient & {
  switch: (uri?: string) => void;
  getStatus: () => StatusChange;
};
export type DeosTypedApi = TypedApi<typeof deos>;
export type DeosChainSnapshot = {
  at: HexString;
  currentEpoch: number;
  finalizedBlockHash: HexString;
  finalizedBlockNumber: number;
  typedApi: DeosTypedApi;
};
export type DeosFinalizedBlock = {
  hash: HexString;
  number: number;
};

function buildConnectionState(
  status: DeosChainConnectionState['status'],
  message: string,
  endpoint: string | null,
  label = 'DEOS blockchain provider',
): DeosChainConnectionState {
  return {
    status,
    label,
    endpoint,
    chainName: null,
    nodeName: null,
    nodeVersion: null,
    genesisHash: null,
    finalizedBlockHash: null,
    finalizedBlockNumber: null,
    message,
  };
}

export function normalizeBlockchainEndpoint(endpoint: string): string {
  const trimmed = endpoint.trim();
  if (trimmed.length === 0) {
    return '';
  }
  if (trimmed.startsWith('http://')) {
    return `ws://${trimmed.slice('http://'.length)}`;
  }
  if (trimmed.startsWith('https://')) {
    return `wss://${trimmed.slice('https://'.length)}`;
  }
  if (/^[a-z]+:\/\//i.test(trimmed)) {
    return trimmed;
  }
  return `ws://${trimmed}`;
}

function describeWsStatus(status: StatusChange | null): string | null {
  if (status === null) {
    return null;
  }
  switch (status.type) {
    case WsEvent.CONNECTING:
      return `Connecting to ${status.uri}`;
    case WsEvent.CONNECTED:
      return `Connected websocket ${status.uri}`;
    case WsEvent.ERROR:
      return 'WebSocket transport reported an error';
    case WsEvent.CLOSE:
      return 'WebSocket transport closed';
  }
}

export class DeosPapiConnection {
  private client: DeosClient | null = null;
  private currentEndpoint: string | null = null;
  private state: DeosChainConnectionState = buildConnectionState(
    'unconfigured',
    'PAPI connection not checked yet',
    DEFAULT_DEOS_WS_ENDPOINT,
  );
  private typedApi: DeosTypedApi | null = null;
  private wsStatus: StatusChange | null = null;
  private finalizedBlockListeners = new Set<
    (block: DeosFinalizedBlock) => void
  >();
  private finalizedBlockSubscription: { unsubscribe(): void } | null = null;

  constructor(private readonly endpoint: string = DEFAULT_DEOS_WS_ENDPOINT) {
    this.state = buildConnectionState(
      'unconfigured',
      'PAPI connection not checked yet',
      normalizeBlockchainEndpoint(endpoint),
    );
  }

  private resetClient(): void {
    this.finalizedBlockSubscription?.unsubscribe();
    this.finalizedBlockSubscription = null;
    this.client?.destroy();
    this.client = null;
    this.typedApi = null;
    this.wsStatus = null;
    this.currentEndpoint = null;
  }

  destroy(): void {
    this.resetClient();
  }

  private subscribeFinalizedBlockStream(client: DeosClient): void {
    if (
      this.finalizedBlockListeners.size === 0 ||
      this.finalizedBlockSubscription !== null
    ) {
      return;
    }
    this.finalizedBlockSubscription = client.finalizedBlock$.subscribe(
      (block) => {
        this.wsStatus = client.getStatus();
        if (this.state.status === 'connected') {
          this.state = {
            ...this.state,
            finalizedBlockHash: block.hash,
            finalizedBlockNumber: block.number,
          };
        }
        for (const listener of this.finalizedBlockListeners) {
          listener(block);
        }
      },
    );
  }

  private ensureClient(endpoint: string): {
    client: DeosClient;
    typedApi: DeosTypedApi;
  } {
    if (
      this.client !== null &&
      this.typedApi !== null &&
      this.currentEndpoint === endpoint
    ) {
      return {
        client: this.client,
        typedApi: this.typedApi,
      };
    }
    this.resetClient();
    this.client = createWsClient(endpoint, {
      onStatusChanged: (status) => {
        this.wsStatus = status;
      },
    });
    this.wsStatus = this.client.getStatus();
    this.typedApi = this.client.getTypedApi(deos);
    this.currentEndpoint = endpoint;
    this.subscribeFinalizedBlockStream(this.client);
    return {
      client: this.client,
      typedApi: this.typedApi,
    };
  }

  private ensureFinalizedBlockSubscription(endpoint: string): void {
    const { client } = this.ensureClient(endpoint);
    this.subscribeFinalizedBlockStream(client);
  }

  subscribeToFinalizedBlocks(
    onBlock: (block: DeosFinalizedBlock) => void,
  ): () => void {
    this.finalizedBlockListeners.add(onBlock);
    const normalizedEndpoint = normalizeBlockchainEndpoint(this.endpoint);
    if (normalizedEndpoint.length > 0) {
      this.ensureFinalizedBlockSubscription(normalizedEndpoint);
    }
    return () => {
      this.finalizedBlockListeners.delete(onBlock);
      if (this.finalizedBlockListeners.size === 0) {
        this.finalizedBlockSubscription?.unsubscribe();
        this.finalizedBlockSubscription = null;
      }
    };
  }

  connectionState(): DeosChainConnectionState {
    return this.state;
  }

  async ensureConnected(): Promise<{
    client: DeosClient;
    typedApi: DeosTypedApi;
  }> {
    const normalizedEndpoint = normalizeBlockchainEndpoint(this.endpoint);
    if (normalizedEndpoint.length === 0) {
      this.resetClient();
      this.state = buildConnectionState(
        'unconfigured',
        'No PAPI websocket endpoint configured',
        null,
      );
      throw new Error('No PAPI websocket endpoint configured');
    }
    const connection = this.ensureClient(normalizedEndpoint);
    if (this.state.status !== 'connected') {
      await this.syncConnectionState();
    }
    if (this.state.status !== 'connected') {
      throw new Error(this.state.message ?? 'PAPI provider unavailable');
    }
    return connection;
  }

  async syncConnectionState(): Promise<void> {
    const normalizedEndpoint = normalizeBlockchainEndpoint(this.endpoint);
    if (normalizedEndpoint.length === 0) {
      this.resetClient();
      this.state = buildConnectionState(
        'unconfigured',
        'No PAPI websocket endpoint configured',
        null,
      );
      return;
    }
    try {
      const { client } = this.ensureClient(normalizedEndpoint);
      const [chainSpecData, nodeName, nodeVersion, finalizedBlock] =
        await Promise.all([
          client.getChainSpecData(),
          client._request<string>('system_name', []),
          client._request<string>('system_version', []),
          client.getFinalizedBlock(),
        ]);
      this.state = {
        status: 'connected',
        label: `${chainSpecData.name} via PAPI`,
        endpoint: normalizedEndpoint,
        chainName: chainSpecData.name,
        nodeName,
        nodeVersion,
        genesisHash: chainSpecData.genesisHash,
        finalizedBlockHash: finalizedBlock.hash,
        finalizedBlockNumber: finalizedBlock.number,
        message: 'PAPI connected',
      };
    } catch (error) {
      this.state = {
        ...buildConnectionState(
          'error',
          error instanceof Error
            ? error.message
            : 'Unknown PAPI connection error',
          normalizedEndpoint,
        ),
        message: [
          error instanceof Error
            ? error.message
            : 'Unknown PAPI connection error',
          describeWsStatus(this.wsStatus),
        ]
          .filter(Boolean)
          .join(' · '),
      };
    }
  }

  async snapshot(): Promise<DeosChainSnapshot> {
    const { client, typedApi } = await this.ensureConnected();
    const finalizedBlock = await client.getFinalizedBlock();
    return {
      at: finalizedBlock.hash,
      currentEpoch: finalizedBlock.number,
      finalizedBlockHash: finalizedBlock.hash,
      finalizedBlockNumber: finalizedBlock.number,
      typedApi,
    };
  }
}
