import { Box, Film, Globe, Image, Music, type LucideIcon } from "lucide-react";
import { useTabStore, TabId } from "~/pages/Stores/TabState";

export type AppId =
  | "IMAGE"
  | "VIDEO"
  | "AUDIO"
  | "IMAGE_TO_3D_OBJECT"
  | "IMAGE_TO_3D_WORLD"
  | "SETTINGS";

export interface AppDescriptor {
  id: AppId;
  label: string;
  icon: LucideIcon;
  imageSrc?: string;
  description?: string;
  large?: boolean;
}

// The five modalities.
export const APP_DESCRIPTORS: AppDescriptor[] = [
  {
    id: "IMAGE",
    label: "Image",
    icon: Image,
  },
  {
    id: "VIDEO",
    label: "Video",
    icon: Film,
  },
  {
    id: "AUDIO",
    label: "Audio",
    icon: Music,
  },
  {
    id: "IMAGE_TO_3D_OBJECT",
    label: "Meshes",
    icon: Box,
  },
  {
    id: "IMAGE_TO_3D_WORLD",
    label: "Splats",
    icon: Globe,
  },
];

export const goToApp = (action?: string) => {
  if (
    action &&
    [
      "IMAGE",
      "VIDEO",
      "AUDIO",
      "IMAGE_TO_3D_OBJECT",
      "IMAGE_TO_3D_WORLD",
      "SETTINGS",
    ].includes(action)
  ) {
    useTabStore.getState().setActiveTab(action as TabId);
  }
};
