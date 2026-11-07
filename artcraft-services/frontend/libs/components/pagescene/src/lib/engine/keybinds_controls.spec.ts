import { describe, expect, it } from 'vitest';
import { Object3D, PerspectiveCamera, Vector3 } from 'three';
import { PointerLockControls } from 'three/examples/jsm/controls/PointerLockControls.js';
import { MouseControls } from './keybinds_controls';

function createFocusContext(locked = false, selected: Object3D[] = []) {
  const camera = new PerspectiveCamera();
  const lockControls = new PointerLockControls(camera);
  const controls = Object.assign(Object.create(MouseControls.prototype), {
    deps: { isViewportLocked: () => locked }, lockControls, selected,
  }) as MouseControls;
  return { controls, camera };
}

describe('MouseControls focus', () => {
  it('focuses the selected object through the current Three.js control object', () => {
    const target = new Object3D();
    target.position.set(2, 3, 4);
    const { controls, camera } = createFocusContext(false, [target]);
    controls.focus();
    expect(camera.position.distanceTo(target.position)).toBeCloseTo(Math.sqrt(50));
    expect(camera.getWorldDirection(new Vector3()).dot(target.position.clone().sub(camera.position).normalize())).toBeCloseTo(1);
  });

  it('leaves the camera unchanged when the viewport is locked', () => {
    const { controls, camera } = createFocusContext(true, [new Object3D()]);
    controls.focus();
    expect(camera.position.toArray()).toEqual([0, 0, 0]);
  });

  it('does nothing for an empty selection', () => {
    const { controls, camera } = createFocusContext();
    expect(() => controls.focus()).not.toThrow();
    expect(camera.position.toArray()).toEqual([0, 0, 0]);
  });
});
