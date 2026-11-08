import { useEffect, useMemo, useRef, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { captureLayerProject, restoreLayerProject, useSceneStore } from '@storyteller/ui-pagedraw';
import type { ProjectMediaFileInfo } from '@storyteller/api';
import { useSession } from '../../lib/session';
import { createLayerProjectStorage } from './layer-project-storage';
import { LayerAutosave, type LayerSaveState } from './layer-autosave';
import { hashLayerDocument, openLayerDraftStore, type LayerDraft } from './layer-draft-store';

const fields = ['drawNodes', 'inpaintLineNodes', 'baseImageInfo', 'brushColor', 'brushSize',
  'brushOpacity', 'eraserSize', 'fillColor', 'aspectRatioType', 'inpaintBrushSize', 'inpaintOperation'] as const;
const button = 'rounded border border-white/20 bg-[#171717] px-2 py-1 text-xs disabled:opacity-40';
const initialState: LayerSaveState = { dirty: false, saving: false, error: '', retrying: false, offline: false, local: false, localError: '', notice: '' };
type Recoverable = Pick<LayerDraft, 'id' | 'name' | 'token' | 'updatedAt'>;
type Pending = { token?: string; draftId?: string };
const isOnline = () => typeof navigator === 'undefined' || navigator.onLine !== false;

export function LayerProjectControls() {
  const storage = useMemo(createLayerProjectStorage, []);
  const drafts = useMemo(() => openLayerDraftStore(), []);
  const { loggedIn, user } = useSession();
  const owner = loggedIn ? user?.user_token : undefined;
  const ownerRef = useRef(owner);
  ownerRef.current = owner;
  const [query, setQuery] = useSearchParams();
  const requested = query.get('project') ?? undefined;
  const [token, setToken] = useState<string>();
  const [name, setName] = useState('Untitled image project');
  const [saveState, setSaveState] = useState<LayerSaveState>(initialState);
  const nameRef = useRef(name);
  const onSaved = useRef<(token: string, name?: string) => void>(() => {});
  const autosave = useMemo(() => new LayerAutosave({
    capture: captureLayerProject, getName: () => nameRef.current, save: storage.save,
    onSaved: (saved, renamed) => onSaved.current(saved, renamed), onState: setSaveState,
    drafts, hash: hashLayerDocument, serverHash: storage.serverHash, online: isOnline,
  }), [storage, drafts]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [showProjects, setShowProjects] = useState(false);
  const [pendingDiscard, setPendingDiscard] = useState<Pending | null>(null);
  const [showLayers, setShowLayers] = useState(false);
  const [rows, setRows] = useState<ProjectMediaFileInfo[]>([]);
  const [next, setNext] = useState<string>();
  const [listing, setListing] = useState(false);
  const [recoverable, setRecoverable] = useState<Recoverable[]>([]);
  const [confirmDelete, setConfirmDelete] = useState<string>();
  const [recoveredAt, setRecoveredAt] = useState<number>();
  const loading = useRef(false);
  const opened = useRef<string | undefined>(undefined);
  const operation = useRef(0);
  const recoveredOwner = useRef<string | undefined>(undefined);
  useEffect(() => () => { operation.current += 1; }, []);
  const base = useSceneStore(s => s.baseImageInfo);
  const bitmap = useSceneStore(s => s.baseImageBitmap);
  const layers = useSceneStore(s => s.drawNodes);
  const masks = useSceneStore(s => s.inpaintLineNodes);
  const dirty = !!base && saveState.dirty;
  const saving = saveState.saving;
  onSaved.current = (saved, renamed) => {
    opened.current = saved; setToken(saved);
    if (renamed) { nameRef.current = renamed; setName(renamed); }
    setQuery({ project: saved }, { replace: true });
  };
  useEffect(() => { autosave.activate(); return () => autosave.dispose(); }, [autosave]);
  useEffect(() => { autosave.setOwner(owner); }, [autosave, owner]);
  useEffect(() => {
    autosave.setEnabled(loggedIn && !!base && (!!bitmap || !!base.isBlankCanvas) && !busy && !pendingDiscard &&
      (!requested || requested === opened.current));
  }, [autosave, loggedIn, base, bitmap, busy, pendingDiscard, requested]);
  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;
  const keptLocally = useRef(saveState.local);
  keptLocally.current = saveState.local;

  useEffect(() => useSceneStore.subscribe((state, previous) => {
    if (!loading.current && fields.some(field => state[field] !== previous[field])) autosave.markDirty();
  }), [autosave]);
  useEffect(() => {
    // Edits already kept on this device survive closing the tab, so only warn about the rest.
    const beforeUnload = (event: BeforeUnloadEvent) => {
      if (dirtyRef.current && !keptLocally.current) { event.preventDefault(); event.returnValue = ''; }
    };
    const persist = () => { void autosave.persistNow(); };
    const visibility = () => {
      if (document.visibilityState === 'hidden') persist();
      else void refreshRecoverable();
    };
    const online = () => autosave.setOnline(true);
    const offline = () => autosave.setOnline(false);
    window.addEventListener('beforeunload', beforeUnload);
    window.addEventListener('pagehide', persist);
    window.addEventListener('online', online);
    window.addEventListener('offline', offline);
    document.addEventListener('visibilitychange', visibility);
    autosave.setOnline(isOnline());
    return () => {
      window.removeEventListener('beforeunload', beforeUnload);
      window.removeEventListener('pagehide', persist);
      window.removeEventListener('online', online);
      window.removeEventListener('offline', offline);
      document.removeEventListener('visibilitychange', visibility);
    };
    // refreshRecoverable reads refs only.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autosave]);

  /** Device copies of this account that no tab is editing right now. */
  async function availableDrafts(): Promise<LayerDraft[]> {
    const account = ownerRef.current;
    if (!account) return [];
    try {
      const [stored, held] = await Promise.all([drafts.list(account), drafts.held()]);
      return stored.filter(row => row.id !== autosave.currentDraftId && !held.has(row.id));
    } catch { return []; }
  }
  async function refreshRecoverable() {
    const list = await availableDrafts();
    setRecoverable(list.map(({ id, name, token, updatedAt }) => ({ id, name, token, updatedAt })));
  }

  async function operate(task: (version: number) => Promise<void>, failure: string) {
    autosave.setEnabled(false);
    const version = ++operation.current;
    setBusy(true); setError(''); loading.current = true;
    try { await task(version); }
    catch (error) { if (version === operation.current) setError(error instanceof Error ? error.message : failure); }
    finally {
      if (version === operation.current) { loading.current = false; setBusy(false); void refreshRecoverable(); }
    }
  }

  /** Decodes a device copy into the editor; its edits stay unsynced until the server confirms them. */
  async function applyDraft(draft: LayerDraft, release: () => void, version: number) {
    try { await restoreLayerProject(draft.document, () => version === operation.current); }
    catch (error) { release(); throw error; }
    opened.current = draft.token; setToken(draft.token); nameRef.current = draft.name; setName(draft.name);
    autosave.adopt(draft, release);
    setShowProjects(false); setRecoveredAt(draft.updatedAt);
    setQuery(draft.token ? { project: draft.token } : {}, { replace: true });
  }
  async function claimDraft(id: string): Promise<{ draft: LayerDraft; release: () => void }> {
    const release = await drafts.claim(id);
    if (!release) throw new Error('These edits are open in another tab.');
    const draft = await drafts.get(id).catch(() => undefined);
    if (!draft || draft.owner !== ownerRef.current) { release(); throw new Error('These edits are no longer kept on this device.'); }
    return { draft, release };
  }

  async function open(projectToken: string, confirmed = false) {
    if (dirtyRef.current && !confirmed) { setPendingDiscard({ token: projectToken }); return; }
    if (autosave.state.saving) return;
    await operate(async version => {
      // Unsynced edits of this project on this device are newer than the server copy.
      const local = (await availableDrafts()).find(row => row.token === projectToken);
      if (local && version === operation.current) {
        try {
          const { draft, release } = await claimDraft(local.id);
          await applyDraft(draft, release, version);
          return;
        } catch { /* fall back to the server copy; the device copy stays listed */ }
      }
      const project = await storage.load(projectToken);
      if (version !== operation.current) return;
      await restoreLayerProject(project.document, () => version === operation.current);
      opened.current = projectToken; setToken(projectToken); nameRef.current = project.name; setName(project.name);
      autosave.reset(projectToken, { baseHash: project.hash }); setShowProjects(false); setRecoveredAt(undefined);
      setQuery({ project: projectToken }, { replace: true });
    }, 'Could not open project.');
  }
  useEffect(() => {
    if (requested && requested !== opened.current && loggedIn) void open(requested);
    // A URL/account transition triggers loading; scene edits must not reload it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [requested, loggedIn]);
  useEffect(() => {
    if (!owner || recoveredOwner.current === owner) return;
    recoveredOwner.current = owner;
    if (requested) return; // open() prefers this project's device copy and refreshes the list.
    void (async () => {
      // A new document that never reached the server has no URL; resume it in an empty editor.
      const unsaved = !useSceneStore.getState().baseImageInfo && !opened.current
        ? (await availableDrafts()).find(row => !row.token) : undefined;
      if (unsaved && !useSceneStore.getState().baseImageInfo && !dirtyRef.current) await restoreDraft(unsaved.id, true);
      else await refreshRecoverable();
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [owner]);

  async function restoreDraft(id: string, confirmed = false) {
    if (dirtyRef.current && !confirmed) { setPendingDiscard({ draftId: id }); return; }
    if (autosave.state.saving) return;
    setConfirmDelete(undefined);
    await operate(async version => {
      const { draft, release } = await claimDraft(id);
      if (version !== operation.current) { release(); return; }
      await applyDraft(draft, release, version);
    }, 'Could not restore the edits kept on this device.');
  }
  async function deleteDraft(id: string) {
    setConfirmDelete(undefined);
    try { await drafts.remove(id); } catch { setError('Could not delete the edits kept on this device.'); }
    await refreshRecoverable();
  }

  async function save(asCopy = false) {
    setError('');
    if (!autosave.state.dirty && !asCopy) autosave.markDirty();
    await autosave.flush(asCopy);
  }
  async function list(cursor?: string) {
    setListing(true); setError('');
    try {
      const result = await storage.list(cursor);
      setRows(previous => cursor ? [...previous, ...result.rows] : result.rows); setNext(result.next);
    } catch (error) { setError(error instanceof Error ? error.message : 'Could not list projects.'); }
    finally { setListing(false); }
  }
  function startNew(confirmed = false) {
    if (dirty && !confirmed) { setPendingDiscard({}); return; }
    if (autosave.state.saving) return;
    autosave.setEnabled(false); autosave.reset();
    loading.current = true;
    useSceneStore.getState().setBaseImageInfo(null);
    useSceneStore.getState().RESET();
    opened.current = undefined; setToken(undefined); setQuery({}); nameRef.current = 'Untitled image project'; setName(nameRef.current);
    setError(''); setRecoveredAt(undefined); loading.current = false;
  }
  const status = busy ? 'Working…' : !loggedIn ? 'Sign in to save projects'
    : saveState.offline ? (!dirty ? 'Offline · new edits are kept on this device'
      : saveState.local ? 'Offline · edits kept on this device · syncs when back online' : 'Offline · keeping edits on this device…')
    : saving ? 'Saving…'
    : saveState.retrying ? `Save failed · retrying in 5 seconds${saveState.local ? ' · edits kept on this device' : ''}`
    : dirty ? 'Unsaved changes · autosave pending'
    : token ? 'Saved on server · autosave on' : 'Create a canvas or open an image';
  const alert = [error || saveState.error, saveState.localError].filter(Boolean).join(' · ');
  const first = recoverable[0];
  return <>
    <section aria-label="2D cloud project" className="pointer-events-auto absolute left-3 top-3 z-40 max-w-[calc(100%-1.5rem)] md:max-w-[55%] rounded bg-[#101010]/95 p-2 text-white">
      <div className="flex flex-wrap items-center gap-1">
        <input aria-label="2D project name" maxLength={120} value={name} disabled={busy}
          className="w-36 rounded border border-white/20 bg-transparent px-2 py-1 text-xs"
          onChange={event => { nameRef.current = event.target.value; setName(event.target.value); autosave.markDirty(); }} />
        <button className={button} disabled={busy || saving || !loggedIn || !base} onClick={() => void save()}>Save project</button>
        <button className={button} disabled={busy || saving || !loggedIn || !base} onClick={() => void save(true)}>Save copy</button>
        <button className={button} disabled={busy || saving || !loggedIn} onClick={() => { setShowProjects(true); void list(); }}>Open project</button>
        <button className={button} disabled={busy || saving} onClick={() => startNew()}>New</button>
        <button className={button} aria-expanded={showLayers} onClick={() => setShowLayers(v => !v)}>Layers ({layers.length})</button>
      </div>
      <p role="status" className="mt-1 text-xs text-white/60">{status}</p>
      {recoveredAt !== undefined && <p className="mt-1 text-xs text-emerald-300">Restored edits kept on this device from {new Date(recoveredAt).toLocaleString()}.</p>}
      {saveState.notice && <p role="note" className="mt-1 max-w-md text-xs text-amber-200">{saveState.notice}{' '}
        <button className="underline" onClick={() => autosave.dismissNotice()}>Dismiss</button></p>}
      {alert && <p role="alert" className="mt-1 max-w-md text-xs text-red-300">{alert}</p>}
      {loggedIn && first && !busy && <div aria-label="Edits kept on this device" className="mt-2 max-w-md rounded border border-amber-300/40 p-2 text-xs">
        <p>Unsynced edits on this device: “{first.name}”{first.token ? '' : ' (never saved to the server)'} · {new Date(first.updatedAt).toLocaleString()}
          {recoverable.length > 1 ? ` · ${recoverable.length - 1} more` : ''}</p>
        <div className="mt-1 flex gap-1">{confirmDelete === first.id ? <>
          <button className={button} onClick={() => void deleteDraft(first.id)}>Delete permanently</button>
          <button className={button} onClick={() => setConfirmDelete(undefined)}>Keep them</button>
        </> : <>
          <button className={button} disabled={saving} onClick={() => void restoreDraft(first.id)}>Restore edits</button>
          <button className={button} onClick={() => setConfirmDelete(first.id)}>Discard edits</button>
        </>}</div>
      </div>}
      {showLayers && <div aria-label="Project layers" className="mt-2 max-h-60 overflow-y-auto text-xs">
        <p>Canvas {base?.blankCanvasWidth ?? bitmap?.naturalWidth} × {base?.blankCanvasHeight ?? bitmap?.naturalHeight} · {masks.length} mask strokes</p>
        <ol aria-label="Layer order (top first)">{[...layers].reverse().map((layer, index) => <li key={layer.id}>
          <button className="w-full border-b border-white/10 py-1 text-left" onClick={() => useSceneStore.getState().selectNode(layer.id)}>
            {layers.length - index}. {layer.type === 'line' ? (layer.globalCompositeOperation === 'destination-out' ? 'Eraser stroke' : 'Brush stroke') : layer.type}
            {' · X '}{layer.x ?? 0}{' · Y '}{layer.y ?? 0}{layer.locked ? ' · Locked' : ''}
          </button>
        </li>)}</ol>
      </div>}
    </section>
    {showProjects && <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/70" onKeyDown={event => { if (event.key === 'Escape' && !busy) setShowProjects(false); }}>
      <section role="dialog" aria-modal="true" aria-labelledby="layer-project-heading" className="max-h-[80%] w-[min(90%,32rem)] overflow-y-auto rounded border border-white/20 bg-[#171717] p-5 text-white">
        <h2 id="layer-project-heading" className="mb-3 text-lg">Open 2D layer project</h2>
        {error && <p role="alert" className="mb-2 text-sm text-red-300">{error}</p>}
        {!listing && !rows.length && !error && <p>No saved 2D projects yet.</p>}
        {rows.map(row => <button key={row.token} className="mb-2 block w-full rounded border border-white/20 p-3 text-left" disabled={busy} onClick={() => void open(row.token)}>
          {row.maybe_title || 'Untitled image project'}
          <span className="block text-xs text-white/50">{new Date(row.updated_at).toLocaleString()}</span>
        </button>)}
        {listing && <p role="status">Loading projects…</p>}
        <div className="mt-3 flex gap-2">
          <button className={button} disabled={listing || busy} onClick={() => void list()}>Refresh projects</button>
          {next && <button className={button} disabled={listing || busy} onClick={() => void list(next)}>Load more projects</button>}
          <button className={button} disabled={busy} onClick={() => setShowProjects(false)}>Close projects</button>
        </div>
      </section>
    </div>}
    {pendingDiscard && <div className="absolute inset-0 z-[60] flex items-center justify-center bg-black/80">
      <section role="alertdialog" aria-modal="true" aria-labelledby="discard-layer-title" aria-describedby="discard-layer-description" className="w-[min(90%,28rem)] rounded border border-white/20 bg-[#171717] p-5 text-white">
        <h2 id="discard-layer-title">Unsaved layer edits</h2>
        <p id="discard-layer-description" className="my-3 text-sm text-white/70">Discard unsaved changes and {pendingDiscard.token ? 'open another project' : pendingDiscard.draftId ? 'restore other edits from this device' : 'start a new project'}?</p>
        <div className="flex gap-2">
          <button autoFocus className={button} onClick={() => setPendingDiscard(null)}>Keep editing</button>
          <button className={button} onClick={() => {
            const target = pendingDiscard; setPendingDiscard(null);
            void autosave.discardLocal();
            if (target.token) void open(target.token, true);
            else if (target.draftId) void restoreDraft(target.draftId, true);
            else startNew(true);
          }}>Discard changes</button>
        </div>
      </section>
    </div>}
    {busy && <div className="absolute inset-0 z-30 cursor-wait bg-black/10" aria-label="Project operation in progress" />}
  </>;
}
