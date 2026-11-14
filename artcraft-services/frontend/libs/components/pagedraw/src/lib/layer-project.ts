import { Node, type NodeProps } from './Node';
import { useSceneStore, type LineNode, type SceneState } from './stores/SceneState';
import type { BaseSelectorImage } from './types';

type StoredNode = Omit<NodeProps, 'imageElement' | 'imageFile'> | LineNode;
type Settings = Pick<SceneState, 'brushColor' | 'brushSize' | 'brushOpacity' | 'eraserSize' | 'fillColor' | 'aspectRatioType' | 'inpaintBrushSize' | 'inpaintOperation'>;
export interface LayerProject {
  format: 'artcraft-layer-project';
  version: 1;
  baseImage: BaseSelectorImage;
  drawNodes: StoredNode[];
  inpaintLineNodes: LineNode[];
  settings: Settings;
}

/** Embed pixels, not temporary signed/blob URLs, without resizing the source. */
export async function captureLayerProject(): Promise<LayerProject> {
  const state = useSceneStore.getState();
  if (!state.baseImageInfo || (!state.baseImageInfo.isBlankCanvas && !state.baseImageBitmap)) {
    throw new Error('Open an image or create a canvas before saving.');
  }
  const baseImage = { ...state.baseImageInfo };
  delete baseImage.thumbnailUrlTemplate;
  delete baseImage.fullImageUrl;
  const nodes = state.drawNodes.map(node => {
    if (node.type === 'line') return { ...node, points: [...node.points] };
    const { imageFile, imageElement, ...properties } = node;
    return { ...properties, imageUrl: imageFile ?? imageElement?.src ?? node.imageUrl };
  });
  const masks = structuredClone(state.inpaintLineNodes);
  const settings: Settings = {
    brushColor: state.brushColor, brushSize: state.brushSize, brushOpacity: state.brushOpacity,
    eraserSize: state.eraserSize, fillColor: state.fillColor, aspectRatioType: state.aspectRatioType,
    inpaintBrushSize: state.inpaintBrushSize, inpaintOperation: state.inpaintOperation,
  };
  // Every remote image this document uses, collected before any of them is read: a failed read must
  // not drop the others from the cache.
  const used = new Set<string>();
  for (const source of [...nodes.map(node => node.type === 'image' ? node.imageUrl : undefined), baseImage.isBlankCanvas ? undefined : baseImage.url]) {
    if (typeof source === 'string' && !source.startsWith('data:')) used.add(source);
  }
  try {
    const drawNodes = await Promise.all(nodes.map(async node => {
      if (node.type !== 'image') return node as StoredNode;
      if (!node.imageUrl) throw new Error('An image layer has not finished loading.');
      return { ...node, imageUrl: await embedImage(node.imageUrl) } as StoredNode;
    }));
    baseImage.url = baseImage.isBlankCanvas ? '' : await embedImage(baseImage.url);
    return { format: 'artcraft-layer-project', version: 1, baseImage, drawNodes, inpaintLineNodes: masks, settings };
  } finally {
    // Keep exactly the remote images this document uses, however many: the next capture (offline too)
    // needs all of them, and images of documents no longer open are dropped.
    for (const url of embeddedUrls.keys()) if (!used.has(url)) embeddedUrls.delete(url);
  }
}

/** Decode every asset before changing the current document. Failed opens preserve it. */
export async function restoreLayerProject(value: unknown, isCurrent: () => boolean = () => true): Promise<void> {
  const document = validateLayerProject(value);
  const bitmap = document.baseImage.isBlankCanvas ? null : await loadImage(document.baseImage.url);
  const drawNodes = await Promise.all(document.drawNodes.map(async data => {
    if (data.type === 'line') return structuredClone(data);
    const node = new Node(data);
    if (node.type === 'image') await node.setImageFromUrl(node.imageUrl!);
    return node;
  }));
  if (!isCurrent()) throw new Error("Project open was cancelled.");
  const baseImage = { ...document.baseImage };
  // Invalidate any pending base-image selection before committing the prepared scene.
  useSceneStore.getState().setBaseImageInfo(null);
  useSceneStore.setState({
    ...document.settings, baseImageInfo: baseImage, baseImageBitmap: bitmap,
    drawNodes, inpaintLineNodes: structuredClone(document.inpaintLineNodes),
    selectedNodeIds: [], clipboard: [], historyImageNodeMap: new Map(),
    historyImageBundles: [{ images: [baseImage] }],
    history: [], historyIndex: -1, activeTool: 'select',
  });
  useSceneStore.getState().saveState();
}

export function validateLayerProject(value: unknown): LayerProject {
  const doc = value as LayerProject;
  if (!doc || doc.format !== 'artcraft-layer-project' || doc.version !== 1 || !doc.baseImage ||
      !Array.isArray(doc.drawNodes) || !Array.isArray(doc.inpaintLineNodes) || !doc.settings) {
    throw new Error('This file is not a supported 2D layer project.');
  }
  const embedded = (url: unknown) => typeof url === 'string' && /^data:image\/(png|jpeg|webp|gif|avif|bmp);base64,/i.test(url);
  if (doc.baseImage.isBlankCanvas) {
    if (![doc.baseImage.blankCanvasWidth, doc.baseImage.blankCanvasHeight].every(n => typeof n === 'number' && Number.isFinite(n) && n > 0)) {
      throw new Error('Invalid canvas dimensions.');
    }
  } else if (!embedded(doc.baseImage.url)) throw new Error('The project is missing its embedded base image.');
  const ids = new Set<string>();
  for (const node of [...doc.drawNodes, ...doc.inpaintLineNodes]) {
    if (!node || typeof node.id !== 'string' || ids.has(node.id) ||
        !['rectangle', 'circle', 'triangle', 'image', 'line'].includes(node.type)) throw new Error('Invalid project layer.');
    ids.add(node.id);
    if (node.type === 'line') {
      if (!Array.isArray(node.points) || !node.points.every(Number.isFinite)) throw new Error('Invalid brush stroke.');
    } else if (![node.x, node.y, node.width, node.height].every(Number.isFinite) ||
        (node.type === 'image' && !embedded(node.imageUrl))) throw new Error('Invalid image or shape layer.');
  }
  return doc;
}

// Offline snapshots must not depend on re-downloading a remote image that was already embedded once.
// captureLayerProject() keeps the URLs the open document uses and drops the others.
const embeddedUrls = new Map<string, Promise<string>>();
const embeddedFiles = new WeakMap<File, Promise<string>>();

function embedImage(source: string | File): Promise<string> {
  if (typeof source === 'string' && source.startsWith('data:')) return Promise.resolve(source);
  const cached = typeof source === 'string' ? embeddedUrls.get(source) : embeddedFiles.get(source);
  if (cached) return cached;
  const result = readAsDataUrl(source);
  if (typeof source === 'string') embeddedUrls.set(source, result);
  else embeddedFiles.set(source, result);
  // Failures are not cached, so a later retry reads the image again.
  result.catch(() => {
    if (typeof source === 'string') { if (embeddedUrls.get(source) === result) embeddedUrls.delete(source); }
    else if (embeddedFiles.get(source) === result) embeddedFiles.delete(source);
  });
  return result;
}

async function readAsDataUrl(source: string | File): Promise<string> {
  let blob: Blob;
  if (typeof source === 'string') {
    const response = await fetch(source);
    if (!response.ok) throw new Error('Could not read an image layer. Please retry.');
    blob = await response.blob();
  } else blob = source;
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error('Could not embed an image layer.'));
    reader.readAsDataURL(blob);
  });
}

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error('An embedded project image could not be decoded.'));
    image.src = url;
  });
}
