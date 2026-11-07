import { afterEach, expect, it, vi } from 'vitest';
import { SaveManager } from '../../../../../libs/components/video-editor/src/lib/core/managers/save-manager';

afterEach(() => { vi.useRealTimers(); });

it('keeps failed saves dirty and retries instead of reporting clean', async () => {
  vi.useFakeTimers();
  const saveCurrentProject = vi.fn().mockRejectedValueOnce(new Error('Offline')).mockResolvedValue(undefined);
  const subscribe = () => () => {};
  const manager = new SaveManager({ editor: {
    scenes: { subscribe }, timeline: { subscribe },
    project: { getActive: () => ({}), getIsLoading: () => false,
      getMigrationState: () => ({ isMigrating: false }), saveCurrentProject },
  } as never });
  manager.start();
  await expect(manager.flush()).rejects.toThrow('Offline');
  expect(manager.getIsDirty()).toBe(true);
  await vi.advanceTimersByTimeAsync(4999);
  expect(saveCurrentProject).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(1);
  expect(saveCurrentProject).toHaveBeenCalledTimes(2);
  expect(manager.getIsDirty()).toBe(false);
  manager.stop();
});
