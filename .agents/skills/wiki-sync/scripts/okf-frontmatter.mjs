#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { parseDocument, stringify } from 'yaml';

export function splitFrontmatter(text, path = '<input>') {
  if (!text.startsWith('---\n')) {
    throw new Error(`${path}: missing opening frontmatter delimiter`);
  }
  const end = text.indexOf('\n---\n', 4);
  if (end < 0) {
    throw new Error(`${path}: missing closing frontmatter delimiter`);
  }
  return {
    frontmatter: text.slice(4, end),
    body: text.slice(end + 5),
  };
}

export function parseFrontmatter(frontmatter, path = '<input>') {
  const document = parseDocument(frontmatter, {
    prettyErrors: false,
    strict: true,
    uniqueKeys: true,
  });
  if (document.errors.length) {
    const detail = document.errors.map((error) => error.message).join('; ');
    throw new Error(`${path}: invalid YAML frontmatter: ${detail}`);
  }
  const value = document.toJS({ maxAliasCount: 100 });
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${path}: YAML frontmatter root must be a mapping`);
  }
  return value;
}

export function stringifyFrontmatter(value) {
  return stringify(value, { lineWidth: 0 }).trimEnd();
}

export function readConcept(path) {
  const text = readFileSync(path, 'utf8');
  const { frontmatter, body } = splitFrontmatter(text, path);
  return { text, body, meta: parseFrontmatter(frontmatter, path) };
}
