import { beforeEach, expect, it, vi } from 'vitest';
import { uploadByKind } from './upload-by-kind';

const mocks = vi.hoisted(() => ({ video: vi.fn(), image: vi.fn(), audio: vi.fn() }));
vi.mock('@storyteller/api', () => ({
  EIntermediateFile: { false: 1 },
  MediaUploadApi: class { UploadNewVideo = mocks.video; UploadImage = mocks.image; UploadAudio = mocks.audio; },
}));
beforeEach(() => { for (const mock of Object.values(mocks)) mock.mockReset().mockResolvedValue({ success: true, data: 'm_uploaded' }); });

it.each(['video', 'image'] as const)('keeps user %s imports and exports visible in the media library', async (kind) => {
  const blob = new Blob(['media']);
  await expect(uploadByKind({ kind, blob, fileName: `test.${kind}` })).resolves.toBe('m_uploaded');
  expect(mocks[kind]).toHaveBeenCalledWith(expect.objectContaining({ blob, is_intermediate_system_file: 1 }));
});

it('preserves audio dispatch and rejects upload failures', async () => {
  mocks.audio.mockResolvedValueOnce({ success: false, errorMessage: 'Storage full' });
  await expect(uploadByKind({ kind: 'audio', blob: new Blob(), fileName: 'test.wav' })).rejects.toThrow('Storage full');
  expect(mocks.video).not.toHaveBeenCalled();
});
