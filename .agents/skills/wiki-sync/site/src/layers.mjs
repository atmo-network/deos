/*
Domain: Wiki semantic layer views
Owns: Explicit locale-independent membership and induced graph/linear projections.
Excludes: Storage routes, inference from article text, browser controls and search ranking.
*/
export function assignMemberships(nodes, layers, overrides = {}) {
  const declared = layers.map(layer => layer.id);
  if (declared.includes('all')) throw new Error('The virtual layer all is reserved');
  if (!overrides || typeof overrides !== 'object' || Array.isArray(overrides)) throw new Error('Layer memberships must be an object');
  const identities = new Set(nodes.map(node => node.id));
  for (const [id, membership] of Object.entries(overrides)) {
    if (!identities.has(id) || !Array.isArray(membership) || membership.length === 0 || new Set(membership).size !== membership.length || membership.some(layer => !declared.includes(layer))) throw new Error(`Invalid layer membership: ${id}`);
  }
  return nodes.map(node => ({ ...node, layers: Object.hasOwn(overrides, node.id) ? declared.filter(layer => overrides[node.id].includes(layer)) : [node.plane] }));
}

export function selectLayer(graph, layer = 'all') {
  if (layer === 'all') return graph;
  if (!graph.layers.some(candidate => candidate.id === layer)) throw new Error(`Unknown knowledge layer: ${layer}`);
  const nodes = graph.nodes.filter(node => node.layers.includes(layer));
  const identities = new Set(nodes.map(node => node.id));
  return { ...graph, nodes, edges: graph.edges.filter(edge => identities.has(edge.from) && identities.has(edge.to)), linear_order: graph.linear_order.filter(id => identities.has(id)) };
}
