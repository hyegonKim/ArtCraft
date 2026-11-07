import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useSceneStore } from './SceneState';

vi.mock('../Node', () => ({ Node: class {} }));
const images: Array<{ src: string; crossOrigin: string }> = [];

beforeEach(() => {
  images.length = 0;
  vi.stubGlobal('Image', class {
    src = '';
    crossOrigin = '';
    constructor() { images.push(this); }
  });
  useSceneStore.setState({ baseImageInfo: null, baseImageBitmap: null, drawNodes: [], inpaintLineNodes: [] });
});
afterEach(() => { vi.unstubAllGlobals(); });

it.each([
  'https://storage.test/staging-media/file?expires=123&signature=abc',
  'https://storage.test/image.png?existing=value#fragment',
  'data:image/png;base64,abcd',
  'blob:https://private.test/file',
])('preserves an authorized or local base image URL: %s', (url) => {
  useSceneStore.getState().setBaseImageInfo({ url, mediaToken: 'm_test' });
  expect(images[0].src).toBe(url);
  expect(images[0].crossOrigin).toBe('anonymous');
});

it('retains the existing cache marker for plain CDN image URLs', () => {
  useSceneStore.getState().setBaseImageInfo({ url: 'https://cdn.test/image.png', mediaToken: 'm_test' });
  expect(images[0].src).toBe('https://cdn.test/image.png?basecanvasimg');
});
