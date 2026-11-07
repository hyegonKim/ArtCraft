import { render, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { Create3dModal } from './create-3d-modal';

const { listen, unlisten } = vi.hoisted(() => {
  const unlisten = vi.fn();
  return { listen: vi.fn().mockResolvedValue(unlisten), unlisten };
});
vi.mock('@tauri-apps/api/event', () => ({ listen }));

describe('Create3dModal', () => {
  it('registers and cleans up the native generation event listener', async () => {
    const { unmount } = render(<Create3dModal />);
    expect(listen).toHaveBeenCalledWith('generation-complete-event', expect.any(Function));
    unmount();
    await waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });
});
