import { Copy, Minus, Settings, Square, X } from "lucide-react";
import { signal } from "@preact/signals-react";
import { useSignals } from "@preact/signals-react/runtime";
import { useCreditsState } from "@storyteller/credits";
import { gtagEvent } from "@storyteller/google-analytics";
import { ProviderBillingModal } from "@storyteller/provider-billing-modal";
import { ProviderSetupModal } from "@storyteller/provider-setup-modal";
import { useSubscriptionState } from "@storyteller/subscription";
import {
  useCreditsBalanceChangedEvent,
  useSubscriptionPlanChangedEvent,
} from "@storyteller/tauri-events";
import {
  useTauriPlatform,
  useTauriWindowControls,
} from "@storyteller/tauri-utils";
import { CostBreakdownModal, CreditsModal } from "@storyteller/ui-pricing-modal";
import { ModelPage, useSelectedModel } from "@storyteller/ui-model-selector";
import { Tooltip } from "@storyteller/ui-tooltip";
import { useEffect, useRef, useState } from "react";
import { twMerge } from "tailwind-merge";
import { APP_DESCRIPTORS } from "~/config/appMenu";
import { TabId, useTabStore } from "~/pages/Stores/TabState";
import { AUTH_STATUS } from "~/enums";
import { authentication } from "~/signals";
import { TaskQueue } from "./TaskQueue";

const SWITCHER_THROTTLE_TIME = 500; // milliseconds
const CREDITS_POLL_INTERVAL = 60_000; // milliseconds

export const topNavMediaId = signal<string>("");
export const topNavMediaUrl = signal<string>("");

export const TopBar = () => {
  useSignals();

  const { isDesktop, isMaximized, minimize, toggleMaximize, close } =
    useTauriWindowControls();
  const platform = useTauriPlatform();

  const tabStore = useTabStore();

  const [disableSwitcher, setDisableSwitcher] = useState(false);
  const switcherThrottle = useRef(false);

  // Just calling this function kills the app:
  const subscriptionStore = useSubscriptionState();

  // Fetch credits + subscription on entering LOGGED_IN, then poll credits every
  // 60s. Reading via getState() inside the effect keeps the dep array honest
  // (the only real dep is the auth status). Earlier versions had a 1s setTimeout
  // band-aid to outrun intermediate auth states; gating on LOGGED_IN replaces it.
  const authStatus = authentication.status.value;
  useEffect(() => {
    if (authStatus !== AUTH_STATUS.LOGGED_IN) return;

    void useCreditsState.getState().fetchFromServer();
    void useSubscriptionState.getState().fetchFromServer();

    const interval = setInterval(() => {
      void useCreditsState.getState().fetchFromServer();
      console.log("TopBar: Polled credits");
    }, CREDITS_POLL_INTERVAL);

    // Imperative refresh requests, e.g. dispatched by the composers right
    // after an enqueue so the balance updates immediately.
    const handleCreditsChange = () => {
      void useCreditsState.getState().fetchFromServer();
    };
    window.addEventListener("credits-change", handleCreditsChange);

    return () => {
      clearInterval(interval);
      window.removeEventListener("credits-change", handleCreditsChange);
    };
  }, [authStatus]);

  useCreditsBalanceChangedEvent(async () => {
    useCreditsState.getState().fetchFromServer();
  });

  useSubscriptionPlanChangedEvent(async () => {
    subscriptionStore.fetchFromServer();
  });

  const switchTab = (tabId: TabId) => {
    gtagEvent("switch_tab", { tab: tabId });

    // Prevent a second input if the switcher is throttled.
    if (switcherThrottle.current) {
      return;
    }
    switcherThrottle.current = true;
    setDisableSwitcher(true);

    useTabStore.getState().setActiveTab(tabId);
    setTimeout(() => {
      // Clear the throttle
      switcherThrottle.current = false;
      // Trigger a new re-render (important)
      setDisableSwitcher(false);
    }, SWITCHER_THROTTLE_TIME);
  };

  // The titlebar reads "ARTCRAFT-X · <MODEL>" like the marketing site's app
  // window. Hooks must run unconditionally, so every page's selection is read
  // and the active tab picks one; modality label is the fallback (audio has no
  // model-selector page).
  const imageModel = useSelectedModel(ModelPage.TextToImage);
  const videoModel = useSelectedModel(ModelPage.ImageToVideo);
  const worldModel = useSelectedModel(ModelPage.ImageTo3DWorld);
  const objectModel = useSelectedModel(ModelPage.ImageTo3DObject);

  const modelForTab: Partial<Record<TabId, string | undefined>> = {
    IMAGE: imageModel?.fullName,
    VIDEO: videoModel?.fullName,
    IMAGE_TO_3D_WORLD: worldModel?.fullName,
    IMAGE_TO_3D_OBJECT: objectModel?.fullName,
  };
  const activeDescriptor = APP_DESCRIPTORS.find(
    (d) => d.id === tabStore.activeTabId,
  );
  const titleText = `ArtCraft-X · ${
    tabStore.activeTabId === "SETTINGS"
      ? "Settings"
      : (modelForTab[tabStore.activeTabId] ??
        activeDescriptor?.label ??
        "Studio")
  }`;

  return (
    <>
      <header
        className="ax-titlebar fixed left-0 top-0 z-[60] flex h-10 w-full items-center"
        data-tauri-drag-region
      >
        {/* Modality nav (macOS traffic lights need the left inset). */}
        <nav
          className={twMerge(
            "flex h-full items-center gap-5 ps-4",
            platform === "macos" && "ms-16",
          )}
          aria-label="navigation"
          data-tauri-drag-region
        >
          {APP_DESCRIPTORS.map((tab) => (
            <button
              key={tab.id}
              type="button"
              disabled={disableSwitcher}
              aria-current={
                tabStore.activeTabId === tab.id ? "page" : undefined
              }
              className={twMerge(
                "ax-navlink ax-spec no-drag transition-colors",
                tabStore.activeTabId === tab.id
                  ? "text-bone"
                  : "text-ash hover:text-putty",
              )}
              onClick={() => switchTab(tab.id as TabId)}
            >
              {tab.label}
            </button>
          ))}
        </nav>

        {/* Centered window title, marketing-site style. */}
        <span
          className="pointer-events-none absolute left-1/2 hidden -translate-x-1/2 whitespace-nowrap font-mono text-[11px] uppercase tracking-[0.16em] text-mud md:block"
          data-tauri-drag-region
        >
          {titleText}
        </span>

        <div className="ms-auto flex h-full items-center" data-tauri-drag-region>
          <div className="no-drag flex items-center gap-1 pe-2">
            <TaskQueue />

            <Tooltip content="Settings" position="bottom" delay={300}>
              <button
                type="button"
                className={twMerge(
                  "grid size-7 place-items-center rounded-ax-sm transition-colors hover:bg-bone/5 hover:text-bone",
                  tabStore.activeTabId === "SETTINGS"
                    ? "bg-bone/5 text-bone"
                    : "text-ash",
                )}
                onClick={() => {
                  gtagEvent("open_settings_page");
                  switchTab("SETTINGS");
                }}
              >
                <Settings className="h-3.5 w-3.5" />
              </button>
            </Tooltip>
          </div>

          {isDesktop && platform !== "macos" && (
            <div className="no-drag flex h-full items-center">
              <button
                type="button"
                className="grid h-full w-11 place-items-center text-ash transition-colors hover:bg-bone/5 hover:text-bone"
                onClick={minimize}
              >
                <Minus className="h-3.5 w-3.5" />
              </button>
              <button
                type="button"
                className="grid h-full w-11 place-items-center text-ash transition-colors hover:bg-bone/5 hover:text-bone"
                onClick={toggleMaximize}
              >
                {isMaximized ? (
                  <Copy className="h-3 w-3" />
                ) : (
                  <Square className="h-3 w-3" />
                )}
              </button>
              <button
                type="button"
                className="grid h-full w-11 place-items-center text-ash transition-colors hover:bg-[#c05a4a]/20 hover:text-bone"
                onClick={close}
              >
                <X className="h-3.5 w-3.5" />
              </button>
            </div>
          )}
        </div>
      </header>

      <ProviderSetupModal />
      <ProviderBillingModal isVideoPage={tabStore.activeTabId === "VIDEO"} />
      <CreditsModal />
      <CostBreakdownModal activeTabId={tabStore.activeTabId} />
    </>
  );
};
