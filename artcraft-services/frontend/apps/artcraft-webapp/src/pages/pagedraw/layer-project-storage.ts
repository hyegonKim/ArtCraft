import { MediaFilesApi, ProjectsApi, Visibility } from '@storyteller/api';
import { validateLayerProject, type LayerProject } from '@storyteller/ui-pagedraw';

export function createLayerProjectStorage() {
  const projects = new ProjectsApi();
  const files = new MediaFilesApi();
  return {
    async save(document: LayerProject, name: string, token?: string) {
      validateLayerProject(document);
      const args = { projectType: 'editor_2d' as const,
        blob: new Blob([JSON.stringify(document)], { type: 'application/json' }),
        fileName: 'project.editor.json', uuid: crypto.randomUUID(), maybe_title: name };
      if (!token) {
        const response = await projects.UploadNewProject({ ...args, maybe_visibility: Visibility.Private });
        if (!response.success || !response.data) throw new Error(response.errorMessage || 'Could not save the project.');
        return response.data;
      }
      const response = await projects.UpdateProject({ ...args, token });
      if (!response.success) throw new Error(response.errorMessage || 'Could not update the project.');
      const renamed = await files.RenameMediaFileByToken({ mediaToken: token, name });
      if (!renamed.success) throw new Error(renamed.errorMessage || 'Saved layers, but could not update the name. Retry Save.');
      return token;
    },
    async list(cursor?: string) {
      const response = await projects.ListSessionProjects({ filter_project_type: 'editor_2d', page_size: 30, cursor });
      if (!response.success) throw new Error(response.errorMessage || 'Could not load projects.');
      return { rows: response.data ?? [], next: response.pagination?.maybe_next ?? undefined };
    },
    async load(token: string) {
      const response = await files.GetMediaFileByToken({ mediaFileToken: token });
      const file = response.success ? response.data : undefined;
      if (!file?.media_links?.cdn_url) throw new Error('This 2D project is unavailable.');
      const bytes = await fetch(file.media_links.cdn_url, { cache: 'no-store' });
      if (!bytes.ok) throw new Error('Could not read the saved project.');
      return { document: validateLayerProject(await bytes.json()), name: file.maybe_title || 'Untitled image project' };
    },
  };
}
