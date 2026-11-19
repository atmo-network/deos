/*
Domain: Static-site fragment state grammar
Owns: `#!` state parsing/serialization, default omission, unknown-key preservation and query isolation.
Excludes: History actions, layer membership, browser presentation and search ranking.
*/
import assert from 'node:assert/strict';
import test from 'node:test';
import { readHashState, writeHashState } from '../src/hash-state.mjs';

const page = 'https://wiki.invalid/deos/en/deos/overview/actor.html';

test('the reader state lives in the fragment and never in the server-facing query', () => {
  assert.deepEqual(readHashState(page), { stateful: false, layer: 'all', search: null, highlight: null, at: null });
  assert.deepEqual(readHashState(`${page}?layer=deos&search=observations`), { stateful: false, layer: 'all', search: null, highlight: null, at: null }, 'a query string must not carry reader state');
  const state = readHashState(`${page}#!layer=deos&search=%D0%B0%D0%BA%D1%82%D0%BE%D1%80&highlight=%25%2B&at=system-contract`);
  assert.equal(state.stateful, true);
  assert.equal(state.layer, 'deos');
  assert.equal(state.search, 'актор');
  assert.equal(state.highlight, '%+');
  assert.equal(state.at, 'system-contract');
  assert.equal(writeHashState(`${page}?keep=1`, { layer: 'deos' }), `${page}?keep=1#!layer=deos`);
});

test('a plain fragment stays a native anchor and a bare page keeps no fragment', () => {
  assert.deepEqual(readHashState(`${page}#summary`), { stateful: false, layer: 'all', search: null, highlight: null, at: 'summary' });
  assert.equal(readHashState(`${page}#%D0%B0%D0%BA%D1%82`).at, 'акт');
  assert.equal(writeHashState(page, {}), page);
  assert.equal(writeHashState(`${page}#summary`, {}), `${page}#summary`);
  assert.equal(writeHashState(`${page}#!layer=deos`, { layer: 'all' }), page, 'default state must leave no fragment');
  assert.equal(writeHashState(`${page}#!layer=deos&at=summary`, { layer: 'all' }), `${page}#summary`, 'section-only state stays a native anchor');
});

test('the open search overlay is fragment state, including an empty query', () => {
  assert.equal(writeHashState(`${page}#summary`, { search: '' }), `${page}#!search=&at=summary`);
  assert.equal(readHashState(`${page}#!search=`).search, '');
  assert.equal(readHashState(`${page}#!search=`).stateful, true);
  assert.equal(writeHashState(`${page}#!search=&at=summary`, { search: null, highlight: null }), `${page}#summary`);
  const typed = writeHashState(`${page}#!search=&layer=deos`, { search: '% %25 + &=# ?/«Ё»' });
  assert.equal(readHashState(typed).search, '% %25 + &=# ?/«Ё»');
  assert.equal(readHashState(typed).layer, 'deos', 'typing must not drop independent state');
});

test('updates rewrite only this codec’s keys and keep every other fragment key', () => {
  const source = `${page}#!tag=a%3Db&tag=x%26y&layer=deos&unknown=1`;
  const next = writeHashState(source, { layer: 'example', at: 'summary' });
  const state = readHashState(next);
  assert.equal(state.layer, 'example');
  assert.equal(state.at, 'summary');
  assert.deepEqual(new URLSearchParams(new URL(next).hash.slice(2)).getAll('tag'), ['a=b', 'x&y']);
  assert.equal(new URLSearchParams(new URL(next).hash.slice(2)).get('unknown'), '1');
  assert.equal(new URL(next).search, '', 'the codec must never write a query string');
});

test('repeated or empty layer values stay representable for exact admission', () => {
  assert.equal(readHashState(`${page}#!layer=deos&layer=example`).layer, null);
  assert.equal(readHashState(`${page}#!layer=`).layer, '');
  assert.equal(readHashState(`${page}#!layer=deos`).layer, 'deos');
});
