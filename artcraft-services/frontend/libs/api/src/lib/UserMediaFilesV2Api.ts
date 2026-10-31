import { ApiManager, type ApiResponse } from "./ApiManager.js";
import type { MediaFile } from "./models/MediaFile.js";
import type { FilterEngineCategories, FilterMediaClasses, FilterMediaType } from "./enums/QueryFilters.js";

export interface ListUserMediaFilesV2Query {
  username: string;
  sort_ascending?: boolean;
  page_size?: number;
  page_index?: number;
  filter_media_classes?: FilterMediaClasses[];
  filter_media_type?: FilterMediaType[];
  filter_engine_categories?: FilterEngineCategories[];
  include_user_uploads?: boolean;
}

export interface UserMediaFilesV2Pagination {
  current: number;
  has_more: boolean;
}

/**
 * Faster /v1/media_files/list_v2 pages: fetches one extra row for has_more,
 * avoiding the full-library count required by the original list endpoint.
 */
export class UserMediaFilesV2Api extends ApiManager {
  public async ListUserMediaFiles(
    query: ListUserMediaFilesV2Query,
  ): Promise<ApiResponse<MediaFile[], UserMediaFilesV2Pagination>> {
    const endpoint = `${this.getApiSchemeAndHost()}/v1/media_files/list_v2/user/${encodeURIComponent(query.username)}`;
    return this.get<{
      success: boolean;
      results: MediaFile[];
      pagination: UserMediaFilesV2Pagination;
    }>({
      endpoint,
      query: {
        sort_ascending: query.sort_ascending,
        page_size: query.page_size,
        page_index: query.page_index,
        include_user_uploads: query.include_user_uploads,
        filter_media_classes: query.filter_media_classes?.join(","),
        filter_media_type: query.filter_media_type?.join(","),
        filter_engine_categories: query.filter_engine_categories?.join(","),
      },
    })
      .then((response) => ({
        success: response.success,
        data: response.results ?? [],
        pagination: response.pagination,
      }))
      .catch((err) => ({ success: false, errorMessage: err.message }));
  }
}
