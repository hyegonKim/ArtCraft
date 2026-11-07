import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ButtonDropdown } from './button-dropdown';

describe('button-dropdown', () => {
  it('renders with its public props and supports the expected behavior', () => {
    render(<ButtonDropdown label="Actions" options={[{ label: 'Save', onClick: vi.fn() }]} />);
    expect(screen.getByRole('button', { name: 'Actions' })).toBeTruthy();
  });
});
