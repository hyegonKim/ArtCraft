import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useSceneStore } from '@storyteller/ui-pagedraw';
import { LayerProjectControls } from './LayerProjectControls';
const mocks = vi.hoisted(() => ({ save: vi.fn(), list: vi.fn(), load: vi.fn(), capture: vi.fn(), restore: vi.fn(), loggedIn: true }));
vi.mock('../../lib/session', () => ({ useSession: () => ({ loggedIn: mocks.loggedIn }) }));
vi.mock('./layer-project-storage', () => ({ createLayerProjectStorage: () => mocks }));
vi.mock('@storyteller/ui-pagedraw', async () => {
  const { create } = await import('zustand');
  const state = { baseImageInfo: { isBlankCanvas: true, blankCanvasWidth: 1920, blankCanvasHeight: 1080 },
    baseImageBitmap: null, drawNodes: [], inpaintLineNodes: [], RESET: vi.fn() };
  return { useSceneStore: create(() => state), captureLayerProject: mocks.capture, restoreLayerProject: mocks.restore };
});
beforeEach(() => { vi.resetAllMocks(); mocks.loggedIn = true; useSceneStore.setState({ baseImageInfo: { isBlankCanvas: true, blankCanvasWidth: 1920, blankCanvasHeight: 1080, mediaToken: 'blank', url: '' } }); mocks.capture.mockResolvedValue({ format: 'artcraft-layer-project' }); });
afterEach(() => { cleanup(); vi.useRealTimers(); vi.restoreAllMocks(); });
const mount = (path = '/edit-image') => render(<MemoryRouter initialEntries={[path]}><LayerProjectControls /></MemoryRouter>);

it('keeps failed saves unsaved and retries, then updates rather than creating another document', async () => {
  mocks.save.mockRejectedValueOnce(new Error('Offline')).mockResolvedValue('m_saved'); mount();
  fireEvent.change(screen.getByLabelText('2D project name'), { target: { value: 'Layer UI test' } });
  fireEvent.click(screen.getByRole('button', { name: 'Save project' }));
  await screen.findByRole('alert');expect(screen.getByRole('status').textContent).toBe('Save failed · retrying in 5 seconds');
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
  expect(screen.getByRole('status').textContent).toBe('Save failed · retrying in 5 seconds');
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
