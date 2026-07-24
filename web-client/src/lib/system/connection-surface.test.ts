import assert from 'node:assert/strict';
import test from 'node:test';

import type { DeosChainConnectionState } from '$lib/adapters/blockchain/deos';

import {
  chainSurfaceIsBlocking,
  resolveChainSurfaceState,
} from './connection-surface.ts';

function connection(
  status: DeosChainConnectionState['status'],
  message: string | null = null,
): DeosChainConnectionState {
  return {
    status,
    label: 'DEOS blockchain provider',
    endpoint: null,
    chainName: null,
    nodeName: null,
    nodeVersion: null,
    genesisHash: null,
    finalizedBlockHash: null,
    finalizedBlockNumber: null,
    message,
  };
}

test('blocks factual chain surfaces until a connected snapshot exists', () => {
  assert.equal(resolveChainSurfaceState(null, false).status, 'loading');
  assert.equal(
    resolveChainSurfaceState(connection('connected'), false).status,
    'loading',
  );
  assert.equal(
    resolveChainSurfaceState(connection('unconfigured'), false).status,
    'unconfigured',
  );
  assert.equal(
    resolveChainSurfaceState(connection('error'), false).status,
    'error',
  );
});

test('distinguishes live data from retained session data', () => {
  const live = resolveChainSurfaceState(connection('connected'), true);
  const stale = resolveChainSurfaceState(
    connection('error', 'Provider disconnected'),
    true,
  );

  assert.equal(live.status, 'ready');
  assert.equal(chainSurfaceIsBlocking(live), false);
  assert.equal(stale.status, 'stale');
  assert.equal(chainSurfaceIsBlocking(stale), false);
  assert.match(stale.detail, /may be stale/);
});

test('labels explicit preview providers without presenting them as chain truth', () => {
  const state = resolveChainSurfaceState(
    { status: 'mock', message: 'In-memory fixtures' },
    true,
  );

  assert.equal(state.status, 'preview');
  assert.equal(chainSurfaceIsBlocking(state), false);
  assert.match(state.detail, /not canonical chain state/);
});

test('preserves the provider error when no data can be shown', () => {
  const state = resolveChainSurfaceState(
    connection('error', 'Endpoint refused the connection'),
    false,
  );

  assert.equal(state.title, 'Chain data unavailable');
  assert.equal(state.detail, 'Endpoint refused the connection');
  assert.equal(chainSurfaceIsBlocking(state), true);
});
