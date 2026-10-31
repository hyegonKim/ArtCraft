import { create } from "zustand";

export type TabId =
  | "IMAGE"
  | "VIDEO"
  | "AUDIO"
  | "IMAGE_TO_3D_OBJECT"
  | "IMAGE_TO_3D_WORLD"
  | "SETTINGS";

const DEFAULT_TAB: TabId = "IMAGE";

interface TabState {
  // Current active tab
  activeTabId: TabId;
  // Tab data stored as stringified JSON
  tabData: {
    [K in TabId]?: string;
  };
  // Actions
  setActiveTab: (tabId: TabId) => Promise<boolean>;
  updateTabData: (tabId: TabId, data: unknown) => void;
  getTabData: <T>(tabId: TabId) => T | null;
  clearTabData: (tabId: TabId) => void;
}

export const useTabStore = create<TabState>((set, get) => ({
  activeTabId: DEFAULT_TAB,
  tabData: {},

  setActiveTab: async (newTabId) => {
    const currentTabId = get().activeTabId;

    // Don't do anything if we're already on this tab
    if (currentTabId === newTabId) return true;

    try {
      // Update active tab
      set({ activeTabId: newTabId });
      return true;
    } catch (error) {
      console.error("Error during tab change:", error);
      return false;
    }
  },

  updateTabData: (tabId, data) => {
    set((state) => ({
      tabData: {
        ...state.tabData,
        [tabId]: JSON.stringify(data),
      },
    }));
  },

  getTabData: <T>(tabId: TabId): T | null => {
    const state = get();
    const data = state.tabData[tabId];
    if (!data) return null;
    try {
      return JSON.parse(data) as T;
    } catch (e) {
      console.error(`Error parsing tab data for ${tabId}:`, e);
      return null;
    }
  },

  clearTabData: (tabId) => {
    set((state) => {
      const newTabData = { ...state.tabData };
      delete newTabData[tabId];
      return { tabData: newTabData };
    });
  },
}));
