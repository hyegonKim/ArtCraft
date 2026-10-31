"use client";

import { useEffect } from "react";
import Script from "next/script";
import { GA_MEASUREMENT_ID, trackClick } from "@/lib/analytics";

// Loads gtag.js after hydration and installs the one delegated click
// listener behind the site's link and data-track events (lib/analytics.ts).
// Renders nothing when no measurement id is configured.
export default function Analytics() {
  useEffect(() => {
    if (!GA_MEASUREMENT_ID) return;
    // Capture phase: runs before click-to-play facades swap themselves out,
    // and before any handler can stop propagation. auxclick covers
    // middle-click "open in new tab".
    const onClick = (event: MouseEvent) => {
      if (event.type === "auxclick" && event.button !== 1) return;
      trackClick(event.target);
    };
    document.addEventListener("click", onClick, true);
    document.addEventListener("auxclick", onClick, true);
    return () => {
      document.removeEventListener("click", onClick, true);
      document.removeEventListener("auxclick", onClick, true);
    };
  }, []);

  if (!GA_MEASUREMENT_ID) return null;

  return (
    <>
      <Script
        src={`https://www.googletagmanager.com/gtag/js?id=${GA_MEASUREMENT_ID}`}
        strategy="afterInteractive"
      />
      <Script id="ga-init" strategy="afterInteractive">
        {`window.dataLayer=window.dataLayer||[];window.gtag=function(){dataLayer.push(arguments);};gtag("js",new Date());gtag("config",${JSON.stringify(GA_MEASUREMENT_ID)});`}
      </Script>
    </>
  );
}
