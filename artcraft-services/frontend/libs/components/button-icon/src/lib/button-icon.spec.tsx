import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ButtonIcon } from './button-icon';
import { PlusIcon } from 'lucide-react';

describe('button-icon', () => {
  it('renders with its public props and supports the expected behavior', () => {
    const onClick = vi.fn();
    const { rerender } = render(<ButtonIcon icon={PlusIcon} onClick={onClick} />);
    fireEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalledTimes(1);
    rerender(<ButtonIcon icon={PlusIcon} onClick={onClick} disabled />);
    fireEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
