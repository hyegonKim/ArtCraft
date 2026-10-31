// Internal to @storyteller/soundboard. Consumers go through SoundManager.
//
// Plays the user's own .wav files. The bytes come from the backend (which
// reads the path saved in preferences) and are wrapped in a blob URL for
// Howler; decoded sounds are cached per path so a file is only loaded once.
// Every failure is logged and swallowed — a missing file must never break
// the event that triggered the sound.
import { Howl } from "howler";
import { AppSoundEvent, LoadCustomSound } from "@storyteller/tauri-api";

type CachedSound = { path: string; howl: Howl };

const DEFAULT_VOLUME = 0.5;

export class CustomSoundPlayer {
  // Keyed by event; invalidated when the configured path changes.
  private static readonly cache = new Map<AppSoundEvent, CachedSound>();

  // Play the custom sound configured for `event` (whose current path is
  // `path`). Resolves once playback starts or the load failed.
  static async play(event: AppSoundEvent, path: string): Promise<void> {
    const howl = await this.load(event, path);
    howl?.play();
  }

  private static async load(event: AppSoundEvent, path: string): Promise<Howl | undefined> {
    const cached = this.cache.get(event);
    if (cached && cached.path === path) {
      return cached.howl;
    }
    if (cached) {
      cached.howl.unload();
      this.cache.delete(event);
    }

    let bytes: ArrayBuffer;
    try {
      bytes = await LoadCustomSound(event);
    } catch (err) {
      console.warn(`Custom sound for ${event} could not be loaded (${path}):`, err);
      return undefined;
    }

    const url = URL.createObjectURL(new Blob([bytes], { type: "audio/wav" }));
    const howl = new Howl({
      src: [url],
      // Blob URLs carry no extension, so tell Howler the codec.
      format: ["wav"],
      autoplay: false,
      loop: false,
      volume: DEFAULT_VOLUME,
      onloaderror: (_id, err) => {
        console.warn(`Custom sound for ${event} could not be decoded (${path}):`, err);
      },
      onplayerror: (_id, err) => {
        console.warn(`Custom sound for ${event} could not be played (${path}):`, err);
      },
    });
    this.cache.set(event, { path, howl });
    return howl;
  }
}
