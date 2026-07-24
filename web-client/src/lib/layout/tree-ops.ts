/*
Domain: Layout tree operations
Owns: Pure tile-tree mutations for tabs, splits, collapse, extraction, merge, and drop insertion.
Excludes: Store persistence, DOM drag events, widget loading, and visual rendering.
Zone: Layout algorithm helper; depends only on layout contracts.
*/
import type { DropEdge, PanelId, TileLeaf, TileNode } from './types';

function cloneLeaf(leaf: TileLeaf): TileLeaf {
  return {
    ...leaf,
    tabs: [...leaf.tabs],
  };
}

export function removeTabFromLeaf(
  node: TileNode,
  leafId: string,
  tabId: PanelId,
): TileNode {
  if (node.type === 'leaf') {
    if (node.id !== leafId) {
      return node;
    }
    const tabs = node.tabs.filter((tab) => tab !== tabId);
    return {
      ...node,
      tabs,
      activeTab: node.activeTab === tabId ? tabs[0] : node.activeTab,
    };
  }
  return {
    ...node,
    children: [
      removeTabFromLeaf(node.children[0], leafId, tabId),
      removeTabFromLeaf(node.children[1], leafId, tabId),
    ],
  };
}

export function splitLeafWithTab(
  node: TileNode,
  leafId: string,
  tabId: PanelId,
  edge: DropEdge,
  genId: () => string,
): TileNode {
  if (node.type === 'leaf') {
    if (node.id !== leafId) {
      return node;
    }
    const newLeaf: TileLeaf = {
      type: 'leaf',
      id: genId(),
      tabs: [tabId],
      activeTab: tabId,
    };
    const direction =
      edge === 'left' || edge === 'right' ? 'horizontal' : 'vertical';
    const first = edge === 'left' ? newLeaf : node;
    const second = edge === 'left' ? node : newLeaf;
    return {
      type: 'split',
      id: genId(),
      direction,
      ratio: 0.5,
      children: [first, second],
    };
  }
  return {
    ...node,
    children: [
      splitLeafWithTab(node.children[0], leafId, tabId, edge, genId),
      splitLeafWithTab(node.children[1], leafId, tabId, edge, genId),
    ],
  };
}

export function addTabToLeaf(
  node: TileNode,
  leafId: string,
  tabId: PanelId,
  index?: number,
): TileNode {
  if (node.type === 'leaf') {
    if (node.id !== leafId) {
      return node;
    }
    const tabs = [...node.tabs];
    if (index !== undefined) {
      tabs.splice(index, 0, tabId);
    } else {
      tabs.push(tabId);
    }
    return { ...node, tabs, activeTab: tabId };
  }
  return {
    ...node,
    children: [
      addTabToLeaf(node.children[0], leafId, tabId, index),
      addTabToLeaf(node.children[1], leafId, tabId, index),
    ],
  };
}

export function collapseEmpty(node: TileNode): TileNode {
  if (node.type === 'leaf') {
    return node;
  }
  const left = collapseEmpty(node.children[0]);
  const right = collapseEmpty(node.children[1]);
  if (left.type === 'leaf' && left.tabs.length === 0) {
    return right;
  }
  if (right.type === 'leaf' && right.tabs.length === 0) {
    return left;
  }
  return { ...node, children: [left, right] };
}

export function extractLeaf(
  node: TileNode,
  leafId: string,
): { leaf: TileLeaf | null; node: TileNode | null } {
  if (node.type === 'leaf') {
    return node.id === leafId
      ? { leaf: cloneLeaf(node), node: null }
      : { leaf: null, node };
  }
  const left = extractLeaf(node.children[0], leafId);
  if (left.leaf) {
    if (!left.node) {
      return { leaf: left.leaf, node: node.children[1] };
    }
    return {
      leaf: left.leaf,
      node: {
        ...node,
        children: [left.node, node.children[1]],
      },
    };
  }
  const right = extractLeaf(node.children[1], leafId);
  if (right.leaf) {
    if (!right.node) {
      return { leaf: right.leaf, node: node.children[0] };
    }
    return {
      leaf: right.leaf,
      node: {
        ...node,
        children: [node.children[0], right.node],
      },
    };
  }
  return { leaf: null, node };
}

export function splitLeafWithExistingLeaf(
  node: TileNode,
  leafId: string,
  incomingLeaf: TileLeaf,
  edge: DropEdge,
  genId: () => string,
): TileNode {
  if (node.type === 'leaf') {
    if (node.id !== leafId) {
      return node;
    }
    const liftedLeaf = cloneLeaf(incomingLeaf);
    const direction =
      edge === 'left' || edge === 'right' ? 'horizontal' : 'vertical';
    const first = edge === 'left' ? liftedLeaf : node;
    const second = edge === 'left' ? node : liftedLeaf;
    return {
      type: 'split',
      id: genId(),
      direction,
      ratio: 0.5,
      children: [first, second],
    };
  }
  return {
    ...node,
    children: [
      splitLeafWithExistingLeaf(
        node.children[0],
        leafId,
        incomingLeaf,
        edge,
        genId,
      ),
      splitLeafWithExistingLeaf(
        node.children[1],
        leafId,
        incomingLeaf,
        edge,
        genId,
      ),
    ],
  };
}

export function mergeLeafIntoLeaf(
  node: TileNode,
  leafId: string,
  incomingLeaf: TileLeaf,
): TileNode {
  if (node.type === 'leaf') {
    if (node.id !== leafId) {
      return node;
    }
    return {
      ...node,
      tabs: [...node.tabs, ...incomingLeaf.tabs],
      activeTab: incomingLeaf.activeTab,
    };
  }
  return {
    ...node,
    children: [
      mergeLeafIntoLeaf(node.children[0], leafId, incomingLeaf),
      mergeLeafIntoLeaf(node.children[1], leafId, incomingLeaf),
    ],
  };
}

function rebalanceDirectionalSplit(
  node: TileNode,
  direction: 'horizontal' | 'vertical',
  weights: ReadonlyMap<string, number>,
): { node: TileNode; weight: number } {
  if (node.type === 'leaf' || node.direction !== direction) {
    return { node, weight: Math.max(0, weights.get(node.id) ?? 0) };
  }
  const first = rebalanceDirectionalSplit(node.children[0], direction, weights);
  const second = rebalanceDirectionalSplit(
    node.children[1],
    direction,
    weights,
  );
  const totalWeight = first.weight + second.weight;
  const nextRatio = totalWeight > 0 ? first.weight / totalWeight : node.ratio;
  const nextNode =
    first.node === node.children[0] &&
    second.node === node.children[1] &&
    nextRatio === node.ratio
      ? node
      : {
          ...node,
          ratio: nextRatio,
          children: [first.node, second.node] as [TileNode, TileNode],
        };
  return { node: nextNode, weight: totalWeight };
}

export function updateDirectionalSplitWeights(
  node: TileNode,
  splitId: string,
  weights: ReadonlyMap<string, number>,
): TileNode {
  if (node.type === 'leaf') {
    return node;
  }
  if (node.id === splitId) {
    return rebalanceDirectionalSplit(node, node.direction, weights).node;
  }
  const first = updateDirectionalSplitWeights(
    node.children[0],
    splitId,
    weights,
  );
  const second = updateDirectionalSplitWeights(
    node.children[1],
    splitId,
    weights,
  );
  if (first === node.children[0] && second === node.children[1]) {
    return node;
  }
  return { ...node, children: [first, second] };
}

export function setActiveInLeaf(
  node: TileNode,
  leafId: string,
  tabId: PanelId,
): TileNode {
  if (node.type === 'leaf') {
    if (node.id !== leafId || !node.tabs.includes(tabId)) {
      return node;
    }
    return { ...node, activeTab: tabId };
  }
  return {
    ...node,
    children: [
      setActiveInLeaf(node.children[0], leafId, tabId),
      setActiveInLeaf(node.children[1], leafId, tabId),
    ],
  };
}

export function reorderTabInLeaf(
  node: TileNode,
  leafId: string,
  tabId: PanelId,
  newIndex: number,
): TileNode {
  if (node.type === 'leaf') {
    if (node.id !== leafId) {
      return node;
    }
    const filtered = node.tabs.filter((tab) => tab !== tabId);
    filtered.splice(Math.min(newIndex, filtered.length), 0, tabId);
    return { ...node, tabs: filtered };
  }
  return {
    ...node,
    children: [
      reorderTabInLeaf(node.children[0], leafId, tabId, newIndex),
      reorderTabInLeaf(node.children[1], leafId, tabId, newIndex),
    ],
  };
}
