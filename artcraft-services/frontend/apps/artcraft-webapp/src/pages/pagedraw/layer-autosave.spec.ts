import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { LayerProject } from '@storyteller/ui-pagedraw';
import { LayerAutosave, LAYER_AUTOSAVE_DELAY_MS, LAYER_SAVE_RETRY_MS } from './layer-autosave';

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
