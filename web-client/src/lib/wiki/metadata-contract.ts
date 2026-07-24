/*
Domain: Wiki metadata contracts
Owns: TypeScript shapes for generated wiki navigation, graph, aliases, and state manifests consumed by the client.
Excludes: Wiki rendering, metadata generation, validation scripts, and widget presentation state.
Zone: Wiki domain contract; shared by wiki helpers and widgets without importing generated JSON directly.
*/

export type LocalizedValue = Record<string, string>;

export type WikiGraphNode = {
  id: string;
  title: LocalizedValue;
  page_type: string;
  path: LocalizedValue;
};

export type WikiGraphEdge = {
  from: string;
  to: string;
  type: string;
};

export type WikiGraphManifest = {
  default_locale: string;
  available_locales: string[];
  nodes: WikiGraphNode[];
  edges: WikiGraphEdge[];
};

export type WikiAliasManifest = {
  default_locale: string;
  available_locales: string[];
  aliases: Record<string, Record<string, string>>;
};

export type WikiStatePage = {
  path: LocalizedValue;
  page_type: string;
  title: LocalizedValue;
  status: string;
  audience: string;
  confidence: number;
  sources: Record<string, string[]>;
};

export type WikiStateManifest = {
  generated_at: string;
  mode: string;
  source_root: string;
  default_locale: string;
  available_locales: string[];
  pages: Record<string, WikiStatePage>;
};

export type WikiSearchPage = {
  id: string;
  text: LocalizedValue;
};

export type WikiSearchManifest = {
  generated_at: string;
  max_chars_per_page: number;
  default_locale: string;
  available_locales: string[];
  pages: WikiSearchPage[];
};

export type WikiNavigationItem = {
  id: string;
  title: LocalizedValue;
  path: LocalizedValue;
  page_type: string;
  summary: LocalizedValue;
};

export type WikiNavigationSection = {
  id: string;
  title: LocalizedValue;
  items: WikiNavigationItem[];
};

export type WikiNavigationManifest = {
  default_locale: string;
  available_locales: string[];
  entrypoints: string[];
  sections: WikiNavigationSection[];
};

export type ResolvedWikiNavigationItem = {
  id: string;
  title: string;
  path: string;
  page_type: string;
  summary: string;
};

export type ResolvedWikiNavigationSection = {
  id: string;
  title: string;
  items: ResolvedWikiNavigationItem[];
};

export type RelatedWikiItem = {
  id: string;
  title: string;
  path: string;
  summary: string;
  relation: string;
};
