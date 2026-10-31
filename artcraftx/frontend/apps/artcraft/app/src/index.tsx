import { StrictMode, useEffect } from "react";
import { useSignals, useSignalEffect } from "@preact/signals-react/runtime";
import { BrowserRouter } from "react-router-dom";
import { MainApp } from "./pages/MainApp";
import { GlobalFileDropHandler } from "./components/GlobalFileDropHandler/GlobalFileDropHandler";
import { createRoot } from "react-dom/client";
// Bundled fonts (desktop app — no CDN): Archivo needs the wdth axis for the
// site's stretched display style; Instrument Sans is the body face; IBM Plex
// Mono covers spec labels and readouts.
import "@fontsource-variable/archivo/wdth.css";
import "@fontsource-variable/instrument-sans";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import "@fontsource/ibm-plex-mono/600.css";
import "./styles/normalize.css";
import "./styles/tailwind.css";
import "./styles/base.css";
import EnvironmentVariables from "~/Classes/EnvironmentVariables";
import { pageHeight, pageWidth, persistLogin } from "~/signals";
import { SyncStorytellerApiConfig } from "./api/SyncStorytellerApiConfig";
import { posthog } from "posthog-js";
import { SoundManager } from "@storyteller/soundboard";
import { useModelsStore } from "@storyteller/tauri-api";

// Outside the dev server, suppress the webview's right-click menu and the
// devtools hotkeys (F12, Ctrl+Shift+I/J/C). Release Tauri builds don't
// compile devtools at all — this just keeps the native-feeling chrome from
// leaking browser affordances. Capture phase so page handlers can't re-enable.
if (!import.meta.env.DEV) {
  window.addEventListener(
    "contextmenu",
    (e) => {
      // Keep the native menu where it's genuinely useful — copy/paste in
      // text fields and the mention editor — and suppress it everywhere else.
      const target = e.target as HTMLElement | null;
      const editable = target?.closest?.(
        'input, textarea, [contenteditable]:not([contenteditable="false"])',
      );
      if (!editable) e.preventDefault();
    },
    { capture: true },
  );
  window.addEventListener(
    "keydown",
    (e) => {
      const key = e.key.toUpperCase();
      const devtoolsCombo =
        key === "F12" ||
        (e.ctrlKey && e.shiftKey && (key === "I" || key === "J" || key === "C"));
      if (devtoolsCombo) {
        e.preventDefault();
        e.stopImmediatePropagation();
      }
    },
    { capture: true },
  );
}

// TODO(bt,2025-04-19): Make these configurable
const ENV = {
  GOOGLE_API: "https://studio.storyteller.ai",
  FUNNEL_API: "https://studio.storyteller.ai",
  CDN_API: "https://cdn-2.fakeyou.com",
  GRAVATAR_API: "https://studio.storyteller.ai",
  DEPLOY_PRIME_URL: "https://studio.storyteller.ai",
};

const GlobalSettingsManager = ({ env }: { env: Record<string, string> }) => {
  console.log("GlobalSettingsManager()");

  SyncStorytellerApiConfig();

  useSignals();

  useSignalEffect(() => {
    persistLogin();
  });

  /// Initizations that depends on ENV vars ///
  function PostHogInit() {
    const apiKey = import.meta.env.VITE_POSTHOG_API_KEY;
    posthog.init(apiKey, {
      api_host: "https://us.i.posthog.com/",
      ui_host: "https://us.i.posthog.com/",
    });
  }

  useEffect(() => {
    EnvironmentVariables.initialize(env);
    if (import.meta.env.DEV) {
      return;
    }
    PostHogInit();
  }, [env]);

  // Themes are parked while the app is single-look (the ax palette in :root).
  // The legacy theme classes override every --st-* token, so nothing is
  // applied by default — flip this to restore stored-theme application when
  // a theme picker returns.
  const ENABLE_LEGACY_THEMES = false;
  useEffect(() => {
    const root = document.documentElement;
    const toRemove: string[] = [];
    root.classList.forEach((c) => {
      if (c.startsWith("theme-")) toRemove.push(c);
    });
    toRemove.forEach((c) => root.classList.remove(c));

    if (!ENABLE_LEGACY_THEMES) return;
    const value = (localStorage.getItem("st-theme") || "gray").trim();
    const allowed = ["light", "gray", "black", "aurora", "sunset", "gradient"];
    const normalized = value === "gradient" ? "aurora" : value;
    const theme = allowed.includes(value) ? normalized : "gray";
    root.classList.add(`theme-${theme}`);
  }, []);

  /// Initizations that run only once on 1ST mount ///
  function setPage() {
    // TODO address this issue with zooming
    pageHeight.value = window.innerHeight;
    pageWidth.value = window.innerWidth;
  }

  useEffect(() => {
    setPage();
    window.addEventListener("resize", setPage);
    return () => {
      window.removeEventListener("resize", setPage);
    };
  }, []);

  useEffect(() => {
    SoundManager.install();
  }, []);

  // Load every model list from the Rust `models` crate once on boot. The
  // pickers are empty until this resolves.
  useEffect(() => {
    void useModelsStore.getState().loadModelsFromBackend();
  }, []);

  return null;
};

// TODO: Replace environment variables from `root.tsx`
createRoot(document.getElementById("root")!).render(
  <>
    <StrictMode>
      <BrowserRouter>
        <GlobalSettingsManager env={ENV} />
        <div className="topbar-spacer" data-tauri-drag-region={true} />
        <MainApp />
        <GlobalFileDropHandler />
      </BrowserRouter>
    </StrictMode>
  </>,
);
