import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { TabSelector } from './tab-selector';

describe('tab-selector', () => {
  it('renders with its public props and supports the expected behavior', () => {
    const onTabChange = vi.fn();
    render(<TabSelector tabs={[{ id: 'generate', label: 'Generate' }, { id: 'history', label: 'History' }]} activeTab="generate" onTabChange={onTabChange} />);
    expect(screen.getByRole('tab', { name: 'Generate' }).getAttribute('aria-selected')).toBe('true');
    fireEvent.click(screen.getByRole('tab', { name: 'History' }));
    expect(onTabChange).toHaveBeenCalledWith('history');
  });
});
