/*
Domain: Market store
Owns: Swap quote/session state, market projections, runtime market refresh, and derived price/TOL views.
Excludes: Adapter transport implementation, wallet custody, layout state, and widget presentation.
Zone: Market state slice; may depend on adapter contract, read-model provenance, and UI formatting helpers.
*/
import type { Adapter } from '$lib/adapters/contract';
import { PRECISION } from '$lib/economics';
import type { PricePoint, Quote, SwapResult } from '$lib/market/types';
import {
  type ReadModelValue,
  fromRuntimeView,
  fromSessionDerivedChain,
} from '$lib/read-model';
import type { SystemSnapshot } from '$lib/system/types';
import { toFloat } from '$lib/ui/format';

const PROBE_AMOUNT = 100n * PRECISION;
const NETWORK_FEE_ESTIMATE_TIMEOUT_MS = 2_000;

type MarketBridge = {
  getAdapter: () => Adapter;
  getSnapshot: () => SystemSnapshot | null;
  refreshSystem: () => Promise<void>;
};

class MarketStore {
  direction: 'buy' | 'sell' = $state('buy');
  history: PricePoint[] = $state([]);
  historyView: ReadModelValue<PricePoint[]> | null = $state(null);
  quoteView: ReadModelValue<Quote> | null = $state(null);
  private bridge: MarketBridge | null = null;
  private quoteRequestId = 0;

  bind(bridge: MarketBridge) {
    this.bridge = bridge;
  }

  reset() {
    this.direction = 'buy';
    this.history = [];
    this.historyView = null;
    this.quoteView = null;
    this.quoteRequestId = 0;
  }

  private adapter(): Adapter | null {
    return this.bridge?.getAdapter() ?? null;
  }

  private snapshot(): SystemSnapshot | null {
    return this.bridge?.getSnapshot() ?? null;
  }

  async syncHistory() {
    const snapshot = this.snapshot();
    const adapter = this.adapter();
    if (!snapshot || !adapter) {
      return;
    }
    const lastPoint = this.history.at(-1) ?? null;
    const blockNumber = snapshot.blockNumber;
    const step =
      blockNumber ?? (lastPoint ? lastPoint.step + 1 : this.history.length);

    let pEffTMC = 0;
    try {
      pEffTMC = await adapter.getEffectiveMintPrice(PROBE_AMOUNT);
    } catch {
      // expected if not fully implemented
    }
    const pXYK = snapshot.priceXyk ? toFloat(snapshot.priceXyk) : 0;

    let priceRouter: number | null = null;
    let routeRouter: 'TMC' | 'XYK' | null = null;
    try {
      const probeRouter = PROBE_AMOUNT;
      const quoteBuy = await adapter.getQuoteBuy(probeRouter);
      if (quoteBuy && quoteBuy.out > 0n) {
        priceRouter = toFloat(probeRouter) / toFloat(quoteBuy.out);
        routeRouter = quoteBuy.route;
      }
    } catch {
      // expected if not fully implemented
    }

    const supply = toFloat(snapshot.supply);
    const nextPoint = {
      step,
      blockNumber,
      priceEffTMC: pEffTMC,
      priceXYK: pXYK,
      priceRouter,
      routeRouter,
      supply,
    };
    const nextHistory =
      lastPoint && blockNumber !== null && lastPoint.blockNumber === blockNumber
        ? [...this.history.slice(0, -1), nextPoint]
        : [...this.history, nextPoint];
    this.history = nextHistory;
    this.historyView = fromSessionDerivedChain(
      nextHistory,
      'bounded-block-sampler',
      'marketStore.syncHistory',
      'bounded-recent',
      {
        asOfBlock: blockNumber ?? undefined,
      },
    );
  }

  async buyNative(
    foreignAmount: bigint,
    slippageBps?: number,
  ): Promise<SwapResult> {
    const adapter = this.adapter();
    if (!adapter || !this.bridge) {
      throw new Error('Market bridge not initialized');
    }
    const result = await adapter.buyNative(foreignAmount, slippageBps);
    await this.bridge.refreshSystem();
    return result;
  }

  async sellNative(
    nativeAmount: bigint,
    slippageBps?: number,
  ): Promise<SwapResult> {
    const adapter = this.adapter();
    if (!adapter || !this.bridge) {
      throw new Error('Market bridge not initialized');
    }
    const result = await adapter.sellNative(nativeAmount, slippageBps);
    await this.bridge.refreshSystem();
    return result;
  }

  async getQuoteBuy(foreignAmount: bigint): Promise<Quote | null> {
    const adapter = this.adapter();
    if (!adapter) {
      this.quoteView = null;
      return null;
    }
    const requestId = ++this.quoteRequestId;
    try {
      const quote = await adapter.getQuoteBuy(foreignAmount);
      if (requestId === this.quoteRequestId) {
        this.quoteView = quote
          ? fromRuntimeView(quote, 'AxialRouter.quote_exact_input', {
              asOfBlock: this.snapshot()?.blockNumber ?? undefined,
            })
          : null;
      }
      return quote;
    } catch (error) {
      if (requestId === this.quoteRequestId) {
        this.quoteView = null;
      }
      throw error;
    }
  }

  async estimateSwapNetworkFee(
    direction: 'buy' | 'sell',
    amountIn: bigint,
    minAmountOut: bigint,
  ): Promise<bigint | null> {
    const adapter = this.adapter();
    if (!adapter?.estimateSwapNetworkFee) {
      return null;
    }
    let timeout: ReturnType<typeof setTimeout> | null = null;
    try {
      return await Promise.race([
        Promise.resolve(
          adapter.estimateSwapNetworkFee(direction, amountIn, minAmountOut),
        ),
        new Promise<null>((resolve) => {
          timeout = setTimeout(
            () => resolve(null),
            NETWORK_FEE_ESTIMATE_TIMEOUT_MS,
          );
        }),
      ]);
    } catch {
      return null;
    } finally {
      if (timeout !== null) {
        clearTimeout(timeout);
      }
    }
  }

  async getQuoteSell(nativeAmount: bigint): Promise<Quote | null> {
    const adapter = this.adapter();
    if (!adapter) {
      this.quoteView = null;
      return null;
    }
    const requestId = ++this.quoteRequestId;
    try {
      const quote = await adapter.getQuoteSell(nativeAmount);
      if (requestId === this.quoteRequestId) {
        this.quoteView = quote
          ? fromRuntimeView(quote, 'AxialRouter.quote_exact_input', {
              asOfBlock: this.snapshot()?.blockNumber ?? undefined,
            })
          : null;
      }
      return quote;
    } catch (error) {
      if (requestId === this.quoteRequestId) {
        this.quoteView = null;
      }
      throw error;
    }
  }

  flipDirection() {
    this.direction = this.direction === 'buy' ? 'sell' : 'buy';
  }
}

export const marketStore = new MarketStore();
