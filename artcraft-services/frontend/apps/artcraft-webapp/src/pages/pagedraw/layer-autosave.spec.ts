import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { LayerProject } from '@storyteller/ui-pagedraw';
import { LayerAutosave, LayerProjectUnavailableError, LAYER_AUTOSAVE_DELAY_MS, LAYER_LOCAL_DELAY_MS, LAYER_SAVE_RETRY_MS,
  LOCAL_RECOVERY_UNAVAILABLE } from './layer-autosave';
import { createMemoryDraftStore, type LayerDraft, type LayerDraftStore } from './layer-draft-store';

const document = { version: 1, drawNodes: [{ id: 'original' }] } as unknown as LayerProject;
let saver: LayerAutosave;
let name: string;
let capture: ReturnType<typeof vi.fn>;
let save: ReturnType<typeof vi.fn>;
let onSaved: ReturnType<typeof vi.fn>;
beforeEach(() => {
  vi.useFakeTimers(); name = 'Original'; capture = vi.fn().mockResolvedValue(document);
  save = vi.fn().mockResolvedValue('m_saved'); onSaved = vi.fn();
  saver = new LayerAutosave({ capture, save, getName: () => name, onSaved, onState: vi.fn() });
  saver.setEnabled(true);
});
afterEach(() => { saver.dispose(); vi.useRealTimers(); });

it('debounces edits and updates the same server identity after its first automatic save', async () => {
  saver.markDirty(); await vi.advanceTimersByTimeAsync(600); name = 'Latest'; saver.markDirty();
  await vi.advanceTimersByTimeAsync(999); expect(save).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(1); expect(save).toHaveBeenCalledTimes(1);
  expect(save.mock.calls[0].slice(0, 3)).toEqual([document, 'Latest', undefined]);
  expect(saver.state).toMatchObject({ dirty: false, saving: false });
  saver.markDirty(); await vi.advanceTimersByTimeAsync(LAYER_AUTOSAVE_DELAY_MS);
  expect(save.mock.calls[1][2]).toBe('m_saved');
});

it('never overlaps writes or marks edits made during an in-flight save clean', async () => {
  let finish!: (token: string) => void;
  save.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
  saver.markDirty(); await vi.advanceTimersByTimeAsync(1000);
  name = 'Edited while saving'; saver.markDirty();
  await vi.advanceTimersByTimeAsync(5000); expect(save).toHaveBeenCalledTimes(1);
  finish('m_saved'); await vi.advanceTimersByTimeAsync(0);
  expect(saver.state.dirty).toBe(true);
  await vi.advanceTimersByTimeAsync(1000);
  expect(save.mock.calls[1].slice(1, 3)).toEqual(['Edited while saving', 'm_saved']);
  expect(saver.state.dirty).toBe(false);
});

it('retries the latest edits with a fresh API request ID and retains the dirty state until acknowledgment', async () => {
  const newer = { ...document, drawNodes: [] };
  save.mockRejectedValueOnce(new Error('Offline'));
  saver.markDirty(); await vi.advanceTimersByTimeAsync(1000);
  expect(saver.state).toMatchObject({ dirty: true, retrying: true, error: 'Offline' });
  const attempt = save.mock.calls[0];
  capture.mockResolvedValue(newer); name = 'Newer'; saver.markDirty();
  await vi.advanceTimersByTimeAsync(4999); expect(save).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(1);
  expect(save.mock.calls[1].slice(0, 3)).toEqual([newer, 'Newer', undefined]);
  expect(save.mock.calls[1][3]).not.toBe(attempt[3]);
  expect(capture).toHaveBeenCalledTimes(2);
  expect(saver.state).toMatchObject({ dirty: false, error: '' });
});

it('does not save while disabled and resumes a dirty document when enabled', async () => {
  saver.setEnabled(false); saver.markDirty();
  await vi.advanceTimersByTimeAsync(10000); expect(save).not.toHaveBeenCalled();
  saver.setEnabled(true); await vi.advanceTimersByTimeAsync(1000);
  expect(save).toHaveBeenCalledTimes(1);
});

it('recovers from an image capture failure without pretending it saved', async () => {
  capture.mockRejectedValueOnce(new Error('Image unavailable'));
  saver.markDirty(); await vi.advanceTimersByTimeAsync(1000);
  expect(save).not.toHaveBeenCalled(); expect(saver.state.dirty).toBe(true);
  await vi.advanceTimersByTimeAsync(LAYER_SAVE_RETRY_MS);
  expect(save).toHaveBeenCalledTimes(1); expect(saver.state.dirty).toBe(false);
});

it('cancels timers and ignores a response for a project that has been replaced', async () => {
  let finish!: (token: string) => void;
  save.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
  saver.markDirty(); await vi.advanceTimersByTimeAsync(1000);
  saver.reset('m_other'); finish('m_stale'); await vi.advanceTimersByTimeAsync(10000);
  expect(onSaved).not.toHaveBeenCalled(); expect(saver.state.dirty).toBe(false);
  saver.markDirty(); await saver.flush();
  expect(save.mock.calls[1][2]).toBe('m_other');
});

it('stops retrying after disposal and can activate again for React StrictMode effect replay', async () => {
  save.mockRejectedValueOnce(new Error('Offline'));
  saver.markDirty(); await vi.advanceTimersByTimeAsync(1000); saver.dispose();
  await vi.advanceTimersByTimeAsync(10000); expect(save).toHaveBeenCalledTimes(1);
  saver.activate(); saver.setEnabled(true); await vi.advanceTimersByTimeAsync(5000);
  expect(save).toHaveBeenCalledTimes(2); expect(saver.state.dirty).toBe(false);
});

it('keeps a failed Save copy separate from the original through automatic retry', async () => {
  saver.reset('m_original'); save.mockRejectedValueOnce(new Error('Offline'));
  await saver.flush(true); const attempt = save.mock.calls[0];
  expect(attempt[2]).toBeUndefined();
  await vi.advanceTimersByTimeAsync(5000); expect(save.mock.calls[1].slice(0, 3)).toEqual(attempt.slice(0, 3));
  expect(save.mock.calls[1][3]).not.toBe(attempt[3]);
  saver.markDirty(); await saver.flush(); expect(save.mock.calls[2][2]).toBe('m_saved');
});

// ---- Device copies (IndexedDB in the browser) and the offline save queue ----
const hash = async (value: LayerProject) => `h:${JSON.stringify(value)}`;
let drafts: LayerDraftStore;
function local(extra: Partial<ConstructorParameters<typeof LayerAutosave>[0]> = {}, owner = 'u_owner') {
  saver.dispose();
  drafts = (extra.drafts as LayerDraftStore | undefined) ?? createMemoryDraftStore(true);
  saver = new LayerAutosave({ capture, save, getName: () => name, onSaved, onState: vi.fn(), drafts, hash, ...extra });
  saver.activate(); saver.setOwner(owner); saver.setEnabled(true);
  return saver;
}
const kept = (id = 'd_recovered', patch: Partial<LayerDraft> = {}): LayerDraft => ({ id, owner: 'u_owner', token: 'm_saved',
  copyPending: false, name: 'Offline edits', document, baseHash: 'h:base', ownHashes: [], updatedAt: 1, ...patch });

it('keeps an edit on this device before the server confirms it and deletes that copy only after acknowledgment', async () => {
  local();
  let finish!: (token: string) => void;
  save.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
  saver.markDirty(); await vi.advanceTimersByTimeAsync(LAYER_LOCAL_DELAY_MS);
  expect(await drafts.list('u_owner')).toMatchObject([{ owner: 'u_owner', name: 'Original', token: undefined, document }]);
  expect(await drafts.list('u_other')).toEqual([]);
  expect(saver.state).toMatchObject({ dirty: true, local: true });
  await vi.advanceTimersByTimeAsync(LAYER_AUTOSAVE_DELAY_MS); expect(save).toHaveBeenCalledTimes(1);
  expect(await drafts.list('u_owner')).toHaveLength(1);
  finish('m_saved'); await vi.advanceTimersByTimeAsync(0);
  expect(await drafts.list('u_owner')).toEqual([]);
  expect(saver.state).toMatchObject({ dirty: false, local: false });
});

it('queues saves while offline and sends the latest edits as soon as the browser is back online', async () => {
  local({ online: () => false });
  saver.markDirty(); name = 'Edited offline'; saver.markDirty();
  await vi.advanceTimersByTimeAsync(60_000);
  expect(save).not.toHaveBeenCalled();
  expect(saver.state).toMatchObject({ dirty: true, offline: true, local: true, retrying: false, error: '' });
  expect((await drafts.list('u_owner'))[0].name).toBe('Edited offline');
  saver.setOnline(true); await vi.advanceTimersByTimeAsync(0);
  expect(save).toHaveBeenCalledTimes(1);
  expect(save.mock.calls[0].slice(0, 3)).toEqual([document, 'Edited offline', undefined]);
  expect(await drafts.list('u_owner')).toEqual([]);
});

it('records the new server identity on a kept copy at once, so a later replay updates instead of duplicating', async () => {
  local();
  let finish!: (token: string) => void;
  save.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
  saver.markDirty(); await vi.advanceTimersByTimeAsync(LAYER_AUTOSAVE_DELAY_MS);
  name = 'Edited during the first create'; saver.markDirty(); await vi.advanceTimersByTimeAsync(LAYER_LOCAL_DELAY_MS);
  finish('m_created'); await vi.advanceTimersByTimeAsync(0);
  const [row] = await drafts.list('u_owner');
  expect(row).toMatchObject({ token: 'm_created', baseHash: await hash(document), name: 'Edited during the first create' });
});

it('replays a recovered copy onto the same project when the server still has the version it was based on', async () => {
  const serverHash = vi.fn().mockResolvedValue('h:base');
  local({ serverHash }); await drafts.put(kept()); name = 'Offline edits';
  saver.adopt(kept()); await vi.advanceTimersByTimeAsync(0);
  expect(serverHash).toHaveBeenCalledWith('m_saved');
  expect(save.mock.calls[0].slice(0, 3)).toEqual([document, 'Offline edits', 'm_saved']);
  expect(onSaved).toHaveBeenCalledWith('m_saved', undefined);
  expect(await drafts.list('u_owner')).toEqual([]);
});

it.each([
  ['changed on another device', () => Promise.resolve('h:someone-else')],
  ['deleted', () => Promise.reject(new LayerProjectUnavailableError('This 2D project is unavailable.'))],
])('saves a recovered copy as a separate private project when the original was %s', async (_case, serverState) => {
  save.mockResolvedValue('m_copy');
  local({ serverHash: vi.fn(serverState) }); await drafts.put(kept()); name = 'Offline edits';
  saver.adopt(kept()); await vi.advanceTimersByTimeAsync(0);
  expect(save).toHaveBeenCalledTimes(1);
  expect(save.mock.calls[0].slice(0, 3)).toEqual([document, 'Offline edits (offline copy)', undefined]);
  expect(onSaved).toHaveBeenCalledWith('m_copy', 'Offline edits (offline copy)');
  expect(saver.state.notice).toContain('was not overwritten');
  expect(await drafts.list('u_owner')).toEqual([]);
  saver.markDirty(); await saver.flush();
  expect(save.mock.calls[1][2]).toBe('m_copy');
});

it('treats its own landed write with a lost response as safe, without creating a copy', async () => {
  const newer = { ...document, drawNodes: [] } as unknown as LayerProject;
  const serverHash = vi.fn().mockResolvedValue('h:base');
  save.mockRejectedValueOnce(new TypeError('Failed to fetch'));
  local({ serverHash }); await drafts.put(kept()); name = 'Offline edits';
  saver.adopt(kept()); await vi.advanceTimersByTimeAsync(0);
  expect(saver.state).toMatchObject({ dirty: true, retrying: true });
  expect((await drafts.get('d_recovered'))!.ownHashes).toEqual([await hash(document)]);
  // The first request did reach the server; the device only saw the network failure.
  serverHash.mockResolvedValue(await hash(document)); capture.mockResolvedValue(newer); saver.markDirty();
  saver.setOnline(false); saver.setOnline(true); await vi.advanceTimersByTimeAsync(0);
  expect(serverHash).toHaveBeenCalledTimes(2);
  expect(save.mock.calls[1].slice(0, 3)).toEqual([newer, 'Offline edits', 'm_saved']);
});

it('keeps waiting instead of copying when the server cannot be reached for the pre-save check', async () => {
  const serverHash = vi.fn().mockRejectedValueOnce(new TypeError('Failed to fetch')).mockResolvedValue('h:base');
  local({ serverHash }); await drafts.put(kept()); name = 'Offline edits';
  saver.adopt(kept()); await vi.advanceTimersByTimeAsync(0);
  expect(save).not.toHaveBeenCalled();
  expect(saver.state).toMatchObject({ dirty: true, retrying: true, error: 'Failed to fetch', local: true });
  await vi.advanceTimersByTimeAsync(LAYER_SAVE_RETRY_MS);
  expect(save.mock.calls[0].slice(0, 3)).toEqual([document, 'Offline edits', 'm_saved']);
});

it('keeps a Save copy made offline as a pending separate project after reload', async () => {
  local({ online: () => false }); saver.reset('m_original');
  await saver.flush(true); await vi.advanceTimersByTimeAsync(0);
  expect(save).not.toHaveBeenCalled();
  expect((await drafts.list('u_owner'))[0]).toMatchObject({ token: 'm_original', copyPending: true });
  saver.setOnline(true); await vi.advanceTimersByTimeAsync(0);
  expect(save.mock.calls[0][2]).toBeUndefined();
});

it('writes a pending edit to the device when the editor unmounts before the local delay', async () => {
  local(); saver.markDirty(); saver.dispose();
  await vi.advanceTimersByTimeAsync(0);
  expect(await drafts.list('u_owner')).toMatchObject([{ document }]);
  expect(save).not.toHaveBeenCalled();
});

it('reports when the device cannot keep a copy, while server saves continue', async () => {
  const full = createMemoryDraftStore(true);
  full.put = () => Promise.reject(new DOMException('full', 'QuotaExceededError'));
  local({ drafts: full }); saver.markDirty(); await vi.advanceTimersByTimeAsync(LAYER_LOCAL_DELAY_MS);
  expect(saver.state).toMatchObject({ local: false, localError: expect.stringContaining('out of storage') });
  await vi.advanceTimersByTimeAsync(LAYER_AUTOSAVE_DELAY_MS); expect(save).toHaveBeenCalledTimes(1);
  local({ drafts: createMemoryDraftStore(false) }); saver.markDirty(); await vi.advanceTimersByTimeAsync(LAYER_LOCAL_DELAY_MS);
  expect(saver.state).toMatchObject({ local: false, localError: LOCAL_RECOVERY_UNAVAILABLE });
});

it('never writes a device copy without a signed-in owner', async () => {
  local({}, ''); saver.setOwner(undefined); saver.markDirty();
  await vi.advanceTimersByTimeAsync(LAYER_LOCAL_DELAY_MS);
  expect(await drafts.list('')).toEqual([]);
});

it('keeps the last edit when the session ends and never writes it into another account\'s record', async () => {
  local(); saver.markDirty(); saver.setOwner(undefined); await vi.advanceTimersByTimeAsync(0);
  const [first] = await drafts.list('u_owner');
  expect(first.document).toEqual(document);
  saver.setOwner('u_owner'); name = 'After signing in again'; saver.markDirty(); await vi.advanceTimersByTimeAsync(LAYER_LOCAL_DELAY_MS);
  expect((await drafts.list('u_owner')).map(row => [row.id, row.name])).toEqual([[first.id, 'After signing in again']]);
  saver.setOwner('u_other'); await vi.advanceTimersByTimeAsync(LAYER_AUTOSAVE_DELAY_MS);
  // The previous account's document is neither kept for nor saved to the new account.
  expect(saver.state.dirty).toBe(false);
  expect(await drafts.list('u_other')).toEqual([]);
  expect(save).not.toHaveBeenCalled();
  expect((await drafts.list('u_owner')).map(row => row.name)).toEqual(['After signing in again']);
  saver.markDirty(); await vi.advanceTimersByTimeAsync(LAYER_LOCAL_DELAY_MS);
  expect((await drafts.list('u_other'))[0].id).not.toBe(first.id);
});

it('keeps saving a conflicting replay as a separate copy when the first copy upload fails', async () => {
  save.mockRejectedValueOnce(new TypeError('Failed to fetch')).mockResolvedValue('m_copy');
  local({ serverHash: vi.fn().mockResolvedValue('h:someone-else') }); await drafts.put(kept()); name = 'Offline edits';
  saver.adopt(kept()); await vi.advanceTimersByTimeAsync(0);
  expect(save.mock.calls[0].slice(0, 3)).toEqual([document, 'Offline edits (offline copy)', undefined]);
  // The retry falls inside the server-check reuse window, yet must never overwrite the original.
  await vi.advanceTimersByTimeAsync(LAYER_SAVE_RETRY_MS);
  expect(save).toHaveBeenCalledTimes(2);
  expect(save.mock.calls[1].slice(0, 3)).toEqual([document, 'Offline edits (offline copy)', undefined]);
  expect(onSaved).toHaveBeenCalledWith('m_copy', 'Offline edits (offline copy)');
  expect(saver.state.notice).toContain('was not overwritten');
});

it('stops reporting edits as kept once their device copy is discarded, even if a write was already queued', async () => {
  local({ online: () => false });
  saver.markDirty(); await vi.advanceTimersByTimeAsync(LAYER_LOCAL_DELAY_MS);
  expect(saver.state.local).toBe(true);
  await saver.discardLocal();
  expect(saver.state).toMatchObject({ dirty: true, local: false });
  expect(await drafts.list('u_owner')).toEqual([]);
  await saver.persistNow();
  expect(saver.state.local).toBe(true);
  saver.markDirty(); const writing = saver.persistNow(); const discarding = saver.discardLocal();
  await writing; await discarding;
  expect(saver.state.local).toBe(false);
  expect(await drafts.list('u_owner')).toEqual([]);
});

it('reports an edit that cannot be captured for the device copy instead of implying it is being kept', async () => {
  local({ online: () => false });
  capture.mockRejectedValueOnce(new Error('Could not read an image layer. Please retry.'));
  saver.markDirty(); await vi.advanceTimersByTimeAsync(LAYER_LOCAL_DELAY_MS);
  expect(saver.state).toMatchObject({ local: false,
    localError: 'Could not keep a recovery copy on this device. Could not read an image layer. Please retry.' });
  saver.markDirty(); await vi.advanceTimersByTimeAsync(LAYER_LOCAL_DELAY_MS);
  expect(saver.state).toMatchObject({ local: true, localError: '' });
  // Without an open document (saving disabled) there is nothing to recover, so nothing to report.
  saver.setEnabled(false); capture.mockRejectedValueOnce(new Error('Open an image or create a canvas before saving.'));
  saver.markDirty(); await vi.advanceTimersByTimeAsync(LAYER_LOCAL_DELAY_MS);
  expect(saver.state.localError).toBe('');
});
