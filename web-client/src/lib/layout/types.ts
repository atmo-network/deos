/*
Domain: Layout contracts
Owns: Workspace widget identities, placement subsets, tile tree types, lane topology types, and persisted frame state shapes.
Excludes: Store mutation, DOM rendering, widget implementations, and adapter/domain state.
Zone: Layout public contract; safe for layout, system, and widget composition to import.
*/
export type WorkspaceWidgetId =
  | 'swap'
  | 'chart'
  | 'statistics'
  | 'log'
  | 'governance'
  | 'wallet'
  | 'automation'
  | 'wiki'
  | 'account-menu'
  | 'settings';

export type PanelId = WorkspaceWidgetId;
export type SidebarWidgetId = WorkspaceWidgetId;

export const ALL_WORKSPACE_WIDGETS: WorkspaceWidgetId[] = [
  'swap',
  'chart',
  'statistics',
  'log',
  'governance',
  'wallet',
  'automation',
  'wiki',
  'account-menu',
  'settings',
];

export const ALL_PANELS: PanelId[] = ALL_WORKSPACE_WIDGETS;

const WORKSPACE_WIDGET_SET = new Set<string>(ALL_WORKSPACE_WIDGETS);

export function isWorkspaceWidgetId(
  value: unknown,
): value is WorkspaceWidgetId {
  return typeof value === 'string' && WORKSPACE_WIDGET_SET.has(value);
}

export const WIDGET_LABELS: Record<WorkspaceWidgetId, string> = {
  swap: 'Swap',
  chart: 'Chart',
  statistics: 'Statistics',
  log: 'Log',
  governance: 'Governance',
  wallet: 'Wallet',
  automation: 'Automation',
  wiki: 'Wiki',
  'account-menu': 'Account',
  settings: 'Settings',
};

export const PANEL_LABELS: Record<PanelId, string> = WIDGET_LABELS;

export type TileLeaf = {
  type: 'leaf';
  id: string;
  tabs: PanelId[];
  activeTab: PanelId;
};

export type TileSplit = {
  type: 'split';
  id: string;
  direction: 'horizontal' | 'vertical';
  ratio: number;
  children: [TileNode, TileNode];
};

export type TileNode = TileLeaf | TileSplit;

export type DropEdge = 'right' | 'bottom' | 'left';

export type DragTabState = {
  tabId: PanelId;
  sourceLeafId: string | null;
};

export type DragLeafState = {
  sourceLeafId: string;
};

export const MOBILE_LAYOUT_BREAKPOINT = 640;
export const MIN_PANE_STACK_HEIGHT_PX = 96;
export const SPLIT_HANDLE_EXTENT_PX = 12;
export const MAX_TILE_GRID_COLUMNS = 4;
export const MAX_TILE_GRID_ROWS = 4;
export const MAX_TILE_LEAF_COUNT = MAX_TILE_GRID_COLUMNS * MAX_TILE_GRID_ROWS;

export type ReservedLaneId = 'header' | 'footer' | 'sidebar';
export type ReservedLaneWidgetId =
  | 'account-chip'
  | 'account-menu'
  | 'settings'
  | 'status';
export const SIDEBAR_WIDGET_IDS: SidebarWidgetId[] = [
  'account-menu',
  'settings',
];
export type SidebarLaneEdge = 'left' | 'right';
export type ReservedLaneSpec = {
  id: ReservedLaneId;
  edge: 'top' | 'bottom' | SidebarLaneEdge;
  linear: true;
  tabbed: false;
  userMutable: false;
  widgets: ReservedLaneWidgetId[];
  mobileWidgets?: ReservedLaneWidgetId[];
};
export type SidebarLaneState = {
  placementVersion: 1;
  open: boolean;
  widgetOrder: SidebarWidgetId[];
  expandedWidgetId: SidebarWidgetId | null;
};
export type MobileWorkspaceState = {
  panelOrder: PanelId[];
  expandedPanelId: PanelId | null;
};
export type WorkspaceFrameState = {
  sidebar: SidebarLaneState;
  mobile: MobileWorkspaceState;
};

export const RESERVED_LANE_SPECS: Record<ReservedLaneId, ReservedLaneSpec> = {
  header: {
    id: 'header',
    edge: 'top',
    linear: true,
    tabbed: false,
    userMutable: false,
    widgets: ['account-chip'],
    mobileWidgets: ['account-chip'],
  },
  footer: {
    id: 'footer',
    edge: 'bottom',
    linear: true,
    tabbed: false,
    userMutable: false,
    widgets: ['status'],
    mobileWidgets: ['status'],
  },
  sidebar: {
    id: 'sidebar',
    edge: 'right',
    linear: true,
    tabbed: false,
    userMutable: false,
    widgets: ['account-menu', 'settings'],
    mobileWidgets: ['account-menu', 'settings'],
  },
};

export function reservedLaneWidgetsFor(
  laneId: ReservedLaneId,
  mobile: boolean,
): ReservedLaneWidgetId[] {
  const spec = RESERVED_LANE_SPECS[laneId];
  return mobile && spec.mobileWidgets ? spec.mobileWidgets : spec.widgets;
}
