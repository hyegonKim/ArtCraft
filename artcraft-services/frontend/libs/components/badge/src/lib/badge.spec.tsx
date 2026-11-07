import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { Badge } from './badge';

describe('badge', () => {
  it('renders with its public props and supports the expected behavior', () => {
    render(<Badge label="Preview" />);
    expect(screen.getByText('Preview')).toBeTruthy();
  });
});
