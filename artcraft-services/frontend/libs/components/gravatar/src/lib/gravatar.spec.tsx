import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { Gravatar } from './gravatar';

describe('gravatar', () => {
  it('renders with its public props and supports the expected behavior', () => {
    render(<Gravatar size={48} username="Artist" avatarIndex={2} />);
    const image = screen.getByRole('img', { name: "Artist's gravatar" });
    expect(image.getAttribute('width')).toBe('48');
    fireEvent.error(image);
    expect(image.getAttribute('src')).toBe('/resources/avatars/2.webp');
  });
});
