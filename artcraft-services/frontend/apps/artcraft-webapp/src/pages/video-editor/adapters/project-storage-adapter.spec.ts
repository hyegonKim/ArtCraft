import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { createWebappProjectStorage } from './project-storage-adapter';

const mocks = vi.hoisted(() => ({ create: vi.fn(), update: vi.fn(), error: vi.fn(), get: vi.fn(), rename: vi.fn() }));
vi.mock('@storyteller/api', () => ({
  ProjectsApi: class { UploadNewProject = mocks.create; UpdateProject = mocks.update; },
  MediaFilesApi: class { GetMediaFileByToken = mocks.get; RenameMediaFileByToken = mocks.rename; },
}));
vi.mock('@storyteller/common', () => ({ getThumbnailUrl: () => undefined }));
vi.mock('./toast-adapter', () => ({ webappToastAdapter: { error: mocks.error } }));
vi.mock('./thumbnail-capture', () => ({ captureAndUploadProjectCover: vi.fn() }));

beforeEach(() => { vi.clearAllMocks(); mocks.rename.mockResolvedValue({ success: true }); vi.spyOn(console, 'error').mockImplementation(() => {}); });
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });

it('rejects a failed cloud save and permits a later retry without duplicate creation', async () => {
  mocks.create.mockResolvedValueOnce({ success: false, errorMessage: 'Offline' })
    .mockResolvedValueOnce({ success: true, data: 'mf_saved' });
  mocks.update.mockResolvedValue({ success: true, data: 'mf_saved' });
  const onProjectCreated = vi.fn();
  const storage = createWebappProjectStorage({ onProjectCreated });
  const project = { id: 'local-test', name: 'Cloud test', updatedAt: Date.now(), data: { clips: [] } };
  await expect(storage.saveProject(project)).rejects.toThrow('Offline');
  expect(onProjectCreated).not.toHaveBeenCalled();
  await storage.saveProject(project);
  await storage.saveProject(project);
  expect(mocks.create).toHaveBeenCalledTimes(2);
  expect(mocks.update).toHaveBeenCalledTimes(1);
  expect(onProjectCreated).toHaveBeenCalledExactlyOnceWith({ localId: 'local-test', token: 'mf_saved' });
  await storage.saveProject({ ...project, name: 'New cloud name' });
  expect(mocks.rename).toHaveBeenCalledExactlyOnceWith({ mediaToken: 'mf_saved', name: 'New cloud name' });
});

it.each(['versioned', 'legacy'])('restores the server identity and renamed title for a %s project', async (shape) => {
  const project = { metadata: { id: 'local-old', name: 'Old name' }, scenes: [] };
  const data = shape === 'versioned' ? { version: 1, project, media: [] } : project;
  const fetch = vi.fn().mockResolvedValue({ ok: true, json: async () => data });
  vi.stubGlobal('fetch', fetch);
  mocks.get.mockResolvedValue({ success: true, data: { maybe_title: 'Renamed on server',
    updated_at: '2026-11-07T08:00:00Z', media_links: { cdn_url: 'https://test/project' } } });
  const loaded = await createWebappProjectStorage().loadProject('mf_saved');
  const restored = loaded?.data as { project?: typeof project; metadata?: typeof project.metadata };
  expect((restored.project ?? restored).metadata).toMatchObject({ id: 'mf_saved', name: 'Renamed on server' });
  expect(loaded?.name).toBe('Renamed on server');
});
