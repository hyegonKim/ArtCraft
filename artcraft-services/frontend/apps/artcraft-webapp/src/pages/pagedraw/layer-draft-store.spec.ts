import { expect, it } from 'vitest';
import type { LayerProject } from '@storyteller/ui-pagedraw';
import { createMemoryDraftStore, hashLayerDocument, openLayerDraftStore, type LayerDraft } from './layer-draft-store';

const document = { format: 'artcraft-layer-project', version: 1, drawNodes: [{ id: 'a', type: 'line', points: [1, 2.5, -0] }] } as unknown as LayerProject;
const draft = (id: string, owner: string, updatedAt: number): LayerDraft =>
  ({ id, owner, copyPending: false, name: id, document, ownHashes: [], updatedAt });

it('lists only the owner\'s drafts, newest first, and returns copies that cannot mutate the store', async () => {
  const store = createMemoryDraftStore(true);
  await store.put(draft('old', 'u_a', 1)); await store.put(draft('new', 'u_a', 2)); await store.put(draft('other', 'u_b', 3));
  expect((await store.list('u_a')).map(row => row.id)).toEqual(['new', 'old']);
  (await store.get('new'))!.name = 'changed';
  expect((await store.get('new'))!.name).toBe('new');
  await store.remove('new');
  expect((await store.list('u_a')).map(row => row.id)).toEqual(['old']);
});

it('lets only one holder claim a draft until it is released', async () => {
  const store = createMemoryDraftStore(true);
  const release = await store.claim('d1');
  expect(release).toBeTypeOf('function');
  expect(await store.claim('d1')).toBeUndefined();
  expect(await store.held()).toEqual(new Set(['d1']));
  release!();
  expect(await store.claim('d1')).toBeTypeOf('function');
});

it('falls back to page memory and reports it as not durable when IndexedDB is unavailable', async () => {
  const store = openLayerDraftStore(undefined);
  expect(await store.durable()).toBe(false);
  await store.put(draft('d1', 'u_a', 1));
  expect((await store.list('u_a')).map(row => row.id)).toEqual(['d1']);
});

it('hashes a document identically after a JSON round trip', async () => {
  const roundTrip = JSON.parse(JSON.stringify(document)) as LayerProject;
  expect(await hashLayerDocument(roundTrip)).toBe(await hashLayerDocument(document));
  expect(await hashLayerDocument({ ...document, version: 2 } as unknown as LayerProject)).not.toBe(await hashLayerDocument(document));
});

it('lists details without documents and updates details without touching the document', async () => {
  const store = createMemoryDraftStore(true);
  await store.put(draft('d1', 'u_a', 1));
  const [row] = await store.list('u_a');
  expect(row).not.toHaveProperty('document');
  await store.update('d1', { token: 'm_saved', name: 'renamed', ownHashes: ['h:sent'] });
  expect(await store.get('d1')).toMatchObject({ token: 'm_saved', name: 'renamed', ownHashes: ['h:sent'], owner: 'u_a', document });
  await store.update('missing', { name: 'gone' });
  expect(await store.get('missing')).toBeUndefined();
});

it('shares one store (and one database connection) per IndexedDB factory, and one page fallback', async () => {
  const blocked = { open: () => { throw new DOMException('blocked', 'SecurityError'); } } as unknown as IDBFactory;
  expect(openLayerDraftStore(blocked)).toBe(openLayerDraftStore(blocked));
  expect(await openLayerDraftStore(blocked).durable()).toBe(false);
  expect(openLayerDraftStore(undefined)).toBe(openLayerDraftStore(undefined));
});
