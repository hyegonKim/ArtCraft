import { signal } from "@preact/signals-core";

export const loadingBarIsShowing = signal(false);
export const loadingBarData = signal<{
  label: string;
  message: string;
  progress: number;
}>({
  label: "Loading Editor Engine... 🦊",
  progress: 5,
  message: "",
});
