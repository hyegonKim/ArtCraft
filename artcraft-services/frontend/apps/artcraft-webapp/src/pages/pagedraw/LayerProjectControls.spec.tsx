import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useSceneStore, type LayerProject } from '@storyteller/ui-pagedraw';
import { LayerProjectControls } from './LayerProjectControls';
import { createMemoryDraftStore, hashLayerDocument, type LayerDraft, type LayerDraftStore } from './layer-draft-store';
const mocks = vi.hoisted(() => ({ save: vi.fn(), list: vi.fn(), load: vi.fn(), serverHash: vi.fn(), capture: vi.fn(), restore: vi.fn(),
  loggedIn: true, user: 'u_test', drafts: undefined as unknown as LayerDraftStore }));
vi.mock('../../lib/session', () => ({ useSession: () => ({ loggedIn: mocks.loggedIn, user: mocks.loggedIn ? { user_token: mocks.user } : undefined }) }));
vi.mock('./layer-project-storage', () => ({ createLayerProjectStorage: () => mocks }));
// crypto.subtle finishes on the real event loop, outside fake timers, so tests use a microtask-only hash.
// layer-draft-store.spec.ts covers the real SHA-256.
vi.mock('./layer-draft-store', async importOriginal => ({
  ...await importOriginal<typeof import('./layer-draft-store')>(), openLayerDraftStore: () => mocks.drafts,
  hashLayerDocument: async (document: LayerProject) => `h:${JSON.stringify(document)}`, hashLayerJson: async (json: string) => `h:${json}` }));
vi.mock('@storyteller/ui-pagedraw', async () => {
  const { create } = await import('zustand');
  const state = { baseImageInfo: { isBlankCanvas: true, blankCanvasWidth: 1920, blankCanvasHeight: 1080 },
    baseImageBitmap: null, drawNodes: [], inpaintLineNodes: [], RESET: vi.fn(), setBaseImageInfo: vi.fn() };
  return { useSceneStore: create(() => state), captureLayerProject: mocks.capture, restoreLayerProject: mocks.restore };
});
const canvas = { isBlankCanvas: true, blankCanvasWidth: 1920, blankCanvasHeight: 1080, mediaToken: 'blank', url: '' };
beforeEach(() => { vi.resetAllMocks(); mocks.loggedIn = true; mocks.user = 'u_test'; mocks.drafts = createMemoryDraftStore(true); useSceneStore.setState({ baseImageInfo: canvas }); mocks.capture.mockResolvedValue({ format: 'artcraft-layer-project' }); });
afterEach(() => { cleanup(); vi.useRealTimers(); vi.restoreAllMocks(); });
const mount = (path = '/edit-image') => render(<MemoryRouter initialEntries={[path]}><LayerProjectControls /></MemoryRouter>);

it('keeps failed saves unsaved and retries, then updates rather than creating another document', async () => {
  mocks.save.mockRejectedValueOnce(new Error('Offline')).mockResolvedValue('m_saved'); mount();
  fireEvent.change(screen.getByLabelText('2D project name'), { target: { value: 'Layer UI test' } });
  fireEvent.click(screen.getByRole('button', { name: 'Save project' }));
  await screen.findByRole('alert');expect(screen.getByRole('status').textContent).toMatch(/^Save failed · retrying in 5 seconds/);
  fireEvent.click(screen.getByRole('button', { name: 'Save project' }));
  await waitFor(() => expect(screen.getByRole('status').textContent).toBe('Saved on server · autosave on'));
  fireEvent.click(screen.getByRole('button', { name: 'Save project' }));
  await waitFor(() => expect(mocks.save).toHaveBeenCalledTimes(3));
  expect(mocks.save.mock.calls[2][2]).toBe('m_saved');
});
it('restores a server project from its URL and applies its saved name', async () => {
  useSceneStore.setState({ baseImageInfo: null });
  const document = { layers: ['rectangle', 'brush'] };mocks.load.mockResolvedValue({ name: 'Restored layers', document });
  mocks.restore.mockResolvedValue(undefined);mount('/edit-image?project=m_saved');
  await waitFor(() => expect(mocks.restore).toHaveBeenCalledWith(document, expect.any(Function)));
  await waitFor(() => expect((screen.getByLabelText('2D project name') as HTMLInputElement).value).toBe('Restored layers'));
  expect(screen.getByRole('status').textContent).toBe('Saved on server · autosave on');
});
it('does not enable cloud writes for a guest', () => {
  mocks.loggedIn = false;mount();
  expect((screen.getByRole('button', { name: 'Save project' }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole('button', { name: 'Open project' }) as HTMLButtonElement).disabled).toBe(true);
});

it('keeps unsaved edits when the in-app discard dialog is cancelled', () => {
  mount();
  fireEvent.change(screen.getByLabelText('2D project name'), { target: { value: 'Keep these edits' } });
  fireEvent.click(screen.getByRole('button', { name: 'New', exact: true }));
  expect(screen.getByRole('alertdialog').textContent).toContain('Unsaved layer edits');
  fireEvent.click(screen.getByRole('button', { name: 'Keep editing' }));
  expect(screen.queryByRole('alertdialog')).toBeNull();
  expect((screen.getByLabelText('2D project name') as HTMLInputElement).value).toBe('Keep these edits');
  expect(screen.getByRole('status').textContent).toBe('Unsaved changes · autosave pending');
});

it('automatically saves a layer edit and retries a failed save without a manual click', async () => {
  vi.useFakeTimers();
  mocks.save.mockRejectedValueOnce(new Error('Offline')).mockResolvedValue('m_saved');
  mount();
  act(() => useSceneStore.setState({ drawNodes: [{ id: 'brush', type: 'line', points: [1, 2, 30, 40], stroke: '#f00', strokeWidth: 4, draggable: false }] }));
  await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
  expect(screen.getByRole('status').textContent).toBe('Save failed · retrying in 5 seconds · edits kept on this device');
  await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
  expect(mocks.save).toHaveBeenCalledTimes(2);
  expect(screen.getByRole('status').textContent).toBe('Saved on server · autosave on');
});

it('does not autosave or create a copy merely because an existing project was opened', async () => {
  vi.useFakeTimers();
  useSceneStore.setState({ baseImageInfo: null });
  mocks.load.mockResolvedValue({ document: {}, name: 'Saved document' });
  mocks.restore.mockImplementation(async () => {
    useSceneStore.setState({ baseImageInfo: { isBlankCanvas: true, blankCanvasWidth: 1920, blankCanvasHeight: 1080, mediaToken: 'blank', url: '' } });
  });
  mount('/edit-image?project=m_saved');
  await act(async () => { await vi.advanceTimersByTimeAsync(10000); });
  expect(screen.getByRole('status').textContent).toBe('Saved on server · autosave on');
  expect(mocks.save).not.toHaveBeenCalled();
});

// Recovered edits sync through the normal 1 s autosave delay once the restore finishes.
const slow = { timeout: 3000 };
const keptDocument = { format: 'artcraft-layer-project', version: 1, drawNodes: [{ id: 'offline-brush' }] } as unknown as LayerProject;
const kept = (patch: Partial<LayerDraft>): LayerDraft => ({ id: 'd_kept', owner: 'u_test', copyPending: false, name: 'Kept offline',
  document: keptDocument, ownHashes: [], updatedAt: Date.UTC(2026, 10, 8, 1, 2), ...patch });
const restoreScene = () => mocks.restore.mockImplementation(async () => { useSceneStore.setState({ baseImageInfo: canvas }); });

it('resumes a never-saved document kept on this device after the browser closed and creates it on the server', async () => {
  useSceneStore.setState({ baseImageInfo: null }); restoreScene();
  await mocks.drafts.put(kept({})); mocks.capture.mockResolvedValue(keptDocument); mocks.save.mockResolvedValue('m_created');
  mount();
  await waitFor(() => expect(mocks.restore).toHaveBeenCalledWith(keptDocument, expect.any(Function)));
  await waitFor(() => expect(screen.getByRole('status').textContent).toBe('Saved on server · autosave on'), slow);
  expect(mocks.save.mock.calls[0].slice(0, 3)).toEqual([keptDocument, 'Kept offline', undefined]);
  expect((screen.getByLabelText('2D project name') as HTMLInputElement).value).toBe('Kept offline');
  await waitFor(async () => expect(await mocks.drafts.list('u_test')).toEqual([]), slow);
});

it('prefers the device copy of the project in the URL and updates that project after checking the server copy', async () => {
  useSceneStore.setState({ baseImageInfo: null }); restoreScene();
  const baseHash = await hashLayerDocument({ format: 'artcraft-layer-project', version: 1 } as unknown as LayerProject);
  await mocks.drafts.put(kept({ token: 'm_saved', baseHash }));
  mocks.serverHash.mockResolvedValue(baseHash); mocks.capture.mockResolvedValue(keptDocument); mocks.save.mockResolvedValue('m_saved');
  mount('/edit-image?project=m_saved');
  await waitFor(() => expect(mocks.save).toHaveBeenCalledTimes(1), slow);
  expect(mocks.load).not.toHaveBeenCalled();
  expect(mocks.serverHash).toHaveBeenCalledWith('m_saved');
  expect(mocks.save.mock.calls[0].slice(0, 3)).toEqual([keptDocument, 'Kept offline', 'm_saved']);
  await waitFor(() => expect(screen.getByRole('status').textContent).toBe('Saved on server · autosave on'), slow);
});

it('lists unsynced edits of another project and deletes them only after a second confirmation', async () => {
  await mocks.drafts.put(kept({ token: 'm_other', name: 'Other project' }));
  mount();
  const banner = await screen.findByLabelText('Edits kept on this device');
  expect(banner.textContent).toContain('Other project');
  fireEvent.click(screen.getByRole('button', { name: 'Discard edits' }));
  expect(await mocks.drafts.list('u_test')).toHaveLength(1);
  fireEvent.click(screen.getByRole('button', { name: 'Delete permanently' }));
  await waitFor(() => expect(screen.queryByLabelText('Edits kept on this device')).toBeNull());
  expect(await mocks.drafts.list('u_test')).toEqual([]);
});

it('restores another project\'s unsynced edits from the banner without touching the server copy first', async () => {
  restoreScene(); mocks.save.mockResolvedValue('m_other');
  await mocks.drafts.put(kept({ token: 'm_other', name: 'Other project' }));
  mount();
  fireEvent.click(await screen.findByRole('button', { name: 'Restore edits' }));
  await waitFor(() => expect(mocks.restore).toHaveBeenCalledWith(keptDocument, expect.any(Function)));
  await waitFor(() => expect(mocks.save).toHaveBeenCalledTimes(1), slow);
  expect(mocks.save.mock.calls[0][2]).toBe('m_other');
  expect(screen.getByText(/Restored edits kept on this device/)).toBeTruthy();
});

it('keeps edits locally while offline, does not block closing the tab, and syncs when the network returns', async () => {
  vi.useFakeTimers(); mocks.save.mockResolvedValue('m_saved');
  mount();
  act(() => { window.dispatchEvent(new Event('offline')); });
  act(() => useSceneStore.setState({ drawNodes: [{ id: 'offline-brush', type: 'line', points: [1, 2], stroke: '#000', strokeWidth: 3, draggable: false }] }));
  const unload = () => { const event = new Event('beforeunload', { cancelable: true }); window.dispatchEvent(event); return event.defaultPrevented; };
  expect(unload()).toBe(true);
  await act(async () => { await vi.advanceTimersByTimeAsync(10_000); });
  expect(screen.getByRole('status').textContent).toBe('Offline · edits kept on this device · syncs when back online');
  expect(mocks.save).not.toHaveBeenCalled();
  expect(await mocks.drafts.list('u_test')).toHaveLength(1);
  expect(unload()).toBe(false);
  await act(async () => { window.dispatchEvent(new Event('online')); await vi.advanceTimersByTimeAsync(0); });
  expect(mocks.save).toHaveBeenCalledTimes(1);
  expect(screen.getByRole('status').textContent).toBe('Saved on server · autosave on');
  expect(await mocks.drafts.list('u_test')).toEqual([]);
});

it('never shows or writes device copies for a guest', async () => {
  mocks.loggedIn = false; await mocks.drafts.put(kept({ token: 'm_other' }));
  mount();
  act(() => useSceneStore.setState({ drawNodes: [{ id: 'guest', type: 'line', points: [1, 2], stroke: '#000', strokeWidth: 3, draggable: false }] }));
  await new Promise(resolve => setTimeout(resolve, 400));
  expect(screen.queryByLabelText('Edits kept on this device')).toBeNull();
  expect((await mocks.drafts.list('u_test')).map(row => row.id)).toEqual(['d_kept']);
});

const offlineStroke = () => act(() => useSceneStore.setState({ drawNodes: [{ id: 'offline-brush', type: 'line', points: [1, 2], stroke: '#000', strokeWidth: 3, draggable: false }] }));
const keptOffline = 'Offline · edits kept on this device · syncs when back online';

it('keeps the device copy when Discard is chosen but the other project cannot be opened', async () => {
  mocks.list.mockResolvedValue({ rows: [{ token: 'm_other', maybe_title: 'Other project', updated_at: '2026-11-08T00:00:00Z' }] });
  mocks.load.mockRejectedValue(new Error('Failed to fetch'));
  mount();
  act(() => { window.dispatchEvent(new Event('offline')); });
  offlineStroke();
  await waitFor(() => expect(screen.getByRole('status').textContent).toBe(keptOffline));
  fireEvent.click(screen.getByRole('button', { name: 'Open project' }));
  fireEvent.click(await screen.findByRole('button', { name: /^Other project/ }));
  fireEvent.click(screen.getByRole('button', { name: 'Discard changes' }));
  await waitFor(() => expect(mocks.load).toHaveBeenCalledWith('m_other'));
  await waitFor(() => expect(screen.getByRole('status').textContent).toBe(keptOffline));
  expect(await mocks.drafts.list('u_test')).toHaveLength(1);
});

it('clears the previous account\'s document when another account signs in and never keeps it for that account', async () => {
  const view = mount();
  act(() => { window.dispatchEvent(new Event('offline')); });
  fireEvent.change(screen.getByLabelText('2D project name'), { target: { value: 'Private to u_test' } });
  await waitFor(async () => expect(await mocks.drafts.list('u_test')).toHaveLength(1));
  mocks.user = 'u_other';
  view.rerender(<MemoryRouter initialEntries={['/edit-image']}><LayerProjectControls /></MemoryRouter>);
  await waitFor(() => expect(useSceneStore.getState().RESET).toHaveBeenCalled());
  expect((screen.getByLabelText('2D project name') as HTMLInputElement).value).toBe('Untitled image project');
  await new Promise(resolve => setTimeout(resolve, 400));
  expect(await mocks.drafts.list('u_other')).toEqual([]);
  expect((await mocks.drafts.list('u_test')).map(row => row.name)).toEqual(['Private to u_test']);
});

it('does not delete device edits that another tab opened after the list was shown', async () => {
  await mocks.drafts.put(kept({ token: 'm_other', name: 'Other project' }));
  mount();
  fireEvent.click(await screen.findByRole('button', { name: 'Discard edits' }));
  const release = await mocks.drafts.claim('d_kept'); // Another tab restores them now.
  fireEvent.click(screen.getByRole('button', { name: 'Delete permanently' }));
  expect((await screen.findByRole('alert')).textContent).toContain('These edits are open in another tab.');
  expect(await mocks.drafts.list('u_test')).toHaveLength(1);
  release?.();
});

it('does not claim offline edits are kept when this browser blocks device storage', async () => {
  mocks.drafts = createMemoryDraftStore(false);
  mount();
  act(() => { window.dispatchEvent(new Event('offline')); });
  const notKept = 'Offline · edits are not kept on this device · keep this page open';
  await waitFor(() => expect(screen.getByRole('status').textContent).toBe(notKept));
  offlineStroke();
  await new Promise(resolve => setTimeout(resolve, 400));
  expect(screen.getByRole('status').textContent).toBe(notKept);
});

it('reads only draft details, never documents, to show the banner when the tab regains focus', async () => {
  await mocks.drafts.put(kept({ token: 'm_other', name: 'Other project' }));
  const get = vi.spyOn(mocks.drafts, 'get');
  const list = vi.spyOn(mocks.drafts, 'list');
  mount();
  await screen.findByLabelText('Edits kept on this device');
  const before = list.mock.calls.length;
  act(() => { document.dispatchEvent(new Event('visibilitychange')); });
  await waitFor(() => expect(list.mock.calls.length).toBeGreaterThan(before));
  expect(get).not.toHaveBeenCalled();
  const [row] = await list.mock.results.at(-1)!.value;
  expect(row).toMatchObject({ id: 'd_kept', name: 'Other project' });
  expect(row).not.toHaveProperty('document');
});

