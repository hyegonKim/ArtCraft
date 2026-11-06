// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiManager } from "./ApiManager";

const { transport, hostStore } = vi.hoisted(() => ({
  transport: vi.fn(),
  hostStore: { getInstance: () => ({ getApiSchemeAndHost: () => "https://fixture.invalid" }) },
}));
vi.mock("@storyteller/tauri-utils", () => ({ FetchProxy: transport }));
vi.mock("@storyteller/api", () => ({ StorytellerApiHostStore: hostStore }));

beforeEach(() => {
  localStorage.clear();
  transport.mockReset();
  transport.mockImplementation(async () => new Response(JSON.stringify({ success: true })));
  vi.stubGlobal("fetch", transport);
});
afterEach(() => vi.unstubAllGlobals());

describe("legacy app transport values", () => {
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

class TestApiManager extends ApiManager {
  query(params: Record<string, string | string[] | boolean | number | undefined>) {
    return this.parseQueryValues(params);
  }
  body(params: Record<string, unknown>) {
    return this.parseBodyValues<Record<string, unknown>, Record<string, unknown>>(params);
  }
}

describe("legacy app parameter helpers", () => {
  it("keeps zero, false and explicit empty text but omits empty query-filter arrays", () => {
    expect(new TestApiManager().query({
      pageIndex: 0, sortAscending: false, searchTerm: "", filterTypes: [],
      scopedWeightType: ["first", "second"], omitted: undefined, nullable: null as never,
    })).toEqual({ page_index: "0", sort_ascending: "false", search_term: "", scoped_weight_type: "first,second" });
  });

  it("preserves body clearing values and the existing null/undefined omission policy", () => {
    expect(new TestApiManager().body({
      descriptionMarkdown: "", count: 0, enabled: false, emptyList: [], omitted: undefined, nullable: null,
    })).toEqual({ description_markdown: "", count: 0, enabled: false, empty_list: [] });
  });
});
