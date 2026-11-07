import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ButtonIconSelect } from './button-icon-select';
import { ImageIcon } from 'lucide-react';

describe('button-icon-select', () => {
  it('renders with its public props and supports the expected behavior', () => {
    const onOptionChange = vi.fn();
    render(<ButtonIconSelect options={[{ value: 'image', icon: ImageIcon, text: 'Image' }, { value: 'video', icon: ImageIcon, text: 'Video' }]} onOptionChange={onOptionChange} />);
    fireEvent.click(screen.getByRole('button', { name: 'Video' }));
    expect(onOptionChange).toHaveBeenCalledWith('video');
  });
});
