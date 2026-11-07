import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { VideoEditorProjectsLanding } from './VideoEditorProjectsLanding';
import type { WebappProjectStorage } from './adapters/project-storage-adapter';

vi.mock('../../components/topbar/TopBarActions', () => ({ TopBarActions: () => null }));
vi.mock('@storyteller/ui-gallery-modal', () => ({ FolderNameDialog: () => null }));
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it('allows a new local edit when saved projects cannot be fetched', async () => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  const listProjects = vi.fn().mockRejectedValue(new Error('Offline'));
  const onNewProject = vi.fn();
  render(<VideoEditorProjectsLanding projectStorage={{ listProjects } as unknown as WebappProjectStorage} onNewProject={onNewProject} onOpenProject={vi.fn()} />);
  await screen.findByText("Couldn't load your projects");
  fireEvent.click(screen.getByRole('button', { name: 'New project' }));
  expect(onNewProject).toHaveBeenCalledTimes(1);
  expect(screen.getByText(/export your work before leaving/)).toBeTruthy();
  fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
  await screen.findByText("Couldn't load your projects");
  expect(listProjects).toHaveBeenCalledTimes(2);
});
