import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { UserInfo } from '@storyteller/api';

const mocks = vi.hoisted(() => ({ getSession: vi.fn() }));
vi.mock('@storyteller/api', () => ({ UsersApi: class { GetSession = mocks.getSession; } }));
const user = { user_token: 'user_test', username: 'test' } as UserInfo;
const authenticated = { success: true, data: { loggedIn: true, user } };
const guest = { success: true, data: { loggedIn: false } };

beforeEach(() => { vi.resetModules(); vi.useFakeTimers(); mocks.getSession.mockReset(); });
afterEach(() => { vi.useRealTimers(); });

it.each([2000, 15000])('accepts a valid delayed session after %i ms, including after the UI deadline', async (delay) => {
  let resolve!: (value: unknown) => void;
  mocks.getSession.mockReturnValue(new Promise(done => { resolve = done; }));
  const session = await import('./session');
  const pending = session.refreshSession();
  await vi.advanceTimersByTimeAsync(delay);
  resolve(authenticated);
  await pending;
  await vi.advanceTimersByTimeAsync(0);
  expect(session.useSessionStore.getState()).toMatchObject({ loggedIn: true, user, authChecked: true });
});

it('preserves an established login on transport failure but clears it on a confirmed guest response', async () => {
  const session = await import('./session');
  session.useSessionStore.getState().setSession({ user, loggedIn: true, authChecked: true });
  mocks.getSession.mockRejectedValueOnce(new Error('Network unavailable')).mockResolvedValueOnce(guest);
  await session.refreshSession(true);
  expect(session.useSessionStore.getState()).toMatchObject({ loggedIn: true, user });
  await session.refreshSession(true);
  expect(session.useSessionStore.getState()).toMatchObject({ loggedIn: false, user: undefined });
});

it('ignores a late authenticated response superseded by logout', async () => {
  let resolve!: (value: unknown) => void;
  mocks.getSession.mockReturnValueOnce(new Promise(done => { resolve = done; })).mockResolvedValueOnce(guest);
  const session = await import('./session');
  const stale = session.refreshSession(true);
  session.invalidateSession();
  await session.refreshSession(true);
  resolve(authenticated);
  await stale;
  expect(session.useSessionStore.getState()).toMatchObject({ loggedIn: false, user: undefined });
});
