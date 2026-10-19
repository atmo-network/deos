/*
Domain: Native mdBook Unicode search
Owns: Unicode token boundaries and pre-index eligibility of native Elasticlunr stored sections.
Excludes: Markdown/HTML rendering, a second search engine and untrusted-script sandboxing.
*/
import { Buffer } from 'node:buffer';
import { createContext, runInContext } from 'node:vm';

const label = 'wikiUnicodeTrim';
const maxTokenBytes = 80; // mdBook's search::MAX_WORD_LENGTH_TO_INDEX.

export function trimSearchToken(token) {
  return token.replace(/^[^\p{L}\p{N}\p{M}_]+|[^\p{L}\p{N}\p{M}_]+$/gu, '');
}

export function nativeSections(data, eligibleDocuments) {
  const docs = data.index?.documentStore?.docs;
  if (!docs || typeof docs !== 'object' || Array.isArray(docs) || !Array.isArray(data.doc_urls) || Object.keys(docs).length !== data.doc_urls.length) throw new Error('Native stored sections and destinations disagree');
  if (eligibleDocuments !== undefined && !(eligibleDocuments instanceof Set)) throw new Error('Eligible native documents must be a Set');
  return Object.entries(docs).filter(([ref]) => {
    if (!/^(0|[1-9][0-9]*)$/.test(ref) || Number(ref) >= data.doc_urls.length || typeof data.doc_urls[Number(ref)] !== 'string') throw new Error(`Unsupported native section reference: ${ref}`);
    return eligibleDocuments === undefined || eligibleDocuments.has(data.doc_urls[Number(ref)].split('#')[0]);
  });
}

export function rebuildNativeSearch(library, indexScript, eligibleDocuments) {
  // Both scripts come directly from the pinned renderer in an owned build stage.
  // node:vm is isolation for globals, not a security sandbox for remote scripts.
  const context = createContext({ console, window: { search: {} } });
  runInContext(library, context, { timeout: 5000 });
  runInContext(indexScript, context, { timeout: 5000 });
  const elasticlunr = context.elasticlunr ?? context.window.elasticlunr;
  const data = context.window.search;
  const original = data.index;
  if (!elasticlunr?.Pipeline || !Array.isArray(original?.fields) || typeof original.ref !== 'string' || !Array.isArray(original.pipeline) || !original.pipeline.includes('trimmer')) throw new Error('Unsupported native Elasticlunr index contract');
  const sections = nativeSections(data, eligibleDocuments);
  const setup = `elasticlunr.Pipeline.registerFunction(${trimSearchToken.toString()}, ${JSON.stringify(label)});\n`;
  const trimForIndex = token => Buffer.byteLength(token, 'utf8') <= maxTokenBytes ? trimSearchToken(token) : undefined;
  elasticlunr.Pipeline.registerFunction(trimForIndex, label);
  const index = new elasticlunr.Index();
  index.setRef(original.ref);
  for (const field of original.fields) index.addField(field);
  index.pipeline.add(...original.pipeline.map(name => {
    const step = elasticlunr.Pipeline.registeredFunctions[name === 'trimmer' ? label : name];
    if (!step) throw new Error(`Unsupported native search pipeline step: ${name}`);
    return step;
  }));
  for (const [ref, doc] of sections) index.addDoc({ ...doc, [original.ref]: ref });
  data.index = index.toJSON();
  return {
    script: setup + `window.search = Object.assign(window.search, JSON.parse(${JSON.stringify(JSON.stringify(data))}));\n`,
    sections: sections.length,
  };
}

export function rebindSearchAsset(html, previous, next) {
  let bindings = 0;
  const content = html.replace(/<script>([\s\S]*?)<\/script>/g, (script, body) => {
    if (!body.includes('const path_to_root = ')) return script;
    return script.replace(/(window\.path_to_searchindex_js = ")([^"]+)(";)/g, (assignment, prefix, path, suffix) => {
      if (!path.endsWith('/' + previous) && path !== previous) throw new Error('Unexpected native search asset binding');
      bindings++;
      return prefix + path.slice(0, -previous.length) + next + suffix;
    });
  });
  return { content, bindings };
}
