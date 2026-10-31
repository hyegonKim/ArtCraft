import { useState } from "react";
import { Tooltip } from "@storyteller/ui-tooltip";
import { ArrowLeftRight, Plus } from "lucide-react";

import { twMerge } from "tailwind-merge";
import { DeckAddAction, DeckItem } from "./deckTypes";
import {
  DeckAddMenu,
  DeckCard,
  DeckPreviewModal,
  DeckStyles,
} from "./DeckCard";

interface KeyframeCardsProps {
  firstFrame?: DeckItem;
  lastFrame?: DeckItem;
  /** Whether the model supports an ending keyframe. */
  showLastFrame: boolean;
  onFirstAddActions: DeckAddAction[];
  onLastAddActions?: DeckAddAction[];
  onRemoveFirst: () => void;
  onRemoveLast?: () => void;
  /** Exchange first/last frames; only shown when both are set. */
  onSwap?: () => void;
  className?: string;
}

/**
 * Dreamina-style keyframe widget: two slightly tilted "First frame" /
 * "Last frame" card slots with a swap glyph between them, sitting left of
 * the prompt text inside the promptbox.
 */
export const KeyframeCards = ({
  firstFrame,
  lastFrame,
  showLastFrame,
  onFirstAddActions,
  onLastAddActions = [],
  onRemoveFirst,
  onRemoveLast,
  onSwap,
  className,
}: KeyframeCardsProps) => {
  const [previewItem, setPreviewItem] = useState<DeckItem | null>(null);

  return (
    <div
      className={twMerge(
        "relative flex shrink-0 items-center self-start",
        className,
      )}
    >
      <DeckStyles />

      <DeckSlotCard
        item={firstFrame}
        label="First frame"
        addActions={onFirstAddActions}
        onRemove={onRemoveFirst}
        onPreview={setPreviewItem}
      />

      {showLastFrame && (
        <>
          <div className="z-10 -mx-1.5 flex items-center justify-center">
            {firstFrame && lastFrame && onSwap ? (
              <button
                type="button"
                onClick={onSwap}
                title="Swap frames"
                className="flex h-5 w-5 items-center justify-center rounded-full border border-white/20 bg-black/60 text-[10px] text-white shadow backdrop-blur-md transition-all hover:scale-110 hover:bg-black/80"
              >
                <ArrowLeftRight className="h-2.5 w-2.5" />
              </button>
            ) : (
              <div className="pointer-events-none flex h-5 w-5 items-center justify-center rounded-full border border-white/10 bg-black/40 text-[10px] text-white/50 backdrop-blur-md">
                <ArrowLeftRight className="h-2.5 w-2.5" />
              </div>
            )}
          </div>

          <DeckSlotCard
            item={lastFrame}
            label="Last frame"
            addActions={onLastAddActions}
            onRemove={onRemoveLast}
            onPreview={setPreviewItem}
          />
        </>
      )}

      <DeckPreviewModal
        item={previewItem}
        onClose={() => setPreviewItem(null)}
      />
    </div>
  );
};

/**
 * One always-visible labeled reference slot: a dashed add card (with a
 * per-slot add menu when there are multiple actions) that becomes a DeckCard
 * once filled. Used for keyframes and for fixed named slots like the 3D
 * multi-view angles; the caller owns the DeckPreviewModal fed by onPreview.
 */
export const DeckSlotCard = ({
  item,
  label,
  tiltClass = "",
  addActions,
  onRemove,
  onPreview,
}: {
  item?: DeckItem;
  label: string;
  tiltClass?: string;
  addActions: DeckAddAction[];
  onRemove?: () => void;
  onPreview: (item: DeckItem) => void;
}) => {
  const enabledActions = addActions.filter((a) => !a.disabled);

  if (item) {
    return (
      <div className="flex flex-col items-center gap-0.5">
        <DeckCard
          item={item}
          onRemove={item.uploading ? undefined : onRemove}
          onClick={() => onPreview(item)}
          className={twMerge(tiltClass, "hover:z-10 hover:rotate-0")}
        />
        <span className="text-[10.5px] font-medium leading-none text-base-fg/60">
          {label}
        </span>
      </div>
    );
  }

  const emptyCard = (
    <button
      type="button"
      onClick={() => enabledActions[0]?.onSelect()}
      className={twMerge(
        "flex aspect-square w-[72px] flex-col items-center justify-center gap-0.5 rounded-lg border-2 border-dashed border-line-2 bg-bone/[0.03] text-putty transition-colors duration-300 ease-out hover:border-bone/30 hover:bg-bone/[0.08] hover:text-bone hover:z-10",
        tiltClass,
      )}
    >
      <Plus className="h-[18px] w-[18px] opacity-80" />
      <span className="px-0.5 text-center text-[10px] font-medium leading-tight opacity-70">
        {label}
      </span>
    </button>
  );

  if (enabledActions.length === 0) {
    return emptyCard;
  }

  return enabledActions.length > 1 ? (
    <Tooltip
      interactive={true}
      position="top"
      delay={100}
      className="-mb-0.5 p-1.5 text-base-fg"
      closeOnClick={true}
      content={<DeckAddMenu actions={enabledActions} />}
    >
      {emptyCard}
    </Tooltip>
  ) : (
    emptyCard
  );
};
