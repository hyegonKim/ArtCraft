import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { createLayerProjectStorage } from './layer-project-storage';
import { validateLayerProject, type LayerProject } from '@storyteller/ui-pagedraw';
vi.mock('@storyteller/ui-pagedraw', () => ({ validateLayerProject: (value: unknown) => value }));
const calls = vi.hoisted(() => ({ create: vi.fn(), update: vi.fn(), rename: vi.fn(), list: vi.fn(), get: vi.fn() }));
vi.mock('@storyteller/api', () => ({
  Visibility: { Private: 'private' },
  ProjectsApi: class { UploadNewProject = calls.create; UpdateProject = calls.update; ListSessionProjects = calls.list; },
  MediaFilesApi: class { RenameMediaFileByToken = calls.rename; GetMediaFileByToken = calls.get; },
}));
const document = { format: 'artcraft-layer-project', version: 1,
  baseImage: { isBlankCanvas: true, blankCanvasWidth: 1920, blankCanvasHeight: 1080, url: '', mediaToken: 'blank' },
  drawNodes: [], inpaintLineNodes: [], settings: {} } as LayerProject;
beforeEach(() => vi.resetAllMocks());
afterEach(() => vi.unstubAllGlobals());

it('creates a private editor_2d document, then updates the same token and server title', async () => {
  calls.create.mockResolvedValue({ success: true, data: 'm_saved' });
  calls.update.mockResolvedValue({ success: true }); calls.rename.mockResolvedValue({ success: true });
  const storage = createLayerProjectStorage();
  const token = await storage.save(document, 'Layer test');
  await storage.save(document, 'Renamed layer test', token);
  expect(calls.create.mock.calls[0][0]).toMatchObject({ projectType: 'editor_2d', maybe_visibility: 'private' });
  const content = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader(); reader.onload = () => resolve(String(reader.result));
    reader.onerror = reject; reader.readAsText(calls.create.mock.calls[0][0].blob);
  });
  expect(JSON.parse(content)).toEqual(document);
  expect(calls.create).toHaveBeenCalledTimes(1);
  expect(calls.update.mock.calls[0][0]).toMatchObject({ projectType: 'editor_2d', token: 'm_saved' });
  expect(calls.rename).toHaveBeenCalledWith({ mediaToken: 'm_saved', name: 'Renamed layer test' });
});
it('propagates failed saves so the UI stays dirty and can retry', async () => {
  calls.update.mockResolvedValue({ success: false, errorMessage: 'Offline' });
  await expect(createLayerProjectStorage().save(document, 'Test', 'm_saved')).rejects.toThrow('Offline');
  expect(calls.rename).not.toHaveBeenCalled();
});
it('loads fresh authorized project bytes and the authoritative server title', async () => {
  calls.get.mockResolvedValue({ success: true, data: { maybe_title: 'Renamed', media_links: { cdn_url: 'https://storage.test/project?fresh=true' } } });
  const fetch = vi.fn().mockResolvedValue({ ok: true, json: async () => document });vi.stubGlobal('fetch', fetch);
  expect(await createLayerProjectStorage().load('m_saved')).toEqual({ name: 'Renamed', document: validateLayerProject(document) });
  expect(fetch).toHaveBeenCalledWith('https://storage.test/project?fresh=true', { cache: 'no-store' });
});
it('lists only 2D projects with pagination and propagates list failures', async () => {
  calls.list.mockResolvedValueOnce({ success: true, data: [], pagination: { maybe_next: 'next' } }).mockResolvedValueOnce({ success: false });
  const storage = createLayerProjectStorage();
  expect(await storage.list('cursor')).toEqual({ rows: [], next: 'next' });
  expect(calls.list).toHaveBeenCalledWith({ filter_project_type: 'editor_2d', page_size: 30, cursor: 'cursor' });
  await expect(storage.list()).rejects.toThrow('Could not load');
});

it('does not issue a redundant rename on every autosave when the name is unchanged', async () => {
  calls.create.mockResolvedValue({ success: true, data: 'm_saved' });
  calls.update.mockResolvedValue({ success: true });
  const storage = createLayerProjectStorage();
  const token = await storage.save(document, 'Unchanged');
  await storage.save(document, 'Unchanged', token, 'retry-request-id');
  expect(calls.rename).not.toHaveBeenCalled();
  expect(calls.update.mock.calls[0][0].uuid).toBe('retry-request-id');
});
