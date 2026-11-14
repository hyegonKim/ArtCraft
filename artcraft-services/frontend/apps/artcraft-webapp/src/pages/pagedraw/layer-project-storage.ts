import { MediaFilesApi, ProjectsApi, Visibility } from '@storyteller/api';
import { validateLayerProject, type LayerProject } from '@storyteller/ui-pagedraw';
import { LayerProjectUnavailableError } from './layer-autosave';
import { hashLayerDocument } from './layer-draft-store';

export function createLayerProjectStorage() {
  const projects = new ProjectsApi();
  const files = new MediaFilesApi();
  const names = new Map<string, string>();
  return {
    /** `json`, when given, is `document` already serialized (the bytes autosave hashed). */
    async save(document: LayerProject, name: string, token?: string, requestId: string = crypto.randomUUID(), json?: string) {
      validateLayerProject(document);
      const args = { projectType: 'editor_2d' as const,
        blob: new Blob([json ?? JSON.stringify(document)], { type: 'application/json' }),
        fileName: 'project.editor.json', uuid: requestId, maybe_title: name };
      if (!token) {
        const response = await projects.UploadNewProject({ ...args, maybe_visibility: Visibility.Private });
        if (!response.success || !response.data) throw new Error(response.errorMessage || 'Could not save the project.');
        names.set(response.data, name);
        return response.data;
      }
      const response = await projects.UpdateProject({ ...args, token });
      if (!response.success) throw new Error(response.errorMessage || 'Could not update the project.');
      if (names.get(token) !== name) {
        const renamed = await files.RenameMediaFileByToken({ mediaToken: token, name });
        if (!renamed.success) throw new Error(renamed.errorMessage || 'Saved layers, but could not update the name. Retry Save.');
        names.set(token, name);
      }
      return token;
    },
    async list(cursor?: string) {
      const response = await projects.ListSessionProjects({ filter_project_type: 'editor_2d', page_size: 30, cursor });
      if (!response.success) throw new Error(response.errorMessage || 'Could not load projects.');
      return { rows: response.data ?? [], next: response.pagination?.maybe_next ?? undefined };
    },
    async load(token: string) {
      const { file, text } = await read(token);
      const document = validateLayerProject(JSON.parse(text));
      const name = file.maybe_title || 'Untitled image project';
      names.set(token, name);
      return { document, name, hash: await hashLayerDocument(document) };
    },
    /** Identity of the server document, used before replaying edits made during an outage. */
    async serverHash(token: string) {
      const { text } = await read(token);
      let value: unknown;
      try { value = JSON.parse(text); } catch { return `unreadable:${text.length}`; }
      return hashLayerDocument(value as LayerProject);
    },
  };

  /**
   * Only a 404 / 410 means the project is gone (LayerProjectUnavailableError, which makes a replay save
   * a separate copy). Every other failure throws a plain Error so the caller retries later.
   */
  async function read(token: string) {
    const response = await files.GetMediaFileByToken({ mediaFileToken: token });
    if (!response.success) {
      if (/status: (404|410)\b/.test(response.errorMessage ?? '')) throw new LayerProjectUnavailableError('This 2D project is unavailable.');
      throw new Error(response.errorMessage || 'Could not reach the saved project.');
    }
    const file = response.data;
    if (!file?.media_links?.cdn_url) throw new Error('The saved project is not ready to read yet.');
    const bytes = await fetch(file.media_links.cdn_url, { cache: 'no-store' });
    if (!bytes.ok) throw new Error('Could not read the saved project.');
    return { file, text: await bytes.text() };
  }
}
