// Shared model store: the single source of truth for the model dropdowns.
//
// Everything comes from the Rust `models` crate via the Tauri list commands —
// there is no built-in frontend list. Until `loadModelsFromBackend()` resolves
// (called once on boot), the lists are empty and `loaded` is false. Disabled
// models are kept out of the picker lists but registered for lookups (task
// history, icons) via `registerLoadedModels`. Each model's `providers` come
// from the listing's provider offerings (a model offered by nobody has none).
// An offering's `overrides` (a provider that runs a model with different
// menus) becomes a per-provider variant on the base model; see
// `Model.forProvider()`.

import { create } from "zustand";
import {
  AudioModelListing,
  ImageModel,
  Model,
  Object3DModel,
  SplatModel,
  VideoModel,
  attachProviderVariants,
  imageModelFromListing,
  object3DModelFromListing,
  providersByModelFromOfferings,
  registerLoadedModels,
  splatModelFromListing,
  videoModelFromListing,
} from "@storyteller/model-list";
import { ListAudioModels } from "../generate/models/audio/ListAudioModels.js";
import { ListImageModels } from "../generate/models/image/ListImageModels.js";
import { ListMeshModels } from "../generate/models/mesh/ListMeshModels.js";
import { ListSplatModels } from "../generate/models/splat/ListSplatModels.js";
import { ListVideoModels } from "../generate/models/video/ListVideoModels.js";

export interface ModelsStoreState {
  imageModels: ImageModel[];
  videoModels: VideoModel[];
  splatModels: SplatModel[];
  object3DModels: Object3DModel[];
  // Audio has no frontend class yet; the raw listing is exposed as-is.
  audioModels: AudioModelListing[];
  // True once the backend has been loaded at least once.
  loaded: boolean;
  isLoading: boolean;
  loadModelsFromBackend: () => Promise<void>;
}

export const useModelsStore = create<ModelsStoreState>((set, get) => ({
  imageModels: [],
  videoModels: [],
  splatModels: [],
  object3DModels: [],
  audioModels: [],
  loaded: false,
  isLoading: false,
  loadModelsFromBackend: async () => {
    if (get().isLoading) return;
    set({ isLoading: true });

    const [image, video, splat, mesh, audio] = await Promise.allSettled([
      ListImageModels(),
      ListVideoModels(),
      ListSplatModels(),
      ListMeshModels(),
      ListAudioModels(),
    ]);

    const next: Partial<ModelsStoreState> = { isLoading: false, loaded: true };
    // Everything served, disabled included, for lookups by id.
    const allModels: Model[] = [];

    if (image.status === "fulfilled") {
      const providers = providersByModelFromOfferings(image.value.payload.providers);
      const built = image.value.payload.models.map((m) => imageModelFromListing(m, providers.get(m.model) ?? []));
      attachProviderVariants(built, image.value.payload.providers, providers, imageModelFromListing);
      allModels.push(...built);
      next.imageModels = enabledOnly(image.value.payload.models, built);
    } else {
      console.error("[models] failed to load image models:", image.reason);
    }

    if (video.status === "fulfilled") {
      const providers = providersByModelFromOfferings(video.value.payload.providers);
      const built = video.value.payload.models.map((m) => videoModelFromListing(m, providers.get(m.model) ?? []));
      attachProviderVariants(built, video.value.payload.providers, providers, videoModelFromListing);
      allModels.push(...built);
      next.videoModels = enabledOnly(video.value.payload.models, built);
    } else {
      console.error("[models] failed to load video models:", video.reason);
    }

    if (splat.status === "fulfilled") {
      const providers = providersByModelFromOfferings(splat.value.payload.providers);
      const built = splat.value.payload.models.map((m) => splatModelFromListing(m, providers.get(m.model) ?? []));
      allModels.push(...built);
      next.splatModels = enabledOnly(splat.value.payload.models, built);
    } else {
      console.error("[models] failed to load splat models:", splat.reason);
    }

    if (mesh.status === "fulfilled") {
      const providers = providersByModelFromOfferings(mesh.value.payload.providers);
      const built = mesh.value.payload.models.map((m) => object3DModelFromListing(m, providers.get(m.model) ?? []));
      allModels.push(...built);
      next.object3DModels = enabledOnly(mesh.value.payload.models, built);
    } else {
      console.error("[models] failed to load mesh models:", mesh.reason);
    }

    if (audio.status === "fulfilled") {
      next.audioModels = audio.value.payload.models.filter((m) => !m.is_disabled);
    } else {
      console.error("[models] failed to load audio models:", audio.reason);
    }

    registerLoadedModels(allModels);
    console.log(
      `[models] loaded ${allModels.length} models from the backend`,
      allModels.map((m) => m.tauriId),
    );
    set(next);
  },
}));

// Keep the picker lists to enabled models; `built[i]` corresponds to `listings[i]`.
const enabledOnly = <T>(listings: { is_disabled: boolean }[], built: T[]): T[] =>
  built.filter((_, i) => !listings[i].is_disabled);

// Hook selectors for React consumers.
export const useImageModels = (): ImageModel[] => useModelsStore((s) => s.imageModels);
export const useVideoModels = (): VideoModel[] => useModelsStore((s) => s.videoModels);
export const useSplatModels = (): SplatModel[] => useModelsStore((s) => s.splatModels);
export const useObject3DModels = (): Object3DModel[] => useModelsStore((s) => s.object3DModels);
export const useAudioModels = (): AudioModelListing[] => useModelsStore((s) => s.audioModels);
