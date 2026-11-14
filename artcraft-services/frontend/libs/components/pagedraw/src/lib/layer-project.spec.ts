import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { captureLayerProject, restoreLayerProject } from './layer-project';
import { Node } from './Node';
import { useSceneStore, type LineNode } from './stores/SceneState';

const png = 'data:image/png;base64,aGVsbG8=';
const stroke: LineNode = { id: 'brush', type: 'line', points: [10, 20, 80, 60], stroke: '#ff0000',
  strokeWidth: 12, opacity: 0.4, draggable: true, globalCompositeOperation: 'source-over' };
beforeEach(() => {
  useSceneStore.getState().RESET();
  useSceneStore.setState({ baseImageInfo: { isBlankCanvas: true, blankCanvasWidth: 1920, blankCanvasHeight: 1080, mediaToken: 'blank', url: '' } });
  vi.stubGlobal('Image', class {
    onload?: () => void; onerror?: () => void;
    set src(value: string) { queueMicrotask(() => value.includes('broken') ? this.onerror?.() : this.onload?.()); }
  });
});
afterEach(() => vi.unstubAllGlobals());

it('round-trips ordered shapes, brush/eraser strokes, masks, transforms, locks and original canvas size', async () => {
  const shape = new Node({ id: 'shape', type: 'rectangle', x: 123, y: 456, width: 321, height: 222,
    fill: '#123456', rotation: 17, scaleX: 0, scaleY: 2, locked: true, zIndex: 7 });
  useSceneStore.setState({ drawNodes: [shape, stroke, { ...stroke, id: 'erase', globalCompositeOperation: 'destination-out' }],
    inpaintLineNodes: [{ ...stroke, id: 'mask' }], brushOpacity: 0.4, fillColor: '#ededed' });
  const saved = JSON.parse(JSON.stringify(await captureLayerProject()));
  useSceneStore.getState().RESET();
  await restoreLayerProject(saved);
  const state = useSceneStore.getState();
  expect(state.drawNodes.map(n => n.id)).toEqual(['shape', 'brush', 'erase']);
  expect(state.drawNodes[0]).toBeInstanceOf(Node);
  expect(state.drawNodes[0]).toMatchObject({ x: 123, y: 456, scaleX: 0, scaleY: 2, rotation: 17, locked: true, fill: '#123456' });
  expect(state.drawNodes[1]).toEqual(stroke);
  expect(state.inpaintLineNodes[0].id).toBe('mask');
  expect(state.baseImageInfo).toMatchObject({ blankCanvasWidth: 1920, blankCanvasHeight: 1080 });
  expect(state.historyImageBundles[0].images[0]).toBe(state.baseImageInfo);
  expect(state.history).toHaveLength(1);
  expect(state.brushOpacity).toBe(0.4);
});

it('embeds the original base and remote image layer so expired signed URLs are not needed on reopen', async () => {
  const fetchImage = vi.fn().mockResolvedValue({ ok: true, blob: async () => new Blob(['hello'], { type: 'image/png' }) });
  vi.stubGlobal('fetch', fetchImage);
  useSceneStore.setState({ baseImageInfo: { url: 'https://storage.test/base?signature=expires', mediaToken: 'base' },
    baseImageBitmap: {} as HTMLImageElement,
    drawNodes: [new Node({ id: 'image', type: 'image', x: 20, y: 30, width: 1200, height: 800, fill: '', imageUrl: 'https://storage.test/layer?signature=expires' })] });
  const saved = await captureLayerProject();
  expect(JSON.stringify(saved)).not.toContain('signature');
  expect(saved.baseImage.url).toBe(png);
  fetchImage.mockRejectedValue(new Error('Links expired'));
  await restoreLayerProject(JSON.parse(JSON.stringify(saved)));
  expect(useSceneStore.getState().drawNodes[0]).toMatchObject({ type: 'image', width: 1200, height: 800, imageUrl: png });
  expect(fetchImage).toHaveBeenCalledTimes(2);
});

it('rejects an unreadable asset on save instead of silently dropping a layer', async () => {
  useSceneStore.setState({ drawNodes: [new Node({ id: 'missing', type: 'image', x: 0, y: 0, width: 10, height: 10, fill: '' })] });
  await expect(captureLayerProject()).rejects.toThrow('not finished loading');
});

it.each(['version', 'external image', 'decode failure'])('keeps the current document when opening fails: %s', async kind => {
  useSceneStore.setState({ drawNodes: [new Node({ id: 'original', type: 'circle', x: 1, y: 2, width: 3, height: 4, fill: 'red' })] });
  const saved = await captureLayerProject();
  if (kind === 'version') saved.version = 2 as 1;
  else saved.baseImage = { url: kind === 'external image' ? 'https://untrusted.test/image.png' : 'data:image/png;base64,broken', mediaToken: 'bad' };
  await expect(restoreLayerProject(saved)).rejects.toThrow();
  expect(useSceneStore.getState().drawNodes[0].id).toBe('original');
});

it('does not commit a project whose host navigated away while images decoded', async () => {
  const saved = await captureLayerProject();
  useSceneStore.setState({ drawNodes: [new Node({ id: 'keep', type: 'rectangle', x: 0, y: 0, width: 5, height: 5, fill: 'red' })] });
  await expect(restoreLayerProject(saved, () => false)).rejects.toThrow('cancelled');
  expect(useSceneStore.getState().drawNodes[0].id).toBe('keep');
});

it('captures again while offline from images it already embedded, without caching a failed read', async () => {
  const fetchImage = vi.fn().mockRejectedValueOnce(new TypeError('Failed to fetch'))
    .mockResolvedValue({ ok: true, blob: async () => new Blob(['hello'], { type: 'image/png' }) });
  vi.stubGlobal('fetch', fetchImage);
  useSceneStore.setState({ baseImageInfo: { url: 'https://storage.test/offline-base?signature=1', mediaToken: 'base' },
    baseImageBitmap: {} as HTMLImageElement, drawNodes: [] });
  await expect(captureLayerProject()).rejects.toThrow('Failed to fetch');
  expect((await captureLayerProject()).baseImage.url).toBe(png);
  fetchImage.mockRejectedValue(new TypeError('Failed to fetch'));
  useSceneStore.setState({ drawNodes: [stroke] });
  const offline = await captureLayerProject();
  expect(offline.baseImage.url).toBe(png);
  expect(offline.drawNodes).toEqual([stroke]);
  expect(fetchImage).toHaveBeenCalledTimes(2);
});

const remoteImage = (id: string, url: string) =>
  ({ id, type: 'image', x: 0, y: 0, width: 10, height: 10, imageUrl: url }) as unknown as Node;
const imageBytes = { ok: true, blob: async () => new Blob(['hello'], { type: 'image/png' }) };

it('keeps every remote image of the open document for offline captures, however many there are', async () => {
  const fetchImage = vi.fn().mockResolvedValue(imageBytes);
  vi.stubGlobal('fetch', fetchImage);
  useSceneStore.setState({ drawNodes: Array.from({ length: 30 }, (_, i) => remoteImage(`img${i}`, `https://storage.test/layer-${i}.png`)) });
  await captureLayerProject();
  expect(fetchImage).toHaveBeenCalledTimes(30);
  fetchImage.mockRejectedValue(new TypeError('Failed to fetch'));
  const offline = await captureLayerProject();
  expect(offline.drawNodes.every(node => node.type === 'image' && node.imageUrl === png)).toBe(true);
  expect(fetchImage).toHaveBeenCalledTimes(30);
});

it('keeps the base image for offline captures when another image of the document fails to load', async () => {
  const fetchImage = vi.fn(async (url: string) => {
    if (url.includes('new-layer')) throw new TypeError('Failed to fetch');
    return imageBytes;
  });
  vi.stubGlobal('fetch', fetchImage);
  useSceneStore.setState({ baseImageInfo: { url: 'https://storage.test/base?signature=2', mediaToken: 'base' },
    baseImageBitmap: {} as HTMLImageElement, drawNodes: [] });
  await captureLayerProject();
  useSceneStore.setState({ drawNodes: [remoteImage('new', 'https://storage.test/new-layer.png')] });
  await expect(captureLayerProject()).rejects.toThrow('Failed to fetch');
  useSceneStore.setState({ drawNodes: [] });
  fetchImage.mockRejectedValue(new TypeError('Failed to fetch'));
  expect((await captureLayerProject()).baseImage.url).toBe(png);
});
