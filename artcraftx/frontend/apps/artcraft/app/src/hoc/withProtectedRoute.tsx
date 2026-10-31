import { ComponentType } from "react";
import { useSignals, useSignalEffect } from "@preact/signals-react/runtime";
import { persistLogin } from "~/signals";

// NB: This no longer gates rendering on authentication — sessions are managed
// by the Tauri side. It only kicks off the session probe that populates the
// authentication signals (credits, subscriptions, etc.).
export const withProtectionRoute = <P extends object>(
  Component: ComponentType<P>,
) =>
  function ProtectionRoute(rest: P) {
    useSignals();
    useSignalEffect(() => {
      persistLogin();
    });

    return <Component {...rest} />;
  };
