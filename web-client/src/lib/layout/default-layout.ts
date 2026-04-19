import type {
  PanelId,
  TileLeaf,
  TileNode,
  TileSplit,
  WorkspaceFrameState,
} from "./types";

export type LayoutLeafSpec = {
  type: "leaf";
  tabs: PanelId[];
  activeTab: PanelId;
};

export type LayoutSplitSpec = {
  type: "split";
  direction: "horizontal" | "vertical";
  ratio: number;
  children: [LayoutNodeSpec, LayoutNodeSpec];
};

export type LayoutNodeSpec = LayoutLeafSpec | LayoutSplitSpec;

export const CANONICAL_DEFAULT_LAYOUT_SPEC = {
  type: "split",
  direction: "horizontal",
  ratio: 0.515,
  children: [
    {
      type: "split",
      direction: "vertical",
      ratio: 0.78,
      children: [
        {
          type: "leaf",
          tabs: ["swap", "wallet"],
          activeTab: "swap",
        },
        {
          type: "leaf",
          tabs: ["log", "statistics"],
          activeTab: "statistics",
        },
      ],
    },
    {
      type: "leaf",
      tabs: ["chart", "automation", "governance", "wiki"],
      activeTab: "automation",
    },
  ],
} satisfies LayoutSplitSpec;

export const LEGACY_SHIPPED_DEFAULT_LAYOUT_SPEC = {
  type: "split",
  direction: "horizontal",
  ratio: 0.25,
  children: [
    {
      type: "leaf",
      tabs: ["governance", "wiki"],
      activeTab: "governance",
    },
    {
      type: "split",
      direction: "horizontal",
      ratio: 0.3,
      children: [
        {
          type: "leaf",
          tabs: ["swap", "wallet"],
          activeTab: "swap",
        },
        {
          type: "split",
          direction: "vertical",
          ratio: 0.6,
          children: [
            {
              type: "leaf",
              tabs: ["chart"],
              activeTab: "chart",
            },
            {
              type: "split",
              direction: "horizontal",
              ratio: 0.5,
              children: [
                {
                  type: "leaf",
                  tabs: ["statistics", "automation"],
                  activeTab: "statistics",
                },
                {
                  type: "leaf",
                  tabs: ["log"],
                  activeTab: "log",
                },
              ],
            },
          ],
        },
      ],
    },
  ],
} satisfies LayoutSplitSpec;

export const CANONICAL_DEFAULT_FRAME_STATE: WorkspaceFrameState = {
  sidebar: {
    open: false,
    edge: "right",
  },
};

export function buildLayoutFromSpec(
  spec: LayoutNodeSpec,
  genId: () => string,
): TileNode {
  if (spec.type === "leaf") {
    return {
      type: "leaf",
      id: genId(),
      tabs: [...spec.tabs],
      activeTab: spec.activeTab,
    } satisfies TileLeaf;
  }
  return {
    type: "split",
    id: genId(),
    direction: spec.direction,
    ratio: spec.ratio,
    children: [
      buildLayoutFromSpec(spec.children[0], genId),
      buildLayoutFromSpec(spec.children[1], genId),
    ],
  } satisfies TileSplit;
}

export function matchesLayoutSpec(
  node: TileNode,
  spec: LayoutNodeSpec,
): boolean {
  if (node.type !== spec.type) {
    return false;
  }
  if (node.type === "leaf" && spec.type === "leaf") {
    return (
      node.activeTab === spec.activeTab &&
      node.tabs.length === spec.tabs.length &&
      node.tabs.every((tab, index) => tab === spec.tabs[index])
    );
  }
  if (node.type === "split" && spec.type === "split") {
    return (
      node.direction === spec.direction &&
      matchesLayoutSpec(node.children[0], spec.children[0]) &&
      matchesLayoutSpec(node.children[1], spec.children[1])
    );
  }
  return false;
}
