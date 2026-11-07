import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { Modal } from './modal';

describe('modal', () => {
  it('renders with its public props and supports the expected behavior', () => {
    const onClose = vi.fn();
    render(<Modal isOpen title="Test modal" onClose={onClose}>Modal content</Modal>);
    expect(screen.getByRole('dialog').textContent).toContain('Modal content');
  });
});
