// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiManager as SharedApiManager } from "./ApiManager";

const { transport, hostStore } = vi.hoisted(() => ({
  transport: vi.fn(),
  hostStore: { getInstance: () => ({ getApiSchemeAndHost: () => "https://fixture.invalid" }) },
}));
vi.mock("@storyteller/tauri-utils", () => ({ FetchProxy: transport }));
vi.mock("./config/StorytellerApiHostStore.js", () => ({ StorytellerApiHostStore: hostStore }));

class SharedManager extends SharedApiManager {
  query(params: Record<string, string | string[] | boolean | number | undefined>) {
    return this.parseQueryValues(params);
  }
  body(params: Record<string, unknown>) {
    return this.parseBodyValues<Record<string, unknown>, Record<string, unknown>>(params);
  }
}

beforeEach(() => {
  localStorage.clear();
  transport.mockReset();
  transport.mockImplementation(async () => new Response(JSON.stringify({ success: true })));
  vi.stubGlobal("fetch", transport);
});
afterEach(() => vi.unstubAllGlobals());

describe.each([
  ["shared library", () => new SharedApiManager()],
] as const)("%s transport values", (_name, create) => {
  it("retains explicit zero, false and empty strings while omitting absent query values", async () => {
    await create().fetch("https://fixture.invalid/items", {
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
    await create().fetch("https://fixture.invalid/items", { method: "POST", body });
    expect(transport.mock.calls[0][1].body).toBe(JSON.stringify(body));
  });
});

describe.each([["shared library", () => new SharedManager()]] as const)("%s parameter helpers", (_name, create) => {
  it("keeps zero, false and explicit empty text but omits empty query-filter arrays", () => {
    expect(create().query({
      pageIndex: 0, sortAscending: false, searchTerm: "", filterTypes: [],
      scopedWeightType: ["first", "second"], omitted: undefined, nullable: null as never,
    })).toEqual({ page_index: "0", sort_ascending: "false", search_term: "", scoped_weight_type: "first,second" });
  });

  it("preserves body clearing values and the existing null/undefined omission policy", () => {
    expect(create().body({
      descriptionMarkdown: "", count: 0, enabled: false, emptyList: [], omitted: undefined, nullable: null,
    })).toEqual({ description_markdown: "", count: 0, enabled: false, empty_list: [] });
  });
});
