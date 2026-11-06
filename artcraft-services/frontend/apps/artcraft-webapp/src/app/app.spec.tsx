import { render, screen } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import App from "./app";

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
