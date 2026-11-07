import { cleanup, render } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import type { ReactNode } from 'react';
import { HistoryStack } from './HistoryStack';

vi.mock('@storyteller/ui-tooltip', () => ({ Tooltip: ({ children }: { children: ReactNode }) => children }));
vi.mock('@storyteller/ui-action-reminder-modal', () => ({
  isActionReminderOpen: { value: false }, showActionReminder: vi.fn(),
}));
afterEach(cleanup);

it('renders authorized history thumbnails and pending backgrounds without corrupting their signature', () => {
  const url = 'https://storage.test/file.png?expires=123&signature=abc';
  const { container } = render(<HistoryStack onClear={() => {}}
    imageBundles={[{ images: [{ url, mediaToken: 'm_test' }] }]}
    pendingPlaceholders={[{ id: 'pending', count: 1 }]} blurredBackgroundUrl={url} />);
  const images = [...container.querySelectorAll('img')];
  expect(images).toHaveLength(2);
  expect(images.map(image => image.getAttribute('src'))).toEqual([url, url]);
});
