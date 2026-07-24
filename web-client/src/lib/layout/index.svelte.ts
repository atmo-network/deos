/*
Domain: Layout store
Owns: Workspace frame state, tile tree mutations, persistence integration, and lane/sidebar UI state.
Excludes: Widget internals, domain data stores, adapter transport, and UI Kit implementation.
Zone: Layout state slice; may depend on layout helpers and system persistence only.
*/
import { readStoredJson, writeStoredJson } from '$lib/system/persistence';

import {
  CANONICAL_DEFAULT_FRAME_STATE,
  CANONICAL_DEFAULT_LAYOUT_SPEC,
  LEGACY_SHIPPED_DEFAULT_LAYOUT_SPEC,
  buildSplitLayoutFromSpec,
  matchesLayoutSpec,
} from './default-layout';
import {
  normalizeFrameState,
  normalizeLegacyLayout,
} from './legacy-normalization';
import {
  mobileOrderFromProjection,
  moveMobilePanel as moveMobilePanelInOrder,
  projectMobilePanels,
  resolveMobileExpandedPanel,
} from './mobile-projection';
import {
  insertSidebarWidget,
  moveSidebarWidget as moveSidebarWidgetInOrder,
  normalizeSidebarWidgetOrder,
  resolveSidebarExpandedWidget,
} from './sidebar-projection';
import { genTileId, recalcNextTileId, resetTileIdSequence } from './tree-ids';
import {
  addTabToLeaf,
  collapseEmpty,
  extractLeaf,
  mergeLeafIntoLeaf,
  removeTabFromLeaf,
  reorderTabInLeaf,
  setActiveInLeaf,
  splitLeafWithExistingLeaf,
  splitLeafWithTab,
  updateDirectionalSplitWeights,
} from './tree-ops';
import {
  countLeaves,
  countPanels,
  findFirstLeaf,
  findLeaf,
  findLeafContainingPanel,
  isValidTree,
} from './tree-utils';
import type {
  DragLeafState,
  DragTabState,
  DropEdge,
  PanelId,
  SidebarWidgetId,
  TileNode,
  TileSplit,
  WorkspaceFrameState,
} from './types';
import { MAX_TILE_LEAF_COUNT } from './types';
import { reconcileWorkspacePlacement } from './widget-placement';

const TILE_LAYOUT_STORAGE_KEY = 'deos-tile-layout';
const WORKSPACE_FRAME_STORAGE_KEY = 'deos-workspace-frame';

function createDefaultLayout(): TileSplit {
  return buildSplitLayoutFromSpec(CANONICAL_DEFAULT_LAYOUT_SPEC, genTileId);
}

function hasTileNodeTypeCandidate(
  value: unknown,
): value is { type: 'leaf' | 'split' } {
  return (
    typeof value === 'object' &&
    value !== null &&
    'type' in value &&
    (value.type === 'leaf' || value.type === 'split')
  );
}

function createDefaultFrameState(): WorkspaceFrameState {
  return {
    sidebar: {
      ...CANONICAL_DEFAULT_FRAME_STATE.sidebar,
      widgetOrder: [...CANONICAL_DEFAULT_FRAME_STATE.sidebar.widgetOrder],
    },
    mobile: {
      panelOrder: [...CANONICAL_DEFAULT_FRAME_STATE.mobile.panelOrder],
      expandedPanelId: CANONICAL_DEFAULT_FRAME_STATE.mobile.expandedPanelId,
    },
  };
}

class LayoutStore {
  root: TileNode = $state(createDefaultLayout());
  frame: WorkspaceFrameState = $state(createDefaultFrameState());
  dragTab: DragTabState | null = $state(null);
  dragLeaf: DragLeafState | null = $state(null);

  constructor() {
    this.load();
  }

  private load() {
    const parsed = readStoredJson(TILE_LAYOUT_STORAGE_KEY);
    if (hasTileNodeTypeCandidate(parsed)) {
      const normalized = normalizeLegacyLayout(parsed, genTileId);
      if (normalized && isValidTree(normalized)) {
        if (matchesLayoutSpec(normalized, LEGACY_SHIPPED_DEFAULT_LAYOUT_SPEC)) {
          resetTileIdSequence();
          this.root = createDefaultLayout();
          this.persist();
        } else {
          recalcNextTileId(normalized);
          this.root = normalized;
        }
      }
    }
    const parsedFrame = readStoredJson(WORKSPACE_FRAME_STORAGE_KEY);
    const normalizedFrame = normalizeFrameState(parsedFrame);
    if (normalizedFrame) {
      const sidebarWidgetOrder = normalizeSidebarWidgetOrder(
        normalizedFrame.sidebar.widgetOrder,
      );
      this.frame = {
        ...normalizedFrame,
        sidebar: {
          ...normalizedFrame.sidebar,
          widgetOrder: sidebarWidgetOrder,
          expandedWidgetId: normalizedFrame.sidebar.open
            ? (sidebarWidgetOrder[0] ?? null)
            : normalizedFrame.sidebar.expandedWidgetId,
        },
      };
    }

    const rootBeforeReconciliation = JSON.stringify(this.root);
    const sidebarBeforeReconciliation = JSON.stringify(this.frame.sidebar);
    const placement = reconcileWorkspacePlacement(
      this.root,
      this.frame.sidebar.widgetOrder,
    );
    const expandedWidgetId = placement.sidebarOrder.includes(
      this.frame.sidebar.expandedWidgetId!,
    )
      ? this.frame.sidebar.expandedWidgetId
      : this.frame.sidebar.expandedWidgetId === null
        ? null
        : (placement.sidebarOrder[0] ?? null);
    this.root = placement.root;
    this.frame = {
      ...this.frame,
      sidebar: {
        ...this.frame.sidebar,
        placementVersion: 1,
        widgetOrder: placement.sidebarOrder,
        expandedWidgetId: this.frame.sidebar.open
          ? (placement.sidebarOrder[0] ?? null)
          : expandedWidgetId,
      },
    };
    if (
      JSON.stringify(this.root) !== rootBeforeReconciliation ||
      JSON.stringify(this.frame.sidebar) !== sidebarBeforeReconciliation
    ) {
      this.persist();
    }
  }

  private persist() {
    writeStoredJson(TILE_LAYOUT_STORAGE_KEY, this.root);
    writeStoredJson(WORKSPACE_FRAME_STORAGE_KEY, this.frame);
  }

  startDrag(tabId: PanelId, sourceLeafId: string) {
    this.dragLeaf = null;
    this.dragTab = { tabId, sourceLeafId };
  }

  startSidebarWidgetDrag(widgetId: SidebarWidgetId) {
    if (!this.frame.sidebar.widgetOrder.includes(widgetId)) {
      return;
    }
    this.dragLeaf = null;
    this.dragTab = { tabId: widgetId, sourceLeafId: null };
  }

  startPaneDrag(sourceLeafId: string) {
    this.dragTab = null;
    this.dragLeaf = { sourceLeafId };
  }

  endDrag() {
    this.dragTab = null;
    this.dragLeaf = null;
  }

  setActiveTab(leafId: string, tabId: PanelId) {
    this.root = setActiveInLeaf(this.root, leafId, tabId);
    this.persist();
  }

  activatePanel(panelId: PanelId) {
    const leaf = findLeafContainingPanel(this.root, panelId);
    if (!leaf || leaf.activeTab === panelId) {
      return;
    }
    this.setActiveTab(leaf.id, panelId);
  }

  dropOnEdge(targetLeafId: string, edge: DropEdge) {
    if (this.dragTab) {
      const { tabId, sourceLeafId } = this.dragTab;

      if (sourceLeafId === targetLeafId) {
        const leaf = findLeaf(this.root, sourceLeafId);
        if (leaf && leaf.tabs.length <= 1) {
          this.endDrag();
          return;
        }
      }

      let result =
        sourceLeafId === null
          ? this.root
          : removeTabFromLeaf(this.root, sourceLeafId, tabId);
      result = splitLeafWithTab(result, targetLeafId, tabId, edge, genTileId);
      result = collapseEmpty(result);
      if (countLeaves(result) > MAX_TILE_LEAF_COUNT) {
        this.endDrag();
        return;
      }
      this.root = result;
      if (sourceLeafId === null) {
        this.frame = {
          ...this.frame,
          sidebar: {
            ...this.frame.sidebar,
            widgetOrder: this.frame.sidebar.widgetOrder.filter(
              (widgetId) => widgetId !== tabId,
            ),
            expandedWidgetId:
              this.frame.sidebar.expandedWidgetId === tabId
                ? null
                : this.frame.sidebar.expandedWidgetId,
          },
        };
      }
      this.persist();
      this.endDrag();
      return;
    }

    if (!this.dragLeaf) {
      return;
    }
    const { sourceLeafId } = this.dragLeaf;
    if (sourceLeafId === targetLeafId) {
      this.endDrag();
      return;
    }
    const extracted = extractLeaf(this.root, sourceLeafId);
    if (!extracted.leaf || !extracted.node) {
      this.endDrag();
      return;
    }
    this.root = splitLeafWithExistingLeaf(
      extracted.node,
      targetLeafId,
      extracted.leaf,
      edge,
      genTileId,
    );
    this.persist();
    this.endDrag();
  }

  dropOnTabBar(targetLeafId: string, insertIndex?: number) {
    if (!this.dragTab) return;
    const { tabId, sourceLeafId } = this.dragTab;

    if (sourceLeafId === targetLeafId) {
      this.endDrag();
      return;
    }

    let result =
      sourceLeafId === null
        ? this.root
        : removeTabFromLeaf(this.root, sourceLeafId, tabId);
    result = addTabToLeaf(result, targetLeafId, tabId, insertIndex);
    result = collapseEmpty(result);

    this.root = result;
    if (sourceLeafId === null) {
      this.frame = {
        ...this.frame,
        sidebar: {
          ...this.frame.sidebar,
          widgetOrder: this.frame.sidebar.widgetOrder.filter(
            (widgetId) => widgetId !== tabId,
          ),
          expandedWidgetId:
            this.frame.sidebar.expandedWidgetId === tabId
              ? null
              : this.frame.sidebar.expandedWidgetId,
        },
      };
    }
    this.persist();
    this.endDrag();
  }

  dropOnSidebar(targetIndex: number) {
    if (!this.dragTab) return false;
    const { tabId, sourceLeafId } = this.dragTab;
    if (sourceLeafId !== null && countPanels(this.root) <= 1) {
      this.endDrag();
      return false;
    }

    if (sourceLeafId !== null) {
      this.root = collapseEmpty(
        removeTabFromLeaf(this.root, sourceLeafId, tabId),
      );
    }
    const widgetOrder = insertSidebarWidget(
      this.frame.sidebar.widgetOrder,
      tabId,
      targetIndex,
    );
    this.frame = {
      ...this.frame,
      sidebar: {
        ...this.frame.sidebar,
        open: true,
        widgetOrder,
        expandedWidgetId: tabId,
      },
    };
    this.persist();
    this.endDrag();
    return true;
  }

  moveTabToSidebar(tabId: PanelId, sourceLeafId: string) {
    this.startDrag(tabId, sourceLeafId);
    return this.dropOnSidebar(this.frame.sidebar.widgetOrder.length);
  }

  moveSidebarWidgetToFirstTile(widgetId: SidebarWidgetId) {
    if (!this.frame.sidebar.widgetOrder.includes(widgetId)) {
      return false;
    }
    const targetLeaf = findFirstLeaf(this.root);
    this.startSidebarWidgetDrag(widgetId);
    this.dropOnTabBar(targetLeaf.id, targetLeaf.tabs.length);
    return true;
  }

  dropPaneOnPlate(targetLeafId: string) {
    if (!this.dragLeaf) {
      return;
    }
    const { sourceLeafId } = this.dragLeaf;
    if (sourceLeafId === targetLeafId) {
      this.endDrag();
      return;
    }
    const extracted = extractLeaf(this.root, sourceLeafId);
    if (!extracted.leaf || !extracted.node) {
      this.endDrag();
      return;
    }
    this.root = mergeLeafIntoLeaf(extracted.node, targetLeafId, extracted.leaf);
    this.persist();
    this.endDrag();
  }

  reorderTab(leafId: string, tabId: PanelId, newIndex: number) {
    this.root = reorderTabInLeaf(this.root, leafId, tabId, newIndex);
    this.persist();
  }

  resizeDirectionalSplit(splitId: string, weights: Map<string, number>) {
    this.root = updateDirectionalSplitWeights(this.root, splitId, weights);
    this.persist();
  }

  setSidebarOpen(open: boolean) {
    const widgetOrder = normalizeSidebarWidgetOrder(
      this.frame.sidebar.widgetOrder,
    );
    this.frame = {
      ...this.frame,
      sidebar: {
        ...this.frame.sidebar,
        open,
        widgetOrder,
        expandedWidgetId: open
          ? (widgetOrder[0] ?? null)
          : this.frame.sidebar.expandedWidgetId,
      },
    };
    this.persist();
  }

  toggleSidebar() {
    this.setSidebarOpen(!this.frame.sidebar.open);
  }

  setSidebarExpandedWidget(widgetId: SidebarWidgetId | null) {
    const expandedWidgetId = resolveSidebarExpandedWidget(
      this.frame.sidebar.widgetOrder,
      widgetId,
    );
    if (this.frame.sidebar.expandedWidgetId === expandedWidgetId) {
      return;
    }
    this.frame = {
      ...this.frame,
      sidebar: {
        ...this.frame.sidebar,
        expandedWidgetId,
      },
    };
    this.persist();
  }

  toggleSidebarExpandedWidget(widgetId: SidebarWidgetId) {
    const order = normalizeSidebarWidgetOrder(this.frame.sidebar.widgetOrder);
    if (!order.includes(widgetId)) {
      return;
    }
    this.setSidebarExpandedWidget(
      this.frame.sidebar.expandedWidgetId === widgetId ? null : widgetId,
    );
  }

  moveSidebarWidget(widgetId: SidebarWidgetId, targetIndex: number) {
    const currentOrder = normalizeSidebarWidgetOrder(
      this.frame.sidebar.widgetOrder,
    );
    if (!currentOrder.includes(widgetId)) {
      return;
    }
    this.frame = {
      ...this.frame,
      sidebar: {
        ...this.frame.sidebar,
        widgetOrder: moveSidebarWidgetInOrder(
          currentOrder,
          widgetId,
          targetIndex,
        ),
      },
    };
    this.persist();
  }

  setMobileExpandedPanel(panelId: PanelId) {
    const projection = projectMobilePanels(
      this.root,
      this.frame.mobile.panelOrder,
    );
    const expandedPanelId = resolveMobileExpandedPanel(projection, panelId);
    if (
      !expandedPanelId ||
      this.frame.mobile.expandedPanelId === expandedPanelId
    ) {
      return;
    }
    this.frame = {
      ...this.frame,
      mobile: {
        ...this.frame.mobile,
        expandedPanelId,
      },
    };
    this.persist();
  }

  toggleMobileExpandedPanel(panelId: PanelId) {
    const projection = projectMobilePanels(
      this.root,
      this.frame.mobile.panelOrder,
    );
    if (!projection.some((entry) => entry.panelId === panelId)) {
      return;
    }
    this.frame = {
      ...this.frame,
      mobile: {
        ...this.frame.mobile,
        expandedPanelId:
          this.frame.mobile.expandedPanelId === panelId ? null : panelId,
      },
    };
    this.persist();
  }

  moveMobilePanel(panelId: PanelId, targetIndex: number) {
    const projection = projectMobilePanels(
      this.root,
      this.frame.mobile.panelOrder,
    );
    const currentOrder = mobileOrderFromProjection(projection);
    if (!currentOrder.includes(panelId)) {
      return;
    }
    this.frame = {
      ...this.frame,
      mobile: {
        ...this.frame.mobile,
        panelOrder: moveMobilePanelInOrder(currentOrder, panelId, targetIndex),
      },
    };
    this.persist();
  }

  resetLayout() {
    resetTileIdSequence();
    this.root = createDefaultLayout();
    this.frame = createDefaultFrameState();
    this.persist();
  }
}

export const layoutStore = new LayoutStore();
