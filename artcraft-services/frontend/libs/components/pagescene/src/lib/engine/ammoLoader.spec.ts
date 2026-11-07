import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type AmmoTestGlobal = typeof globalThis & { Ammo?: unknown };
const host = globalThis as AmmoTestGlobal;
const runtime = { btVector3: class {} };

beforeEach(() => { vi.resetModules(); delete host.Ammo; document.body.innerHTML = ''; });
afterEach(() => { delete host.Ammo; document.body.innerHTML = ''; });

async function loader() { return (await import('./ammoLoader')).ensureAmmoLoaded; }
function script() { return document.querySelector<HTMLScriptElement>('script')!; }
function finish() {
  host.Ammo = vi.fn().mockResolvedValue(runtime);
  script().dispatchEvent(new Event('load'));
}

describe('Ammo initialization', () => {
  it('shares one script and waits for the WASM runtime across concurrent mounts', async () => {
    const load = await loader();
    const first = load();
    expect(load()).toBe(first);
    expect(document.querySelectorAll('script')).toHaveLength(1);
    let initialized = false;
    void first.then(() => { initialized = true; });
    await Promise.resolve();
    expect(initialized).toBe(false);
    finish();
    await first;
    expect(host.Ammo).toBe(runtime);
  });

  it('uses root-relative script and WASM URLs on nested editor routes', async () => {
    history.replaceState(null, '', '/edit-3d/project');
    const load = await loader();
    const pending = load();
    expect(script().getAttribute('src')).toBe('/jsm/libs/ammo.wasm.js');
    const factory = vi.fn().mockResolvedValue(runtime);
    host.Ammo = factory;
    script().dispatchEvent(new Event('load'));
    await pending;
    expect(factory.mock.calls[0][0].locateFile('ammo.wasm.wasm')).toBe('/jsm/libs/ammo.wasm.wasm');
    history.replaceState(null, '', '/');
  });

  it('waits for an existing pending script instead of reporting early success', async () => {
    const existing = document.createElement('script');
    existing.src = '/jsm/libs/ammo.wasm.js'; document.body.appendChild(existing);
    const load = await loader(); const pending = load();
    expect(document.querySelectorAll('script')).toHaveLength(1);
    finish(); await pending; expect(host.Ammo).toBe(runtime);
  });

  it('removes a failed script and allows a successful retry', async () => {
    const load = await loader(); const first = load();
    const failed = expect(first).rejects.toThrow('Failed to load');
    script().dispatchEvent(new Event('error')); await failed;
    expect(document.querySelectorAll('script')).toHaveLength(0);
    const retry = load(); expect(retry).not.toBe(first);
    finish(); await retry; expect(host.Ammo).toBe(runtime);
  });

  it('reuses an initialized runtime without injecting another script', async () => {
    host.Ammo = runtime; await (await loader())();
    expect(document.querySelectorAll('script')).toHaveLength(0);
  });
});
