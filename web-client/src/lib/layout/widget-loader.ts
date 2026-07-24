/*
Domain: Widget loader
Owns: Cached dynamic imports that map all workspace widget ids to components.
Excludes: Widget implementation, layout tree state, and adapter/domain store ownership.
Zone: Layout composition helper; this is the controlled boundary from layout to widgets.
*/
import type { Component } from 'svelte';

import type { WorkspaceWidgetId } from './types';

export type WidgetComponent = Component;

type WidgetModule = {
  default: WidgetComponent;
};

const WIDGET_LOADERS: Record<WorkspaceWidgetId, () => Promise<WidgetModule>> = {
  swap: () => import('$lib/widgets/SwapWidget.svelte'),
  chart: () => import('$lib/widgets/ChartWidget.svelte'),
  statistics: () => import('$lib/widgets/StatisticsWidget.svelte'),
  log: () => import('$lib/widgets/LogWidget.svelte'),
  governance: () => import('$lib/widgets/GovernanceWidget.svelte'),
  wallet: () => import('$lib/widgets/WalletWidget.svelte'),
  automation: () => import('$lib/widgets/AutomationWidget.svelte'),
  wiki: () => import('$lib/widgets/WikiWidget.svelte'),
  'account-menu': () => import('$lib/widgets/AccountWidget.svelte'),
  settings: () => import('$lib/widgets/SettingsWidget.svelte'),
};

const widgetCache = new Map<WorkspaceWidgetId, WidgetComponent>();

export async function loadWidgetComponent(
  panelId: WorkspaceWidgetId,
): Promise<WidgetComponent> {
  const cached = widgetCache.get(panelId);
  if (cached) {
    return cached;
  }
  const module = await WIDGET_LOADERS[panelId]();
  widgetCache.set(panelId, module.default);
  return module.default;
}
