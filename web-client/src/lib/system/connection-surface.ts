/*
Domain: System connection surface contract
Owns: Honest presentation state for canonical-chain surfaces across connection and retained-data conditions.
Excludes: Transport lifecycle, widget composition, and domain-specific empty states.
Zone: System public helper; consumed by widgets that require canonical chain data.
*/
export type ChainConnectionView = {
  status: 'connected' | 'unconfigured' | 'error' | 'mock';
  message: string | null;
};

export type ChainSurfaceState = {
  status: 'loading' | 'ready' | 'stale' | 'preview' | 'unconfigured' | 'error';
  title: string;
  detail: string;
};

export function resolveChainSurfaceState(
  connection: ChainConnectionView | null,
  hasData: boolean,
): ChainSurfaceState {
  if (connection === null) {
    return {
      status: 'loading',
      title: 'Preparing chain data',
      detail: 'Waiting for the client to initialize its chain provider.',
    };
  }

  if (connection.status === 'mock') {
    return {
      status: 'preview',
      title: 'Preview provider data',
      detail:
        'Values come from an explicit in-memory preview provider, not canonical chain state.',
    };
  }

  if (connection.status === 'connected') {
    return hasData
      ? {
          status: 'ready',
          title: 'Live chain data',
          detail: 'Values reflect the connected canonical-chain surface.',
        }
      : {
          status: 'loading',
          title: 'Waiting for chain data',
          detail: 'Connected; waiting for the first finalized snapshot.',
        };
  }

  if (hasData) {
    return {
      status: 'stale',
      title: 'Showing retained session data',
      detail:
        'The chain connection is unavailable. Values remain visible for context and may be stale.',
    };
  }

  if (connection.status === 'error') {
    return {
      status: 'error',
      title: 'Chain data unavailable',
      detail:
        connection.message ??
        'The configured provider failed before canonical data became available.',
    };
  }

  return {
    status: 'unconfigured',
    title: 'Connect a DEOS network',
    detail:
      'This surface needs canonical chain data. Configure a PAPI endpoint in Settings.',
  };
}

export function chainSurfaceIsBlocking(state: ChainSurfaceState): boolean {
  return (
    state.status !== 'ready' &&
    state.status !== 'stale' &&
    state.status !== 'preview'
  );
}
