import { PopoverMenu, type PopoverItem } from "@storyteller/ui-popover";
import {
  useClassyModelSelectorStore,
  useSelectedProviderForModel,
} from "./classy-model-selector-store";
import { useEffect, useMemo } from "react";
import { ModelPage } from "./model-pages";
import { Provider } from "@storyteller/tauri-api";
import { getProviderDisplayName, getProviderIcon } from "./provider-icons";
import { Model } from "@storyteller/model-list";
import { ChevronUp } from "lucide-react";
import { GenerationProvider } from "@storyteller/api-enums";
import { Tooltip } from "@storyteller/ui-tooltip";
import { defaultModelForPage } from "./defaultModelForPage";

interface ClassyModelSelectorProps {
  items: Omit<PopoverItem, "selected">[];
  page: ModelPage;
  mode?: "hoverSelect" | "default" | "toggle" | "button";
  panelTitle?: string;
  buttonClassName?: string;
  panelClassName?: string;
  showIconsInList?: boolean;
  triggerLabel?: string;
  providersByModel?: Partial<Record<string, Provider[]>>;
  maxListHeight?: number | string;
  /**
   * "floating": the original standalone selector (large two-line trigger with
   * model name + provider, hover-to-open list). "embedded": a compact pill
   * (creator icon + model name) for use inside a promptbox option row; opens
   * a webapp-style rich list on click. Both share the same store wiring and
   * provider-selection rows.
   */
  variant?: "floating" | "embedded";
  /**
   * Whether each row shows the icon of the provider the model will run on
   * (its last-used provider, else its default). Defaults to true. Pages
   * locked to a single provider (e.g. PageDraw) pass false to show only the
   * model name. Changing the provider itself is the account picker's job.
   */
  showProviderSelection?: boolean;
}

// Model instances are rebuilt when the backend listing hydrates, so compare by
// tauriId rather than object identity.
// Stable empty map for pages with no provider selections yet (see the selector
// in ClassyModelSelector).
const NO_SELECTED_PROVIDERS: Readonly<Record<string, GenerationProvider>> = Object.freeze({});

const isSameModel = (a: Model | undefined, b: Model | undefined): boolean =>
  a !== undefined && b !== undefined && a.tauriId === b.tauriId;

const DEFAULT_PROVIDER_OPTIONS: GenerationProvider[] = [GenerationProvider.Artcraft];

export function ClassyModelSelector({
  items,
  page,
  providersByModel,
  maxListHeight = "60vh",
  variant = "floating",
  showProviderSelection = true,
  ...popoverProps
}: ClassyModelSelectorProps) {
  const { selectedModels, setSelectedModel, setSelectedProvider, registerPageModels } =
    useClassyModelSelectorStore();
  const itemModels: Model[] = useMemo(
    () => items.map((item) => item.model).filter((model): model is Model => model !== undefined),
    [items],
  );

  // Let the store know what this page offers, so switching provider/account
  // can swap to a model that works there.
  useEffect(() => {
    registerPageModels(page, itemModels);
  }, [itemModels, page, registerPageModels]);
  const selectedModel = selectedModels[page] || defaultModelForPage(itemModels, page);
  const selectedProvider = useSelectedProviderForModel(page, selectedModel?.id);
  const selectedProvidersByModel = useClassyModelSelectorStore(
    // NB: the fallback must be a stable reference — a fresh `{}` per render
    // makes useSyncExternalStore see a new snapshot every time and loop.
    (s) => s.selectedProviders[page] ?? NO_SELECTED_PROVIDERS,
  );

  // Make sure a model is selected for other components to listen to. The
  // lists load from the backend asynchronously, so this re-runs as items
  // arrive and picks the page default once there is something to pick.
  useEffect(() => {
    if (selectedModels[page]) return;
    const fallback = defaultModelForPage(itemModels, page);
    if (fallback) {
      setSelectedModel(page, fallback);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [items, page]);

  // The backend listing hydrates asynchronously and rebuilds the model
  // instances with API capabilities. Swap a stale selected instance for the
  // fresh one so capability-driven UI (keyframes, references, pickers)
  // reflects the API data without needing a manual re-select.
  //
  // NB: the store may hold a provider-specific variant of the model (see
  // `Model.forProvider()`), which is a different instance from the base in
  // `itemModels` on purpose. Compare against the variant's base, or this
  // would re-select the base, the store would swap back to the variant, and
  // the effect would loop until React gives up ("Maximum update depth").
  useEffect(() => {
    const selected = selectedModels[page];
    if (!selected) return;
    const fresh = itemModels.find((m) => m.tauriId === selected.tauriId);
    if (fresh !== undefined && fresh !== selected.forProvider(undefined)) {
      setSelectedModel(page, fresh);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [items, selectedModels, page]);

  // Initialize a default provider for each model so we can render icons even when not selected
  useEffect(() => {
    for (const item of items) {
      const modelId = item.model?.id;
      if (!modelId) continue;
      if (selectedProvidersByModel[modelId]) continue;
      const allowed = item.model?.getProviders() || DEFAULT_PROVIDER_OPTIONS;
      if (allowed.length > 0) {
        setSelectedProvider(page, modelId, allowed[0]);
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [items, providersByModel, page, selectedProvidersByModel]);

  const handleModelSelect = (item: PopoverItem) => {
    console.log(`Model selector changed on page "${page}": `, item.model);
    setSelectedModel(page, item.model!);
  };

  const modelList = useMemo(
    () =>
      items.map((item) => {
        const modelId = item.model?.id;
        const allowedProviders = item.model?.getProviders() || DEFAULT_PROVIDER_OPTIONS;

        // The provider this model will run on: its last-used provider, else
        // its default. Shown as an icon; the account picker changes it.
        const rowProvider = modelId ? selectedProvidersByModel[modelId] ?? allowedProviders[0] : undefined;
        const isSelected = isSameModel(item.model as Model | undefined, selectedModel);
        // Kept quiet: no chip background, low opacity, a touch brighter on
        // hover and for the selected row.
        const providerChip = (className: string) =>
          showProviderSelection && rowProvider ? (
            <div className={`mr-1 p-1.5 transition-opacity ${className}`}>
              <span className="text-base-fg text-base">
                {getProviderIcon(rowProvider)}
              </span>
            </div>
          ) : undefined;

        return {
          ...item,
          selected: isSelected,
          trailing: !isSelected
            ? providerChip("opacity-35 group-hover:opacity-60")
            : undefined,
          selectedRight: isSelected
            ? providerChip("opacity-55 group-hover:opacity-75")
            : undefined,
        } as PopoverItem;
      }),
    [
      items,
      selectedModel,
      selectedProvidersByModel,
      showProviderSelection,
    ],
  );

  if (variant === "embedded") {
    const selectedIcon = modelList.find((i) => i.selected)?.icon;
    return (
      <Tooltip content="Model" position="top" className="z-50" closeOnClick>
        <PopoverMenu
          items={modelList}
          onSelect={handleModelSelect}
          mode="toggle"
          richList
          panelTitle="Select Model"
          panelClassName="w-[360px]"
          maxListHeight={maxListHeight}
          buttonClassName="max-w-48"
          triggerIcon={
            selectedIcon ? (
              <span className="flex h-4 w-4 shrink-0 items-center justify-center">
                {selectedIcon}
              </span>
            ) : undefined
          }
        />
      </Tooltip>
    );
  }

  return (
    <div className="flex items-center gap-3">
      <span className="text-base-fg/90 text-base font-semibold">Model</span>
      <PopoverMenu
        items={modelList}
        onSelect={handleModelSelect}
        mode="hoverSelect"
        maxListHeight={maxListHeight}
        {...popoverProps}
        buttonClassName="rounded-lg bg-bone/[0.04] hover:bg-bone/[0.08] text-left px-3 py-1 gap-3 border border-line-2"
        renderTrigger={(selectedItem) => {
          const modelTitle = selectedItem?.label ?? selectedModel?.selectorName ?? "";
          const providerIcon = selectedProvider
            ? getProviderIcon(selectedProvider)
            : null;
          return (
            <div className="flex items-center justify-between w-full gap-3">
              <div className="flex min-w-0 flex-col">
                <div className="flex items-center gap-2 min-w-0">
                  <span className="truncate text-base font-semibold text-base-fg">
                    {modelTitle}
                  </span>
                </div>
                <div className="flex items-center gap-1.5 text-base-fg/60 text-[13px] -mt-[1px]">
                  <span>via</span>
                  {providerIcon && (
                    <span className="opacity-70">{providerIcon}</span>
                  )}
                  <span className="truncate">
                    {selectedProvider
                      ? getProviderDisplayName(selectedProvider)
                      : ""}
                  </span>
                </div>
              </div>
              <ChevronUp
                size="1em"
                className="text-base text-base-fg/70 self-center"
              />
            </div>
          );
        }}
      />
    </div>
  );
}

export default ClassyModelSelector;
