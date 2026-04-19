import {
  buildLayoutFromSpec,
  CANONICAL_DEFAULT_FRAME_STATE,
  CANONICAL_DEFAULT_LAYOUT_SPEC,
  LEGACY_SHIPPED_DEFAULT_LAYOUT_SPEC,
  matchesLayoutSpec,
} from "./default-layout";
import { genTileId, recalcNextTileId, resetTileIdSequence } from "./tree-ids";
import {
  normalizeFrameState,
  normalizeLegacyLayout,
} from "./legacy-normalization";
import { readStoredJson, writeStoredJson } from "$lib/shared/persistence";
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
  updateSplitRatio,
} from "./tree-ops";
import { countLeaves, findLeaf, isValidTree } from "./tree-utils";
import type {
  DragLeafState,
  DragTabState,
  DropEdge,
  PanelId,
  SidebarLaneEdge,
  TileNode,
  TileSplit,
  WorkspaceFrameState,
} from "./types";
import { MAX_TILE_LEAF_COUNT } from "./types";

const TILE_LAYOUT_STORAGE_KEY = "tmctol-tile-layout";
const WORKSPACE_FRAME_STORAGE_KEY = "tmctol-workspace-frame";

function createDefaultLayout(): TileSplit {
  return buildLayoutFromSpec(CANONICAL_DEFAULT_LAYOUT_SPEC, genTileId) as TileSplit;
}

function createDefaultFrameState(): WorkspaceFrameState {
  return {
    sidebar: {
      ...CANONICAL_DEFAULT_FRAME_STATE.sidebar,
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
    const parsed = readStoredJson<unknown>(TILE_LAYOUT_STORAGE_KEY);
    if (
      parsed &&
      typeof parsed === "object" &&
      "type" in parsed &&
      ((parsed as Record<string, unknown>).type === "leaf" ||
        (parsed as Record<string, unknown>).type === "split")
    ) {
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
    const parsedFrame = readStoredJson<unknown>(WORKSPACE_FRAME_STORAGE_KEY);
    const normalizedFrame = normalizeFrameState(parsedFrame);
    if (normalizedFrame) {
      this.frame = normalizedFrame;
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

      let result = removeTabFromLeaf(this.root, sourceLeafId, tabId);
      result = splitLeafWithTab(result, targetLeafId, tabId, edge, genTileId);
      result = collapseEmpty(result);
      if (countLeaves(result) > MAX_TILE_LEAF_COUNT) {
        this.endDrag();
        return;
      }
      this.root = result;
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

    let result = removeTabFromLeaf(this.root, sourceLeafId, tabId);
    result = addTabToLeaf(result, targetLeafId, tabId, insertIndex);
    result = collapseEmpty(result);

    this.root = result;
    this.persist();
    this.endDrag();
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

  resizeSplit(splitId: string, ratio: number) {
    this.root = updateSplitRatio(this.root, splitId, ratio);
    this.persist();
  }

  setSidebarOpen(open: boolean) {
    this.frame = {
      ...this.frame,
      sidebar: {
        ...this.frame.sidebar,
        open,
      },
    };
    this.persist();
  }

  toggleSidebar() {
    this.setSidebarOpen(!this.frame.sidebar.open);
  }

  setSidebarEdge(edge: SidebarLaneEdge) {
    this.frame = {
      ...this.frame,
      sidebar: {
        ...this.frame.sidebar,
        edge,
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
