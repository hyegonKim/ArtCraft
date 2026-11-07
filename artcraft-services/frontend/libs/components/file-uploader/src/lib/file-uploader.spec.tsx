import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { FileUploader } from './file-uploader';

describe('file-uploader', () => {
  it('renders with its public props and supports the expected behavior', () => {
    const handleChange = vi.fn();
    const { container } = render(<FileUploader files={[]} fileTypes={['PNG']} handleChange={handleChange} />);
    const input = container.querySelector('input[type="file"]');
    expect(input).toBeTruthy();
    const file = new File(['image'], 'art.png', { type: 'image/png' });
    fireEvent.change(input!, { target: { files: [file] } });
    expect(handleChange).toHaveBeenCalledWith([file]);
  });
});
