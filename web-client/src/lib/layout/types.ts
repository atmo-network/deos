export type PanelId = "swap" | "chart" | "statistics" | "log" | "governance" | "wallet" | "automation" | "wiki";

export const ALL_PANELS: PanelId[] = [
  "swap",
  "chart",
  "statistics",
  "log",
  "governance",
  "wallet",
  "automation",
  "wiki",
];

export const PANEL_LABELS: Record<PanelId, string> = {
  swap: "Swap",
  chart: "Chart",
  statistics: "Statistics",
  log: "Log",
  governance: "Governance",
  wallet: "Wallet",
  automation: "Automation",
  wiki: "Wiki",
};

export type TileLeaf = {
  type: "leaf";
  id: string;
  tabs: PanelId[];
  activeTab: PanelId;
};

export type TileSplit = {
  type: "split";
  id: string;
  direction: "horizontal" | "vertical";
  ratio: number;
  children: [TileNode, TileNode];
};

export type TileNode = TileLeaf | TileSplit;

export type DropEdge = "right" | "bottom" | "left";

export type DragTabState = {
  tabId: PanelId;
  sourceLeafId: string;
};

export type DragLeafState = {
  sourceLeafId: string;
};

export const MOBILE_LAYOUT_BREAKPOINT = 640;
export const MAX_TILE_GRID_COLUMNS = 4;
export const MAX_TILE_GRID_ROWS = 4;
export const MAX_TILE_LEAF_COUNT = MAX_TILE_GRID_COLUMNS * MAX_TILE_GRID_ROWS;

export type ReservedLaneId = "header" | "footer" | "sidebar";
export type ReservedLaneWidgetId = "account-chip" | "account-menu" | "settings" | "status";
export type SidebarLaneEdge = "left" | "right";
export type ReservedLaneSpec = {
  id: ReservedLaneId;
  edge: "top" | "bottom" | SidebarLaneEdge;
  linear: true;
  tabbed: false;
  userMutable: false;
  widgets: ReservedLaneWidgetId[];
  mobileWidgets?: ReservedLaneWidgetId[];
};
export type SidebarLaneState = {
  open: boolean;
  edge: SidebarLaneEdge;
};
export type WorkspaceFrameState = {
  sidebar: SidebarLaneState;
};

export const RESERVED_LANE_SPECS: Record<ReservedLaneId, ReservedLaneSpec> = {
  header: {
    id: "header",
    edge: "top",
    linear: true,
    tabbed: false,
    userMutable: false,
    widgets: ["account-chip"],
    mobileWidgets: ["account-chip"],
  },
  footer: {
    id: "footer",
    edge: "bottom",
    linear: true,
    tabbed: false,
    userMutable: false,
    widgets: ["status"],
    mobileWidgets: ["status"],
  },
  sidebar: {
    id: "sidebar",
    edge: "right",
    linear: true,
    tabbed: false,
    userMutable: false,
    widgets: ["account-menu", "settings"],
    mobileWidgets: ["account-menu", "settings"],
  },
};

export function reservedLaneWidgetsFor(
  laneId: ReservedLaneId,
  mobile: boolean,
): ReservedLaneWidgetId[] {
  const spec = RESERVED_LANE_SPECS[laneId];
  return mobile && spec.mobileWidgets ? spec.mobileWidgets : spec.widgets;
}
