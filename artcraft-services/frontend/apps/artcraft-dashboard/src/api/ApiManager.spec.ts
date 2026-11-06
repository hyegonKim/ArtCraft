// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiManager } from "./ApiManager";

const { transport } = vi.hoisted(() => ({ transport: vi.fn() }));
beforeEach(() => {
  transport.mockReset();
  transport.mockImplementation(async () => new Response(JSON.stringify({ success: true })));
  vi.stubGlobal("fetch", transport);
});
afterEach(() => vi.unstubAllGlobals());

describe("dashboard transport values", () => {
  it("retains explicit zero, false and empty strings while omitting absent query values", async () => {
    await new ApiManager().fetch("https://fixture.invalid/items", {
      method: "GET",
      query: { page_index: 0, sort_ascending: false, search_term: "", absent: undefined },
    });
    const [endpoint, options] = transport.mock.calls[0];
    const url = new URL(endpoint);
    expect(url.pathname).toBe("/items");
    expect([...url.searchParams.entries()]).toEqual([
      ["page_index", "0"], ["sort_ascending", "false"], ["search_term", ""],
    ]);
    expect(options.credentials).toBe("include");
    expect(options.body).toBeUndefined();
  });

  it.each([false, 0, ""])("preserves an explicit JSON body %s", async (body) => {
    await new ApiManager().fetch("https://fixture.invalid/items", { method: "POST", body });
    expect(transport.mock.calls[0][1].body).toBe(JSON.stringify(body));
  });
});
