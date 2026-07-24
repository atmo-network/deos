/*
Domain: Workspace widget presentation metadata
Owns: Semantic Lucide icon selection for center-pane and sidebar widget identities.
Excludes: Widget labels, layout ordering, navigation, widget implementation, and generated Wiki page identity.
Zone: Layout presentation helper consumed by tab and accordion chrome.
*/
import {
  ArrowLeftRight,
  BookOpen,
  Bot,
  ChartCandlestick,
  ChartNoAxesColumnIncreasing,
  Landmark,
  ScrollText,
  Settings,
  UserRound,
  WalletCards,
} from '@lucide/svelte';
import type { Component } from 'svelte';

import type { PanelId, SidebarWidgetId, WorkspaceWidgetId } from './types';

export const WIDGET_ICONS: Record<WorkspaceWidgetId, Component<any>> = {
  swap: ArrowLeftRight,
  wallet: WalletCards,
  log: ScrollText,
  statistics: ChartNoAxesColumnIncreasing,
  chart: ChartCandlestick,
  automation: Bot,
  governance: Landmark,
  wiki: BookOpen,
  'account-menu': UserRound,
  settings: Settings,
};

export const PANEL_ICONS: Record<PanelId, Component<any>> = WIDGET_ICONS;
export const SIDEBAR_WIDGET_ICONS: Record<
  SidebarWidgetId,
  Component<any>
> = WIDGET_ICONS;
