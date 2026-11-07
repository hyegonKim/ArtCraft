import { expect, it } from 'vitest';
import { addCorsParam, getThumbnailUrl } from './thumbnail-utils';

it.each([
  'https://storage.test/file.png?expires=123&signature=abc',
  'https://cdn.test/file.png?width=1024#preview',
  'data:image/png;base64,abcd',
  'blob:https://private.test/file',
])('keeps authorized and local media links intact: %s', (url) => {
  expect(addCorsParam(url)).toBe(url);
  expect(getThumbnailUrl(url, { addCors: true })).toBe(url);
});

it('keeps the existing CDN cache marker before a fragment', () => {
  expect(addCorsParam('https://cdn.test/image.png')).toBe('https://cdn.test/image.png?cors=1');
  expect(addCorsParam('https://cdn.test/image.png#preview')).toBe('https://cdn.test/image.png?cors=1#preview');
});

it.each([null, undefined, ''])('handles missing media URLs: %s', (url) => {
  expect(addCorsParam(url)).toBeNull();
});
