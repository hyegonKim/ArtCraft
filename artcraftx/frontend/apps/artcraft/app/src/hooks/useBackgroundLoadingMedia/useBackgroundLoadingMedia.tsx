import { useSignalEffect, useSignals } from "@preact/signals-react/runtime";
import { PollUserGeneratedMovies, PollUserAudioItems } from "./utilities";

import { userMovies, userAudioItems } from "~/signals";

export const useBackgroundLoadingMedia = () => {
  useSignals();

  useSignalEffect(() => {
    // if myMovies undefined, poll for the first time
    if (!userMovies.value) {
      PollUserGeneratedMovies();
    }
  });

  useSignalEffect(() => {
    // if audioItems undefined, poll for the first time
    if (!userAudioItems.value) {
      PollUserAudioItems();
    }
  });
};
