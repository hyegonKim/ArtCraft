// Load and initialize the installed Ammo script once, including concurrent
// editor mounts. Use a root-relative URL so nested editor routes also work.
type AmmoRuntime = { btVector3: unknown };
type AmmoFactory = (options: { locateFile: (name: string) => string }) => Promise<AmmoRuntime>;
const ammoGlobal = globalThis as typeof globalThis & { Ammo?: AmmoRuntime | AmmoFactory };
let ammoLoadPromise: Promise<void> | null = null;

export const ensureAmmoLoaded = (): Promise<void> => {
  if (typeof document === "undefined") return Promise.resolve();
  if (ammoLoadPromise) return ammoLoadPromise;

  const initialize = async () => {
    if (typeof ammoGlobal.Ammo === "function") {
      ammoGlobal.Ammo = await ammoGlobal.Ammo({
        locateFile: (name) => `/jsm/libs/${name}`,
      });
    }
    if (!ammoGlobal.Ammo || !("btVector3" in ammoGlobal.Ammo)) {
      throw new Error("Ammo physics runtime did not initialize.");
    }
  };

  const load = async () => {
    if (!ammoGlobal.Ammo) {
      await new Promise<void>((resolve, reject) => {
        const existing = document.querySelector<HTMLScriptElement>(
          'script[src="/jsm/libs/ammo.wasm.js"]',
        );
        const script = existing ?? document.createElement("script");
        const onLoad = () => { cleanup(); resolve(); };
        const onError = () => {
          cleanup();
          script.remove();
          reject(new Error("Failed to load /jsm/libs/ammo.wasm.js"));
        };
        const cleanup = () => {
          script.removeEventListener("load", onLoad);
          script.removeEventListener("error", onError);
        };
        script.addEventListener("load", onLoad);
        script.addEventListener("error", onError);
        if (!existing) {
          script.src = "/jsm/libs/ammo.wasm.js";
          script.async = true;
          document.body.appendChild(script);
        }
      });
    }
    await initialize();
  };

  ammoLoadPromise = load().catch((error) => {
    ammoLoadPromise = null;
    throw error;
  });
  return ammoLoadPromise;
};
