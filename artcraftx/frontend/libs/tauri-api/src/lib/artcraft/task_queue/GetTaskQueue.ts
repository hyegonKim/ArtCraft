import { invoke } from "@tauri-apps/api/core";
import { CommandResult } from "../../common/CommandStatus";
import {
  TaskStatus,
  TaskType,
  TaskModelType,
  TaskMediaFileClass,
  GenerationProvider,
} from "@storyteller/api-enums";

interface GetTaskQueueResponse {
  tasks: TaskQueueItem[];
}

export interface TaskQueueFailureReason {
  failure_type: string;
  failure_message?: string;
}

export interface TaskQueueItem {
  id: string;
  task_status: TaskStatus;
  task_type: TaskType;
  model_type?: TaskModelType;
  provider?: GenerationProvider;
  provider_job_id?: string;
  // Whether the job produces more than one file.
  is_batch_generation: boolean;
  completed_item?: TaskQueueCompletedItem;
  failure_reason?: TaskQueueFailureReason;
  created_at: Date;
  updated_at: Date;
  completed_at?: Date;
}

export interface TaskQueueCompletedItem {
  // Present only when the result was uploaded to ArtCraft (logged-in
  // session). Local-only completions carry download paths but no cloud
  // media file.
  primary_media_file?: MediaFileData;
  media_file_class?: TaskMediaFileClass;
  maybe_batch_token?: string;
  // If the results were downloaded: the directory and the first (or only)
  // file, as absolute paths recorded at completion time.
  maybe_download_directory?: string;
  maybe_first_downloaded_file?: string;
  // Thumbnails of the first downloaded file once the backend's thumbnail
  // worker has produced them (absolute cache paths; render via
  // `convertFileSrc`). Until then, listen for `local_thumbnail_ready_event`.
  maybe_local_thumbnail_path?: string;
  maybe_local_animated_preview_path?: string;
}

export interface MediaFileData {
  token: string;
  cdn_url: string;
  maybe_thumbnail_url_template?: string;
  created_at: Date;
}

interface GetTaskQueueSuccess extends CommandResult {
  payload: GetTaskQueueResponse;
}

export const GetTaskQueue = async (): Promise<GetTaskQueueResponse> => {
  const result = (await invoke(
    "get_task_queue_command"
  )) as GetTaskQueueSuccess;

  const tasks = result?.payload?.tasks || [];

  // Convert timestamps to Date objects
  const newTasks: TaskQueueItem[] = tasks.map((task) => {
    const completed_item = task.completed_item;

    if (completed_item?.primary_media_file) {
      completed_item.primary_media_file.created_at = new Date(
        completed_item.primary_media_file.created_at
      );
    }

    return {
      id: task.id,
      task_status: task.task_status,
      task_type: task.task_type,
      model_type: task.model_type,
      provider: task.provider,
      provider_job_id: task.provider_job_id,
      is_batch_generation: task.is_batch_generation,
      completed_item: completed_item,
      failure_reason: task.failure_reason,
      created_at: new Date(task.created_at),
      updated_at: new Date(task.updated_at),
      completed_at: task.completed_at ? new Date(task.completed_at) : undefined,
    };
  });

  return {
    tasks: newTasks,
  };
};

// Temporary just for testing - expose on window in a typed way
declare global {
  interface Window {
    test_get_task_queue?: typeof GetTaskQueue;
  }
}
window.test_get_task_queue = GetTaskQueue;
