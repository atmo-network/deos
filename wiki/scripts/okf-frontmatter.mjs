#!/usr/bin/env node

// One parser is shared by the Wiki corpus checks and mdBook projection.
export {
  splitFrontmatter,
  parseFrontmatter,
  stringifyFrontmatter,
  readConcept,
} from '../site/src/frontmatter.mjs';
