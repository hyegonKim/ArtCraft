import { render, screen, cleanup } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { HelmetProvider } from 'react-helmet-async';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from './app';
import { UsersApi } from '@storyteller/api';

beforeEach(() => {
  vi.spyOn(UsersApi.prototype, 'GetSession').mockResolvedValue({ success: true, data: { loggedIn: false } });
  vi.stubGlobal('matchMedia', (media: string) => ({
    media, matches: false, addListener: vi.fn(), removeListener: vi.fn(),
    addEventListener: vi.fn(), removeEventListener: vi.fn(),
  }));
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

function renderApp(route: string) {
  return render(<HelmetProvider><MemoryRouter initialEntries={[route]}><App /></MemoryRouter></HelmetProvider>);
}

describe('App', () => {
  it('renders the public navigation', async () => {
    renderApp('/login');
    expect((await screen.findAllByRole('link', { name: 'Login' })).length).toBeGreaterThan(0);
  });

  it('renders the current login page heading', () => {
    renderApp('/login');
    expect(screen.getByRole('heading', { name: 'Welcome back' })).toBeTruthy();
  });
});
