import { render, screen } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import App from "./app";

// Import only the controls the editor uses: the Addons barrel also initializes
// an unrelated Lottie canvas renderer that is unavailable in jsdom.
vi.mock("three/examples/jsm/Addons.js", async () => ({
  ...(await import("three/examples/jsm/controls/OrbitControls.js")),
  ...(await import("three/examples/jsm/controls/PointerLockControls.js")),
  ...(await import("three/examples/jsm/animation/MMDAnimationHelper.js")),
  ...(await import("three/examples/jsm/objects/Water.js")),
}));

vi.mock("../lib/session", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/session")>()),
  useSession: () => ({ authChecked: true, loggedIn: false, user: null }),
}));

beforeEach(() => {
  vi.stubGlobal("matchMedia", (media: string) => ({
    media,
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
});

afterEach(() => vi.unstubAllGlobals());

describe("App", () => {
  it("renders the public creation routes in the sidebar", () => {
    render(<BrowserRouter><App /></BrowserRouter>);

    expect(screen.getAllByRole("link").some(
      (link) => link.getAttribute("href") === "/create-image",
    )).toBe(true);
    expect(screen.getAllByRole("link").some(
      (link) => link.getAttribute("href") === "/create-video",
    )).toBe(true);
  });

  it("renders the home greeting", () => {
    render(<BrowserRouter><App /></BrowserRouter>);

    expect(screen.getByRole("heading", { name: "What will you craft today?" })).toBeTruthy();
  });
});
