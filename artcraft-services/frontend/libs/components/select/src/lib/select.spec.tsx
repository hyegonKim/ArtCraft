import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { Select } from './select';

describe('select', () => {
  it('renders with its public props and supports the expected behavior', () => {
    render(<Select options={[{ label: 'First', value: 'first' }]} value="first" onChange={vi.fn()} />);
    expect(screen.getByRole('button').textContent).toContain('First');
  });
});
