import {
  ALL_PANELS,
  type PanelId,
  type TileNode,
  type WorkspaceFrameState,
} from "./types";

function normalizeLegacyPanelId(panelId: string): PanelId | null {
  if (panelId === "info" || panelId === "status") {
    return "statistics";
  }
  if (panelId === "actors") {
    return "automation";
  }
  if (panelId === "activity") {
    return "log";
  }
  return ALL_PANELS.includes(panelId as PanelId) ? (panelId as PanelId) : null;
}

export function normalizeLegacyLayout(
  node: unknown,
  genId: () => string,
): TileNode | null {
  if (!node || typeof node !== "object") {
    return null;
  }
  const candidate = node as Record<string, unknown>;
  if (candidate.type === "leaf") {
    const id = typeof candidate.id === "string" ? candidate.id : genId();
    const tabs = Array.isArray(candidate.tabs)
      ? Array.from(
          new Set(
            candidate.tabs
              .map((panel) =>
                typeof panel === "string"
                  ? normalizeLegacyPanelId(panel)
                  : null,
              )
              .filter((panel): panel is PanelId => panel !== null),
          ),
        )
      : [];
    const activeTab =
      typeof candidate.activeTab === "string"
        ? normalizeLegacyPanelId(candidate.activeTab)
        : null;
    if (tabs.length === 0) {
      return null;
    }
    return {
      type: "leaf",
      id,
      tabs,
      activeTab: activeTab && tabs.includes(activeTab) ? activeTab : tabs[0],
    };
  }
  if (candidate.type === "split") {
    const children = Array.isArray(candidate.children)
      ? candidate.children
      : null;
    if (!children || children.length < 2) {
      return null;
    }
    const left = normalizeLegacyLayout(children[0], genId);
    const right = normalizeLegacyLayout(children[1], genId);
    if (!left || !right) {
      return null;
    }
    return {
      type: "split",
      id: typeof candidate.id === "string" ? candidate.id : genId(),
      direction: candidate.direction === "vertical" ? "vertical" : "horizontal",
      ratio: typeof candidate.ratio === "number" ? candidate.ratio : 0.5,
      children: [left, right],
    };
  }
  return null;
}

export function normalizeFrameState(
  candidate: unknown,
): WorkspaceFrameState | null {
  if (!candidate || typeof candidate !== "object") {
    return null;
  }
  const value = candidate as Record<string, unknown>;
  const sidebar = value.sidebar;
  if (!sidebar || typeof sidebar !== "object") {
    return null;
  }
  const sidebarValue = sidebar as Record<string, unknown>;
  const edge =
    sidebarValue.edge === "left"
      ? "left"
      : sidebarValue.edge === "right"
        ? "right"
        : null;
  const open =
    typeof sidebarValue.open === "boolean" ? sidebarValue.open : null;
  if (edge === null || open === null) {
    return null;
  }
  return {
    sidebar: {
      edge,
      open,
    },
  };
}
