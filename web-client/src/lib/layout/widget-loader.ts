import type { PanelId } from "./types";

type WidgetComponent = any;

type WidgetModule = {
  default: WidgetComponent;
};

const WIDGET_LOADERS: Record<PanelId, () => Promise<WidgetModule>> = {
  swap: () => import("$lib/widgets/SwapWidget.svelte"),
  chart: () => import("$lib/widgets/ChartWidget.svelte"),
  statistics: () => import("$lib/widgets/StatisticsWidget.svelte"),
  log: () => import("$lib/widgets/LogWidget.svelte"),
  governance: () => import("$lib/widgets/GovernanceWidget.svelte"),
  wallet: () => import("$lib/widgets/WalletWidget.svelte"),
  automation: () => import("$lib/widgets/AutomationWidget.svelte"),
  wiki: () => import("$lib/widgets/WikiWidget.svelte"),
};

const widgetCache = new Map<PanelId, WidgetComponent>();

export async function loadWidgetComponent(
  panelId: PanelId,
): Promise<WidgetComponent> {
  const cached = widgetCache.get(panelId);
  if (cached) {
    return cached;
  }
  const module = await WIDGET_LOADERS[panelId]();
  widgetCache.set(panelId, module.default);
  return module.default;
}
