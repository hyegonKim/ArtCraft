import { useEffect, useMemo, useRef, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { captureLayerProject, restoreLayerProject, useSceneStore } from '@storyteller/ui-pagedraw';
import type { ProjectMediaFileInfo } from '@storyteller/api';
import { useSession } from '../../lib/session';
import { createLayerProjectStorage } from './layer-project-storage';
import { LayerAutosave, type LayerSaveState } from './layer-autosave';

const fields = ['drawNodes', 'inpaintLineNodes', 'baseImageInfo', 'brushColor', 'brushSize',
  'brushOpacity', 'eraserSize', 'fillColor', 'aspectRatioType', 'inpaintBrushSize', 'inpaintOperation'] as const;
const button = 'rounded border border-white/20 bg-[#171717] px-2 py-1 text-xs disabled:opacity-40';

export function LayerProjectControls() {
  const storage = useMemo(createLayerProjectStorage, []);
  const { loggedIn } = useSession();
  const [query, setQuery] = useSearchParams();
  const requested = query.get('project') ?? undefined;
  const [token, setToken] = useState<string>();
  const [name, setName] = useState('Untitled image project');
  const [saveState, setSaveState] = useState<LayerSaveState>({ dirty: false, saving: false, error: '', retrying: false });
  const nameRef = useRef(name);
  const onSaved = useRef<(token: string) => void>(() => {});
  const autosave = useMemo(() => new LayerAutosave({
    capture: captureLayerProject, getName: () => nameRef.current, save: storage.save,
    onSaved: saved => onSaved.current(saved), onState: setSaveState,
  }), [storage]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [showProjects, setShowProjects] = useState(false);
  const [pendingDiscard, setPendingDiscard] = useState<{ token?: string } | null>(null);
  const [showLayers, setShowLayers] = useState(false);
  const [rows, setRows] = useState<ProjectMediaFileInfo[]>([]);
  const [next, setNext] = useState<string>();
  const [listing, setListing] = useState(false);
  const loading = useRef(false);
  const opened = useRef<string | undefined>(undefined);
  const operation = useRef(0);
  useEffect(() => () => { operation.current += 1; }, []);
  const base = useSceneStore(s => s.baseImageInfo);
  const bitmap = useSceneStore(s => s.baseImageBitmap);
  const layers = useSceneStore(s => s.drawNodes);
  const masks = useSceneStore(s => s.inpaintLineNodes);
  const dirty = !!base && saveState.dirty;
  const saving = saveState.saving;
  onSaved.current = saved => {
    opened.current = saved; setToken(saved); setQuery({ project: saved }, { replace: true });
  };
  useEffect(() => { autosave.activate(); return () => autosave.dispose(); }, [autosave]);
  useEffect(() => {
    autosave.setEnabled(loggedIn && !!base && (!!bitmap || !!base.isBlankCanvas) && !busy && !pendingDiscard &&
      (!requested || requested === opened.current));
  }, [autosave, loggedIn, base, bitmap, busy, pendingDiscard, requested]);
  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;

  useEffect(() => useSceneStore.subscribe((state, previous) => {
    if (!loading.current && fields.some(field => state[field] !== previous[field])) autosave.markDirty();
  }), [autosave]);
  useEffect(() => {
    const handler = (event: BeforeUnloadEvent) => {
      if (dirtyRef.current) { event.preventDefault(); event.returnValue = ''; }
    };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  }, []);

  async function open(projectToken: string, confirmed = false) {
    if (dirtyRef.current && !confirmed) { setPendingDiscard({ token: projectToken }); return; }
    if (autosave.state.saving) return;
    autosave.setEnabled(false);
    const version = ++operation.current;
    setBusy(true); setError(''); loading.current = true;
    try {
      const project = await storage.load(projectToken);
      if (version !== operation.current) return;
      await restoreLayerProject(project.document, () => version === operation.current);
      opened.current = projectToken; setToken(projectToken); nameRef.current = project.name; setName(project.name);
      autosave.reset(projectToken); setShowProjects(false);
      setQuery({ project: projectToken }, { replace: true });
    } catch (error) { if (version === operation.current) setError(error instanceof Error ? error.message : 'Could not open project.'); }
    finally { if (version === operation.current) { loading.current = false; setBusy(false); } }
  }
  useEffect(() => {
    if (requested && requested !== opened.current && loggedIn) void open(requested);
    // A URL/account transition triggers loading; scene edits must not reload it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [requested, loggedIn]);

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
    setError(''); loading.current = false;
  }
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
      <p role="status" className="mt-1 text-xs text-white/60">{busy ? 'Working…' : !loggedIn ? 'Sign in to save projects' : saving ? 'Saving…' : saveState.retrying ? 'Save failed · retrying in 5 seconds' : dirty ? 'Unsaved changes · autosave pending' : token ? 'Saved on server · autosave on' : 'Create a canvas or open an image'}</p>
      {(error || saveState.error) && <p role="alert" className="mt-1 max-w-md text-xs text-red-300">{error || saveState.error}</p>}
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
        <p id="discard-layer-description" className="my-3 text-sm text-white/70">Discard unsaved changes and {pendingDiscard.token ? 'open another project' : 'start a new project'}?</p>
        <div className="flex gap-2">
          <button autoFocus className={button} onClick={() => setPendingDiscard(null)}>Keep editing</button>
          <button className={button} onClick={() => {
            const next = pendingDiscard; setPendingDiscard(null);
            if (next.token) void open(next.token, true); else startNew(true);
          }}>Discard changes</button>
        </div>
      </section>
    </div>}
    {busy && <div className="absolute inset-0 z-30 cursor-wait bg-black/10" aria-label="Project operation in progress" />}
  </>;
}
