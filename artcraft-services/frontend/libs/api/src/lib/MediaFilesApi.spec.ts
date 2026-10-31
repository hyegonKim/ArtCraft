// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { GalleryModalApi } from "./GalleryModalApi";
import { UserMediaFilesV2Api } from "./UserMediaFilesV2Api";
import { FilterMediaClasses } from "./enums/QueryFilters";

const { fetchProxy } = vi.hoisted(() => ({ fetchProxy: vi.fn() }));
vi.mock("@storyteller/tauri-utils", () => ({ FetchProxy: fetchProxy }));

beforeEach(() => {
  fetchProxy.mockReset();
  localStorage.clear();
});

describe("versioned user library HTTP contracts", () => {
  it("keeps existing clients on v1 with numeric numbered pagination", async () => {
    fetchProxy.mockResolvedValue(new Response(JSON.stringify({
      success: true, results: [], pagination: { current: 2, total_page_count: 3 },
    })));
    const response = await new GalleryModalApi().listUserMediaFiles({ username: "fixture_owner", page_index: 2 });
    const url = new URL(fetchProxy.mock.calls[0][0]);
    expect(url.pathname).toBe("/v1/media_files/list/user/fixture_owner");
    expect(url.searchParams.has("include_total_count")).toBe(false);
    expect(response.pagination).toEqual({ current: 2, total_page_count: 3 });
    expect(response.success).toBe(true);
  });

  it.each([true, false])("requests count-free pages from list_v2 (has_more: %s)", async (hasMore) => {
    fetchProxy.mockResolvedValue(new Response(JSON.stringify({
      success: true, results: [], pagination: { current: 1, has_more: hasMore },
    })));
    const response = await new UserMediaFilesV2Api().ListUserMediaFiles({
      username: "fixture_owner", include_user_uploads: true,
      filter_media_classes: [FilterMediaClasses.IMAGE, FilterMediaClasses.VIDEO],
      page_size: 60, page_index: 1,
    });
    const url = new URL(fetchProxy.mock.calls[0][0]);
    expect(url.pathname).toBe("/v1/media_files/list_v2/user/fixture_owner");
    expect(url.searchParams.has("include_total_count")).toBe(false);
    expect(url.searchParams.get("filter_media_classes")).toBe("image,video");
    expect(url.searchParams.get("page_size")).toBe("60");
    expect(url.searchParams.get("page_index")).toBe("1");
    expect(response.pagination).toEqual({ current: 1, has_more: hasMore });
    expect(fetchProxy.mock.calls[0][1].credentials).toBe("include");
  });
});
