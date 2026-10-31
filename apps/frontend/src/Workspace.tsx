import { useEffect, useRef, useState } from 'react';
import type { CSSProperties, KeyboardEvent, PointerEvent, ReactNode } from 'react';
import {
  ArrowUp,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronUp,
  Copy,
  Download,
  Expand,
  FileText,
  Image as ImageIcon,
  LoaderCircle,
  MessageCircle,
  Palette,
  PanelLeft,
  PanelLeftClose,
  Pencil,
  Plus,
  RefreshCw,
  RotateCcw,
  RotateCw,
  SlidersHorizontal,
  Square,
  X,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
import { listen } from '@tauri-apps/api/event';
import {
  api,
  assetUrl,
  DraftSaveQueue,
  native,
  stateLabel,
  type AssetView,
  type ConversationView,
  type StoredDraft,
  type StudioSnapshot,
  type TurnDetail,
  type TurnPage,
  type TurnView,
} from './studio-api';
import {
  clampZoom,
  confirmedDraft,
  createDraft,
  DEFAULT_MODEL_ID,
  initialWorkspace,
  validSeed,
  workspaceReducer,
} from './workspace-state';
import type { Draft, DraftParams, WorkspaceAction, WorkspaceState } from './workspace-state';
import {
  claimOneShotNonce,
  composerActionFor,
  createSubmissionGate,
  isPrimaryActivation,
  mergeTurns,
  progressForTurn,
  shouldSendOnEnter,
  supportedDraftParams,
} from './conversation-state';
import './workspace.css';

export const samples = [
  { title: '山湖晨雾', tag: '风景', src: '/examples/landscape.png', prompt: '清晨的山间湖泊，薄雾从水面缓缓升起，远山与松林倒映在平静湖面，温柔的自然光，电影摄影，细腻层次，横版构图。' },
  { title: '静物光影', tag: '静物', src: '/examples/still-life.png', prompt: '手工奶油色陶瓷花瓶，一枝细长的绿叶与两只青梨，晨光从窗边照在石质桌面和浅色墙面，柔和的长阴影，安静的地中海工作室，细腻的静物摄影。' },
  { title: '窗边日光', tag: '插画', src: '/examples/illustration.png', prompt: '阳光照进高挑的拱形窗，奶油色几何墙面与陶土台阶，一株造型优雅的绿植，金色光面和柔和的青绿色阴影，细腻水粉与丝网印刷质感，安静的建筑空间插画。' },
];

type Transform = { zoom: number; x: number; y: number; rotation: number };
type ViewerState = { kind: 'asset'; asset: AssetView } | { kind: 'sample'; index: number };
type ConversationTurnPages = Record<string, TurnPage>;
type SendAck = { turn: TurnView; draft: StoredDraft; created: boolean };
type ConversationCreateAck = { conversation: ConversationView; draft: StoredDraft; defaultsWarnings?: string[] };
type ConversationListPage = { conversations: ConversationView[]; hasMore: boolean; nextCursor: number | null };
type RequestedAsset = {
  asset: AssetView;
  conversationTitle: string;
  draftId: number | null;
  turnSequence: number | null;
  nonce: number;
  returnPage: string | null;
};
type RequestedConversation = {
  conversationId: string;
  conversationTitle: string;
  draftId: number | null;
  turnId: string | null;
  turnSequence: number | null;
  nonce: number;
};
type RequestedPrompt = {
  caseId: string;
  title: string;
  prompt: string;
  target: 'new' | 'current';
  nonce: number;
};
type PendingLibraryPrompt = {
  draftId: number;
  expectedVersion: number;
  expectedPrompt: string;
  sourceId: string;
  prompt: string;
  title: string;
};
const resetTransform = (): Transform => ({ zoom: 1, x: 0, y: 0, rotation: 0 });
const activeStates = ['Pending', 'Submitted', 'Preparing', 'Running', 'Committing', 'CancelRequested'];
const retryStates = ['Failed', 'Cancelled', 'Interrupted'];

function IconButton({ label, children, onClick, disabled = false, className = '' }: { label: string; children: ReactNode; onClick: () => void; disabled?: boolean; className?: string }) {
  return <button type="button" className={`ws-icon-button ${className}`} aria-label={label} title={label} disabled={disabled} onClick={onClick}>{children}</button>;
}

function storedDraft(record: StoredDraft): Draft {
  return {
    id: record.id,
    name: record.name,
    prompt: record.prompt,
    collapsed: record.collapsed,
    promptSourceId: record.promptSourceId ?? null,
    params: { ...record.params },
    past: [],
    future: [],
  };
}

function localTurn(conversationId: string, draft: Draft, submissionKey: string): TurnView {
  return {
    id: `local-${submissionKey}`,
    conversationId,
    sequence: Date.now(),
    submissionKey,
    prompt: draft.prompt,
    modelId: draft.params.modelId,
    aspect: draft.params.aspect,
    count: draft.params.count,
    seed: null,
    requestedSeed: draft.params.seed,
    status: 'Pending',
    state: 'Pending',
    jobId: null,
    attemptId: null,
    width: draft.params.aspect === '3:4' ? 768 : 1024,
    height: draft.params.aspect === '4:3' ? 768 : 1024,
    assetId: null,
    assetAvailable: false,
    assetError: null,
    error: null,
    progressStage: null,
    progressStep: null,
    progressTotal: null,
    receivedAt: Date.now(),
  };
}

function TurnCard({
  turn,
  execution,
  onCancel,
  onRetry,
  onOpen,
  onExport,
  onCopy,
  onEdit,
  onReload,
  onLoadDetails,
}: {
  turn: TurnView;
  execution: StudioSnapshot['execution'] | null | undefined;
  onCancel: () => void;
  onRetry: () => void;
  onOpen: () => void;
  onExport: () => void;
  onCopy: () => void;
  onEdit: () => void;
  onReload: () => void;
  onLoadDetails: () => Promise<TurnDetail | null>;
}) {
  const [imageReady, setImageReady] = useState(false);
  const [imageFailed, setImageFailed] = useState(false);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [details, setDetails] = useState<TurnDetail | null>(null);
  const [detailsError, setDetailsError] = useState('');
  const progress = progressForTurn(turn, execution, imageReady);
  const active = activeStates.includes(turn.state);
  const complete = turn.state === 'Succeeded' && turn.assetId && turn.assetAvailable && !imageFailed;
  const cardStyle = { '--turn-aspect': `${turn.width} / ${turn.height}` } as CSSProperties;
  return <article className={`conversation-turn state-${turn.state}`} data-turn-id={turn.id} aria-label={`第 ${turn.sequence} 轮生成`}>
    <div className="user-message">
      <div className="user-message-body">
        <p>{turn.prompt}</p>
        <button type="button" aria-label="复制提示词" title="复制提示词" onClick={onCopy}><Copy size={13}/></button>
      </div>
      <span className="message-author">你</span>
    </div>
    <div className="assistant-response">
      <div className="response-source"><span className="assistant-mark"><Palette size={15}/></span><div><strong>Image Studio</strong><span>Z-Image Turbo · {turn.aspect} · 种子 {turn.seed ?? '随机'}</span></div></div>
      <div className="image-card" style={cardStyle}>
        {complete && <button className="image-card-open" type="button" onClick={onOpen} aria-label="放大查看生成图片"><img src={assetUrl(turn.assetId!)} alt={turn.prompt} draggable={false} onLoad={() => setImageReady(true)} onError={() => setImageFailed(true)}/>{imageReady && <span className="image-open-hint"><Expand size={14}/>查看大图</span>}</button>}
        {(!complete || !imageReady) && <div className="image-card-placeholder">
          {turn.state === 'Succeeded' && (!turn.assetAvailable || imageFailed) ? <><ImageIcon size={25}/><strong>任务已完成，图片无法显示</strong><span>{turn.assetError || 'PNG 可能已被移动、改写或无法解码。'}</span></> : active || turn.state === 'Succeeded' ? <><LoaderCircle size={25} className="spin"/><strong>{progress.label}</strong><span>{turn.state === 'Succeeded' ? '正在解码并校验 PNG' : '生成结果会在此处原位出现'}</span></> : <>{turn.state === 'Rejected' ? <X size={24}/> : <ImageIcon size={25}/>}<strong>{stateLabel(turn.state)}</strong><span>{turn.error || (turn.state === 'Cancelled' ? '本次生成已取消' : '本次执行没有可显示结果')}</span></>}
        </div>}
        {(active || turn.state === 'CancelRequested' || turn.state === 'Succeeded') && <div className="image-card-progress">
          <div className="progress-copy" role="status"><span>{progress.label}</span><strong>{progress.percent}%</strong></div>
          <div className={`progress-track ${turn.state === 'Succeeded' && !imageReady ? 'is-indeterminate' : ''}`}><span style={{ width: `${progress.percent}%` }}/></div>
        </div>}
        {turn.error && !active && <p className="turn-error" role="alert">{turn.error}</p>}
        <div className="image-card-actions">
          {active && <button className="danger-quiet" disabled={turn.state === 'CancelRequested' || turn.id.startsWith('local-') && turn.state === 'CancelRequested'} onClick={onCancel}><Square size={12}/>{turn.state === 'CancelRequested' ? '正在取消' : '停止'}</button>}
          {turn.jobId && retryStates.includes(turn.state) && <button onClick={onRetry}><RefreshCw size={12}/>重试</button>}
          {!active && <button onClick={onEdit}><Pencil size={12}/>编辑后重发</button>}
          {turn.assetId && <><button onClick={onOpen}><Expand size={12}/>查看</button><button disabled={!turn.assetAvailable} onClick={onExport}><Download size={12}/>导出 PNG</button></>}
          {turn.state === 'Succeeded' && (!turn.assetAvailable || imageFailed) && <button onClick={() => { setImageFailed(false); setImageReady(false); onReload(); }}><RefreshCw size={12}/>重新加载</button>}
          <button onClick={onCopy}><Copy size={12}/>复制提示词</button>
          <button aria-expanded={detailsOpen} onClick={async () => {
            const next = !detailsOpen;
            setDetailsOpen(next);
            if (next && !details && turn.jobId) {
              try { setDetails(await onLoadDetails()); } catch (error) { setDetailsError(String(error)); }
            }
          }}><FileText size={12}/>{detailsOpen ? '收起记录' : '制作记录'}</button>
        </div>
        {detailsOpen && <div className="turn-details"><p>轮次 {turn.id}<br/>任务 {turn.jobId ?? '尚未创建'}<br/>执行 {turn.attemptId ?? '等待预检'}</p>{details?.attempts.map(attempt => <section key={attempt.attemptId}><strong>{stateLabel(attempt.state)} · {attempt.attemptId}</strong>{attempt.error && <p>{attempt.error}</p>}{attempt.manifest ? <pre>{JSON.stringify(attempt.manifest, null, 2)}</pre> : <p>该 Attempt 尚无完整制作记录。</p>}</section>)}{detailsError && <p className="error-text">{detailsError}</p>}{!turn.jobId && <p>{turn.error || '请求尚未创建任务。'}</p>}</div>}
      </div>
    </div>
  </article>;
}

function ImageViewer({ title, prompt, source, isAsset, backLabel = '返回会话', onClose, onUsePrompt, onExport }: { title: string; prompt: string; source: string; isAsset: boolean; backLabel?: string; onClose: () => void; onUsePrompt: () => void; onExport?: () => void }) {
  const [transform, setTransform] = useState<Transform>(resetTransform);
  const [viewport, setViewport] = useState({ width: 800, height: 500 });
  const [natural, setNatural] = useState({ width: 1200, height: 800 });
  const [failed, setFailed] = useState(false);
  const [descriptionOpen, setDescriptionOpen] = useState(false);
  const canvasRef = useRef<HTMLDivElement>(null);
  const pointer = useRef<{ id: number; startX: number; startY: number; offsetX: number; offsetY: number } | null>(null);
  useEffect(() => {
    if (!canvasRef.current) return;
    const observer = new ResizeObserver(entries => {
      const { width, height } = entries[0].contentRect;
      if (width && height) setViewport({ width, height });
    });
    observer.observe(canvasRef.current);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    const key = (event: globalThis.KeyboardEvent) => { if (event.key === 'Escape' && !event.isComposing) onClose(); };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [onClose]);
  const rotated = Math.abs(transform.rotation % 180) === 90;
  const fit = Math.min(1, (viewport.width - 112) / (rotated ? natural.height : natural.width), (viewport.height - 72) / (rotated ? natural.width : natural.height));
  const imageStyle = {
    width: natural.width * Math.max(.01, fit),
    height: natural.height * Math.max(.01, fit),
    transform: `translate(${transform.x}px, ${transform.y}px) rotate(${transform.rotation}deg) scale(${transform.zoom})`,
  } satisfies CSSProperties;
  const startPan = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || failed) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    pointer.current = { id: event.pointerId, startX: event.clientX, startY: event.clientY, offsetX: transform.x, offsetY: transform.y };
  };
  const movePan = (event: PointerEvent<HTMLDivElement>) => {
    const drag = pointer.current;
    if (!drag || drag.id !== event.pointerId) return;
    if (Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY) >= 4) setTransform(current => ({ ...current, x: drag.offsetX + event.clientX - drag.startX, y: drag.offsetY + event.clientY - drag.startY }));
  };
  return <div className="image-viewer" role="dialog" aria-modal="true" aria-label="图片查看器">
    <header><button className="viewer-back" onClick={onClose}><ChevronLeft size={17}/>{backLabel}</button><button className="viewer-title" onClick={() => setDescriptionOpen(!descriptionOpen)}><Palette size={15}/><span>{prompt}</span></button><span className="viewer-source">{isAsset ? '本机生成' : '示例 · 非本机生成'}</span>{onExport && <button className="viewer-export" onClick={onExport}><Download size={13}/>导出 PNG</button>}</header>
    {descriptionOpen && <aside className="viewer-description"><div><strong>{title}</strong><button onClick={() => setDescriptionOpen(false)}><X size={13}/></button></div><p>{prompt}</p><button onClick={onUsePrompt}>用于当前会话</button></aside>}
    <div ref={canvasRef} className="viewer-canvas" onPointerDown={startPan} onPointerMove={movePan} onPointerUp={() => { pointer.current = null; }} onPointerCancel={() => { pointer.current = null; }} onWheel={event => { event.preventDefault(); setTransform(current => ({ ...current, zoom: clampZoom(current.zoom + (event.deltaY < 0 ? .25 : -.25)) })); }}>
      {failed ? <div className="viewer-error"><ImageIcon size={30}/><h3>无法显示这张图片</h3><p>文件可能已被移动或改写，历史记录保持不变。</p><button onClick={onClose}>返回会话</button></div> : <img src={source} alt={title} draggable={false} style={imageStyle} onLoad={event => setNatural({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight })} onError={() => setFailed(true)}/>}
    </div>
    {!failed && <div className="viewer-tools" role="toolbar" aria-label="图片工具">
      <IconButton label="缩小图片" disabled={transform.zoom <= .25} onClick={() => setTransform(current => ({ ...current, zoom: clampZoom(current.zoom - .25) }))}><ZoomOut size={18}/></IconButton>
      <IconButton label="放大图片" disabled={transform.zoom >= 4} onClick={() => setTransform(current => ({ ...current, zoom: clampZoom(current.zoom + .25) }))}><ZoomIn size={18}/></IconButton>
      <span className="ws-tool-divider"/>
      <IconButton label="向左旋转90度" onClick={() => setTransform(current => ({ ...current, rotation: (current.rotation - 90) % 360 }))}><RotateCcw size={17}/></IconButton>
      <IconButton label="向右旋转90度" onClick={() => setTransform(current => ({ ...current, rotation: (current.rotation + 90) % 360 }))}><RotateCw size={17}/></IconButton>
      <IconButton label="重置画板" onClick={() => setTransform(resetTransform())}><Expand size={17}/></IconButton>
    </div>}
    {!failed && <span className="viewer-zoom">{Math.round(transform.zoom * 100)}%</span>}
  </div>;
}

export default function Workspace({ onModels, studio, requestedAsset, requestedConversation, requestedComposer, requestedModel, requestedPrompt, sidebarCollapsed, onSidebarCollapsedChange, onRefresh, onAssetReturn }: { onModels: () => void; studio: StudioSnapshot | null; requestedAsset: RequestedAsset | null; requestedConversation: RequestedConversation | null; requestedComposer: { nonce: number } | null; requestedModel: { draft: StoredDraft; conversationId: string; nonce: number } | null; requestedPrompt: RequestedPrompt | null; sidebarCollapsed: boolean; onSidebarCollapsedChange: (value: boolean) => void; onRefresh: () => void; onAssetReturn: () => void }) {
  const [state, setState] = useState(initialWorkspace);
  const [conversations, setConversations] = useState<ConversationView[]>([]);
  const [conversationsHasMore, setConversationsHasMore] = useState(false);
  const [conversationsNextCursor, setConversationsNextCursor] = useState<number | null>(null);
  const [activeConversationId, setActiveConversationId] = useState<string | null>(null);
  const [turnPages, setTurnPages] = useState<ConversationTurnPages>({});
  const [optimistic, setOptimistic] = useState<TurnView[]>([]);
  const [popup, setPopup] = useState<'params' | 'models' | null>(null);
  const [loaded, setLoaded] = useState(!native);
  const [switching, setSwitching] = useState(false);
  const [submittingDraftId, setSubmittingDraftId] = useState<number | null>(null);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [loadingConversations, setLoadingConversations] = useState(false);
  const [saveStatus, setSaveStatus] = useState<'saved' | 'pending' | 'saving' | 'error'>('saved');
  const [saveError, setSaveError] = useState('');
  const [actionError, setActionError] = useState('');
  const [copyState, setCopyState] = useState('');
  const [viewer, setViewer] = useState<ViewerState | null>(null);
  const [viewerReturnsToPage, setViewerReturnsToPage] = useState(false);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const [replacePromptTurn, setReplacePromptTurn] = useState<TurnView | null>(null);
  const [replaceLibraryPrompt, setReplaceLibraryPrompt] = useState<PendingLibraryPrompt | null>(null);
  const [promptActionBusy, setPromptActionBusy] = useState(false);
  const [exporting, setExporting] = useState(false);
  const initialized = useRef(false);
  const saveQueue = useRef<DraftSaveQueue | null>(null);
  const flushTail = useRef<Promise<void>>(Promise.resolve());
  const intent = useRef<{ content: string; key: string } | null>(null);
  const compositions = useRef(new Map<number, string>());
  const compositionDraftId = useRef<number | null>(null);
  const compositionEndedAt = useRef(0);
  const textarea = useRef<HTMLTextAreaElement>(null);
  const timeline = useRef<HTMLDivElement>(null);
  const nearBottom = useRef(true);
  const forceScroll = useRef(false);
  const cancellationKeys = useRef(new Set<string>());
  const submissionGate = useRef(createSubmissionGate());
  const handledAssetNonce = useRef<number | null>(null);
  const handledPromptNonce = useRef<number | null>(null);
  const lastSavedSelection = useRef<string | null>(null);
  const loadedMoreConversations = useRef(false);
  const activeRef = useRef<{ state: WorkspaceState; activeConversationId: string | null; optimistic: TurnView[]; conversations: ConversationView[] }>({ state, activeConversationId, optimistic, conversations });
  activeRef.current = { state, activeConversationId, optimistic, conversations };
  if (!saveQueue.current) saveQueue.current = new DraftSaveQueue(
    (draft, expectedVersion) => api<StoredDraft>('draft_save', { draft, expectedVersion }),
    (value, error) => { setSaveStatus(value); if (error) setSaveError(error); },
  );

  const active = state.drafts.find(draft => draft.id === state.activeId) ?? state.drafts[0] ?? initialWorkspace().drafts[0];
  const previewConversation: ConversationView = { id: `local-conversation-${active.id}`, draftId: active.id, title: active.name, titleSource: 'auto', createdAt: 0, activitySeq: 0, running: false };
  const visibleConversations = conversations.length ? conversations : state.drafts.map((draft, index) => ({ ...previewConversation, id: `local-conversation-${draft.id}`, draftId: draft.id, title: draft.name, activitySeq: state.drafts.length - index }));
  const currentConversation = visibleConversations.find(conversation => conversation.id === activeConversationId)
    ?? visibleConversations.find(conversation => conversation.draftId === active.id)
    ?? previewConversation;
  const serverPage = turnPages[currentConversation.id];
  const localTurns = optimistic.filter(turn => turn.conversationId === currentConversation.id);
  const turns = mergeTurns(serverPage?.turns ?? [], localTurns);
  const currentActiveTurn = [...turns].reverse().find(turn => activeStates.includes(turn.state));
  const localBusy = turns.some(turn => turn.id.startsWith('local-') && activeStates.includes(turn.state));
  const systemBusy = Boolean(studio?.execution.busy || localBusy || visibleConversations.some(conversation => conversation.running));
  const runningConversation = visibleConversations.find(conversation => conversation.running)
    ?? visibleConversations.find(conversation => optimistic.some(turn => turn.id.startsWith('local-') && turn.conversationId === conversation.id && activeStates.includes(turn.state)));
  const overByteBudget = new TextEncoder().encode(active.prompt).length > 16384;
  const validParams = supportedDraftParams(active.params);
  const validText = Boolean(active.prompt.trim()) && !overByteBudget;
  const modelReady = studio?.model.ready ?? false;
  const selectedModelExecutable = modelReady && active.params.modelId === DEFAULT_MODEL_ID;
  const selectedModelLabel = active.params.modelId === DEFAULT_MODEL_ID
    ? modelReady ? 'Z-Image Turbo' : 'Z-Image Turbo · 未就绪'
    : `未就绪模型 · ${active.params.modelId}`;
  const busyInAnotherConversation = Boolean(systemBusy && !currentActiveTurn && runningConversation && runningConversation.id !== currentConversation.id);
  const canSend = native && loaded && selectedModelExecutable && validText && validParams && validSeed(active.params.seed) && !switching && submittingDraftId === null && !systemBusy;

  const dispatch = (action: WorkspaceAction) => {
    const next = workspaceReducer(activeRef.current.state, action);
    activeRef.current.state = next;
    setState(next);
  };
  const selectConversationLocally = (id: string | null) => {
    activeRef.current.activeConversationId = id;
    setActiveConversationId(id);
  };
  const enqueueAll = () => {
    for (const draft of activeRef.current.state.drafts) saveQueue.current!.enqueue(confirmedDraft(draft, compositions.current));
  };
  const flushAll = (): Promise<void> => {
    if (!native || !initialized.current) return Promise.resolve();
    const run = async () => {
      try {
        do {
          enqueueAll();
          await saveQueue.current!.flush();
        } while (saveQueue.current!.hasPending());
        setSaveError('');
      } catch (error) {
        setSaveError(String(error));
        throw error;
      }
    };
    const queued = flushTail.current.then(run);
    flushTail.current = queued.catch(() => {});
    return queued;
  };
  const flushRef = useRef(flushAll);
  flushRef.current = flushAll;

  useEffect(() => {
    if (!native || initialized.current || !studio || studio.error) return;
    initialized.current = true;
    const restored = studio.drafts.map(storedDraft);
    saveQueue.current!.initialize(studio.drafts);
    dispatch({ type: 'restore', drafts: restored, activeId: studio.activeId ?? restored[0]?.id ?? 1 });
    setConversations(studio.conversations);
    setConversationsHasMore(studio.conversationsHasMore);
    setConversationsNextCursor(studio.conversationsNextCursor);
    selectConversationLocally(studio.activeConversationId);
    if (studio.activeConversationId) setTurnPages({ [studio.activeConversationId]: { turns: studio.turns, hasMore: studio.turnsHasMore, nextCursor: studio.turnsNextCursor } });
    setLoaded(true);
  }, [studio]);

  useEffect(() => {
    if (!studio) return;
    setConversations(previous => {
      if (!loadedMoreConversations.current) return studio.conversations;
      const merged = new Map(previous.map(conversation => [conversation.id, conversation]));
      for (const conversation of studio.conversations) merged.set(conversation.id, conversation);
      return [...merged.values()].sort((left, right) => right.activitySeq - left.activitySeq || right.id.localeCompare(left.id));
    });
    if (studio.activeConversationId) {
      const persistedKeys = new Set(studio.turns.map(turn => turn.submissionKey));
      const runningConversationIds = new Set(studio.conversations.filter(conversation => conversation.running).map(conversation => conversation.id));
      setOptimistic(previous => previous.filter(turn => !persistedKeys.has(turn.submissionKey)
        && (turn.id.startsWith('local-') || runningConversationIds.has(turn.conversationId))));
    }
    if (!loadedMoreConversations.current) {
      setConversationsHasMore(studio.conversationsHasMore);
      setConversationsNextCursor(studio.conversationsNextCursor);
    }
    const id = studio.activeConversationId;
    if (!id || id !== activeRef.current.activeConversationId) return;
    setTurnPages(previous => {
      const existing = previous[id];
      const merged = new Map(existing?.turns.map(turn => [turn.id, turn]) ?? []);
      for (const turn of studio.turns) merged.set(turn.id, turn);
      const nextTurns = [...merged.values()].sort((left, right) => left.sequence - right.sequence);
      return {
        ...previous,
        [id]: {
          turns: nextTurns,
          hasMore: existing ? existing.hasMore : studio.turnsHasMore,
          nextCursor: existing?.nextCursor ?? studio.turnsNextCursor,
        },
      };
    });
  }, [studio]);

  useEffect(() => {
    if (studio?.execution.busy) setActionError('');
  }, [studio?.execution.busy, studio?.execution.stage]);

  useEffect(() => {
    if (!native || !loaded) return;
    enqueueAll();
    if (lastSavedSelection.current !== activeConversationId) setSaveStatus('pending');
    const timer = window.setTimeout(() => { void flushRef.current().catch(() => {}); }, 300);
    return () => window.clearTimeout(timer);
  }, [state, loaded, activeConversationId]);

  useEffect(() => {
    if (!native) return;
    const registration = listen('studio-close-requested', () => {
      void flushRef.current().then(() => api('app_close')).catch(() => {});
    });
    return () => { void registration.then(unlisten => unlisten()); };
  }, []);

  useEffect(() => {
    const close = (event: globalThis.KeyboardEvent) => {
      if (event.key !== 'Escape' || event.isComposing) return;
      const hadPopup = popup !== null;
      setPopup(null);
      setReplacePromptTurn(null);
      setReplaceLibraryPrompt(null);
      if (hadPopup) requestAnimationFrame(() => textarea.current?.focus());
    };
    const click = (event: globalThis.PointerEvent) => {
      if (!(event.target instanceof Element) || !event.target.closest('[data-composer-popup]')) setPopup(null);
    };
    window.addEventListener('keydown', close);
    window.addEventListener('pointerdown', click);
    return () => { window.removeEventListener('keydown', close); window.removeEventListener('pointerdown', click); };
  }, [popup]);

  useEffect(() => {
    if (activeConversationId) lastSavedSelection.current = activeConversationId;
  }, [activeConversationId]);

  useEffect(() => {
    if (!forceScroll.current && !nearBottom.current) return;
    const frame = requestAnimationFrame(() => {
      const element = timeline.current;
      if (element) element.scrollTop = element.scrollHeight;
      forceScroll.current = false;
    });
    return () => cancelAnimationFrame(frame);
  }, [turns.length, studio?.execution.stage, studio?.execution.step, activeConversationId]);

  const ensureHistory = async (conversationId: string) => {
    if (!native || turnPages[conversationId]) return;
    try {
      const page = await api<TurnPage>('conversation_history', { conversationId, beforeSequence: null, limit: 30 });
      setTurnPages(previous => ({ ...previous, [conversationId]: page }));
    } catch (error) {
      setActionError(String(error));
    }
  };

  const switchConversation = async (conversation: ConversationView) => {
    if (conversation.id === currentConversation.id || switching || submittingDraftId !== null) return;
    if (compositions.current.size > 0) {
      textarea.current?.focus();
      setActionError('请先完成中文输入法候选确认，再切换会话。');
      return;
    }
    setSwitching(true);
    setActionError('');
    try {
      if (!native) {
        selectConversationLocally(conversation.id);
        dispatch({ type: 'select', id: conversation.draftId });
        return;
      }
      await flushAll();
      await api('conversation_select', { conversationId: conversation.id });
      selectConversationLocally(conversation.id);
      dispatch({ type: 'select', id: conversation.draftId });
      nearBottom.current = true;
      setViewer(null);
      setPopup(null);
      await ensureHistory(conversation.id);
      onRefresh();
      requestAnimationFrame(() => textarea.current?.focus());
    } catch (error) {
      setActionError(String(error));
    } finally {
      setSwitching(false);
    }
  };

  const applyCreatedConversation = (created: ConversationCreateAck) => {
    saveQueue.current!.remember(created.draft);
    dispatch({ type: 'append', draft: storedDraft(created.draft) });
    setConversations(previous => [created.conversation, ...previous.filter(conversation => conversation.id !== created.conversation.id)]);
    setTurnPages(previous => ({ ...previous, [created.conversation.id]: { turns: [], hasMore: false, nextCursor: null } }));
    selectConversationLocally(created.conversation.id);
    nearBottom.current = true;
    setViewer(null);
    setPopup(null);
    if (created.defaultsWarnings?.length) setActionError(created.defaultsWarnings.join('；'));
    onRefresh();
    requestAnimationFrame(() => textarea.current?.focus());
  };

  const createPromptConversation = async (request: RequestedPrompt) => {
    setPromptActionBusy(true);
    setActionError('');
    try {
      if (!native) {
        const id = Math.max(...activeRef.current.state.drafts.map(draft => draft.id)) + 1;
        const draft = { ...createDraft(id, request.title), prompt: request.prompt, promptSourceId: request.caseId };
        dispatch({ type: 'append', draft });
        const conversation = { ...previewConversation, id: `local-conversation-${id}`, draftId: id, title: request.title };
        setConversations(previous => [conversation, ...previous.filter(item => item.id !== conversation.id)]);
        selectConversationLocally(conversation.id);
        setPopup(null);
        requestAnimationFrame(() => textarea.current?.focus());
        return;
      }
      await flushAll();
      const created = await api<ConversationCreateAck>('conversation_create', {
        input: {
          initialTitle: request.title,
          initialPrompt: request.prompt,
          promptSourceId: request.caseId,
        },
      });
      applyCreatedConversation(created);
    } catch (error) {
      setActionError(String(error));
    } finally {
      setPromptActionBusy(false);
    }
  };

  const applyPromptToDraft = (draftId: number, prompt: string, sourceId: string) => {
    const draft = activeRef.current.state.drafts.find(value => value.id === draftId);
    if (!draft) {
      setActionError('目标会话已不存在，请重新打开灵感案例。');
      return false;
    }
    dispatch({ type: 'applyPrompt', id: draftId, value: prompt, sourceId, expected: draft.prompt });
    setPopup(null);
    setReplaceLibraryPrompt(null);
    requestAnimationFrame(() => textarea.current?.focus());
    void flushAll().catch(() => {});
    return true;
  };

  const beginLibraryPrompt = (request: RequestedPrompt) => {
    const draft = activeRef.current.state.drafts.find(value => value.id === activeRef.current.state.activeId);
    if (!draft) {
      setActionError('当前没有可用的目标会话。');
      return;
    }
    if (request.target === 'new') {
      void createPromptConversation(request);
      return;
    }
    if (draft.prompt === request.prompt) {
      applyPromptToDraft(draft.id, request.prompt, request.caseId);
      return;
    }
    if (!draft.prompt.trim()) {
      applyPromptToDraft(draft.id, request.prompt, request.caseId);
      return;
    }
    setReplaceLibraryPrompt({
      draftId: draft.id,
      expectedVersion: saveQueue.current!.version(draft.id),
      expectedPrompt: draft.prompt,
      sourceId: request.caseId,
      prompt: request.prompt,
      title: request.title,
    });
  };

  const confirmLibraryPrompt = () => {
    const pending = replaceLibraryPrompt;
    if (!pending) return;
    if (activeRef.current.state.activeId !== pending.draftId) {
      setReplaceLibraryPrompt(null);
      setActionError('目标会话已切换，本次替换未执行。请回到目标会话后重新应用。');
      return;
    }
    const draft = activeRef.current.state.drafts.find(value => value.id === pending.draftId);
    if (!draft || draft.prompt !== pending.expectedPrompt || saveQueue.current!.version(draft.id) !== pending.expectedVersion) {
      setReplaceLibraryPrompt(null);
      setActionError('目标草稿已发生变化，本次替换未执行。请重新应用案例。');
      return;
    }
    applyPromptToDraft(pending.draftId, pending.prompt, pending.sourceId);
  };

  const createConversation = async () => {
    if (switching || submittingDraftId !== null || promptActionBusy) return;
    if (compositions.current.size > 0) {
      textarea.current?.focus();
      setActionError('请先完成中文输入法候选确认，再新建会话。');
      return;
    }
    setActionError('');
    try {
      if (!native) {
        const id = Math.max(...state.drafts.map(draft => draft.id)) + 1;
        dispatch({ type: 'append', draft: { ...state.drafts[0], id, name: '新会话', prompt: '', past: [], future: [], params: { modelId: DEFAULT_MODEL_ID, aspect: '1:1', count: 1, seed: '' } } });
        const conversation = { ...previewConversation, id: `local-conversation-${id}`, draftId: id, title: '新会话' };
        setConversations([conversation, ...visibleConversations]);
        selectConversationLocally(conversation.id);
        return;
      }
      await flushAll();
      const created = await api<ConversationCreateAck>('conversation_create', { input: null });
      applyCreatedConversation(created);
    } catch (error) {
      setActionError(String(error));
    }
  };

  const loadMoreConversations = async () => {
    if (!native || !conversationsHasMore || conversationsNextCursor === null || loadingConversations) return;
    setLoadingConversations(true);
    try {
      const page = await api<ConversationListPage>('conversation_list', { limit: 100, beforeActivity: conversationsNextCursor });
      setConversations(previous => {
        const known = new Set(previous.map(conversation => conversation.id));
        return [...previous, ...page.conversations.filter(conversation => !known.has(conversation.id))];
      });
      setConversationsHasMore(page.hasMore);
      setConversationsNextCursor(page.nextCursor);
      loadedMoreConversations.current = true;
    } catch (error) {
      setActionError(String(error));
    } finally {
      setLoadingConversations(false);
    }
  };

  const beginRename = (conversation: ConversationView) => {
    setRenamingId(conversation.id);
    setRenameValue(conversation.title);
  };
  const saveRename = async () => {
    if (!renamingId) return;
    const id = renamingId;
    const title = renameValue.trim();
    setRenamingId(null);
    if (!title) return;
    try {
      if (native) await api('conversation_rename', { conversationId: id, title });
      setConversations(previous => previous.map(conversation => conversation.id === id ? { ...conversation, title, titleSource: 'manual' } : conversation));
      const conversation = visibleConversations.find(value => value.id === id);
      if (conversation) dispatch({ type: 'rename', draftId: conversation.draftId, name: title });
      onRefresh();
    } catch (error) {
      setActionError(String(error));
    }
  };

  const setDraftParams = (patch: Partial<DraftParams>) => dispatch({ type: 'params', patch });
  const applyPrompt = (prompt: string) => {
    dispatch({ type: 'edit', value: prompt });
    setReplacePromptTurn(null);
    setViewer(null);
    requestAnimationFrame(() => textarea.current?.focus());
  };
  const copyText = async (text: string, marker: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopyState(marker);
      window.setTimeout(() => setCopyState(''), 1400);
    } catch {
      setActionError('无法访问剪贴板，请手动选择文字复制。');
    }
  };
  const requestEdit = (turn: TurnView) => {
    if (active.prompt.trim() && active.prompt !== turn.prompt) setReplacePromptTurn(turn);
    else applyPrompt(turn.prompt);
  };

  const sendPrompt = async () => {
    if (submittingDraftId !== null || switching) return;
    const source = confirmedDraft(active, compositions.current);
    if (compositions.current.has(active.id)) {
      setActionError('请先完成中文输入法候选确认，再发送。');
      return;
    }
    if (!canSend) {
      setActionError(composerStatus);
      return;
    }
    if (!source.prompt.trim() || !supportedDraftParams(source.params) || !validSeed(source.params.seed)) {
      setActionError('请先填写有效提示词和参数。');
      return;
    }
    if (systemBusy) {
      setActionError('当前已有生成或安装在进行，请等待完成或前往运行中的会话。');
      return;
    }
    if (!submissionGate.current.tryStart()) return;
    let submissionKey = '';
    try {
      const content = JSON.stringify({ conversationId: currentConversation.id, draftId: source.id, prompt: source.prompt, ...source.params });
      if (intent.current?.content !== content) intent.current = { content, key: crypto.randomUUID() };
      submissionKey = intent.current.key;
      const pendingTurn = localTurn(currentConversation.id, source, submissionKey);
      forceScroll.current = true;
      setOptimistic(previous => [...previous.filter(turn => turn.submissionKey !== submissionKey), pendingTurn]);
      setSubmittingDraftId(source.id);
      setActionError('');
      await flushAll();
      const expectedVersion = saveQueue.current!.version(source.id);
      if (cancellationKeys.current.has(submissionKey)) {
        setOptimistic(previous => previous.map(turn => turn.submissionKey === submissionKey ? { ...turn, status: 'Cancelled', state: 'Cancelled', error: '发送已停止，提示词保留在输入框。' } : turn));
        intent.current = null;
        cancellationKeys.current.delete(submissionKey);
        requestAnimationFrame(() => textarea.current?.focus());
        return;
      }
      const accepted = await api<SendAck>('conversation_send', {
        input: {
          conversationId: currentConversation.id,
          draftId: source.id,
          expectedVersion,
          prompt: source.prompt,
          modelId: source.params.modelId,
          aspect: source.params.aspect,
          count: source.params.count,
          seed: source.params.seed,
          submissionKey,
        },
      });
      saveQueue.current!.remember(accepted.draft);
      dispatch({ type: 'consume', draftId: accepted.draft.id, name: accepted.draft.name, params: accepted.draft.params });
      setConversations(previous => previous.map(conversation => conversation.id === currentConversation.id ? { ...conversation, title: accepted.draft.name, titleSource: conversation.titleSource === 'manual' ? 'manual' : 'auto', running: true } : conversation));
      setOptimistic(previous => [...previous.filter(turn => turn.submissionKey !== submissionKey), accepted.turn]);
      intent.current = null;
      if (cancellationKeys.current.delete(submissionKey)) {
        try { await api('conversation_cancel_turn', { turnId: accepted.turn.id }); } catch (error) { setActionError(String(error)); }
      }
      onRefresh();
      requestAnimationFrame(() => textarea.current?.focus());
    } catch (error) {
      const message = String(error);
      const cancelled = cancellationKeys.current.delete(submissionKey);
      if (cancelled) intent.current = null;
      setOptimistic(previous => previous.map(turn => turn.submissionKey === submissionKey ? { ...turn, status: cancelled ? 'Cancelled' : 'Rejected', state: cancelled ? 'Cancelled' : 'Rejected', error: cancelled ? '发送已停止，提示词保留在输入框。' : message } : turn));
      setActionError(message);
    } finally {
      submissionGate.current.finish();
      setSubmittingDraftId(null);
    }
  };

  const cancelTurn = async (turn: TurnView) => {
    setActionError('');
    if (turn.id.startsWith('local-')) {
      cancellationKeys.current.add(turn.submissionKey);
      setOptimistic(previous => previous.map(value => value.submissionKey === turn.submissionKey ? { ...value, status: 'CancelRequested', state: 'CancelRequested' } : value));
      return;
    }
    try {
      await api('conversation_cancel_turn', { turnId: turn.id });
      onRefresh();
    } catch (error) {
      setActionError(String(error));
    }
  };
  const retryTurn = async (turn: TurnView) => {
    if (!turn.jobId) return;
    setActionError('');
    try {
      await api('generation_retry', { jobId: turn.jobId });
      onRefresh();
    } catch (error) {
      setActionError(String(error));
    }
  };
  const exportAsset = async (asset: AssetView) => {
    if (exporting) return;
    setExporting(true);
    setActionError('');
    try { await api('asset_export', { assetId: asset.id }); }
    catch (error) { setActionError(String(error)); }
    finally { setExporting(false); }
  };
  const openAsset = (asset: AssetView) => {
    selectConversationLocally(asset.conversationId ?? currentConversation.id);
    setViewerReturnsToPage(false);
    setViewer({ kind: 'asset', asset });
    if (native) void api('workspace_select', { activeId: active.id, selectedAssetId: asset.id }).catch(() => {});
  };
  const assetFromTurn = (turn: TurnView): AssetView | null => {
    if (!turn.assetId) return null;
    return studio?.assets.find(value => value.id === turn.assetId) ?? {
      id: turn.assetId,
      jobId: turn.jobId ?? '',
      conversationId: turn.conversationId,
      turnId: turn.id,
      prompt: turn.prompt,
      width: turn.width,
      height: turn.height,
      seed: turn.seed,
      bytes: 0,
      available: turn.assetAvailable,
      error: turn.assetError,
    };
  };
  const loadOlder = async () => {
    const page = turnPages[currentConversation.id];
    if (!native || !page?.hasMore || page.nextCursor === null || loadingOlder) return;
    setLoadingOlder(true);
    const element = timeline.current;
    const previousHeight = element?.scrollHeight ?? 0;
    const previousScroll = element?.scrollTop ?? 0;
    nearBottom.current = false;
    try {
      const older = await api<TurnPage>('conversation_history', { conversationId: currentConversation.id, beforeSequence: page.nextCursor, limit: 30 });
      setTurnPages(previous => {
        const current = previous[currentConversation.id] ?? page;
        const merged = new Map(older.turns.map(turn => [turn.id, turn]));
        for (const turn of current.turns) merged.set(turn.id, turn);
        return { ...previous, [currentConversation.id]: { turns: [...merged.values()].sort((left, right) => left.sequence - right.sequence), hasMore: older.hasMore, nextCursor: older.nextCursor } };
      });
      requestAnimationFrame(() => { if (element) element.scrollTop = previousScroll + element.scrollHeight - previousHeight; });
    } catch (error) {
      setActionError(String(error));
    } finally {
      setLoadingOlder(false);
    }
  };

  const revealConversation = async (conversationId: string, conversationTitle: string, draftId: number | null, turnId: string | null, turnSequence: number | null) => {
    const target = conversations.find(conversation => conversation.id === conversationId) ?? {
      ...previewConversation,
      id: conversationId,
      draftId: draftId ?? active.id,
      title: conversationTitle,
    };
    if (target.id !== currentConversation.id) await switchConversation(target);
    if (native && turnSequence !== null) {
      const around = await api<TurnPage>('conversation_history', {
        conversationId,
        beforeSequence: turnSequence + 1,
        limit: 30,
      });
      setTurnPages(previous => {
        const merged = new Map(previous[conversationId]?.turns.map(turn => [turn.id, turn]) ?? []);
        for (const turn of around.turns) merged.set(turn.id, turn);
        return {
          ...previous,
          [conversationId]: {
            turns: [...merged.values()].sort((left, right) => left.sequence - right.sequence),
            hasMore: around.hasMore || previous[conversationId]?.hasMore || false,
            nextCursor: previous[conversationId]?.nextCursor ?? around.nextCursor,
          },
        };
      });
    }
    if (turnId) {
      requestAnimationFrame(() => {
        const escaped = CSS.escape(turnId);
        timeline.current?.querySelector<HTMLElement>(`[data-turn-id="${escaped}"]`)?.scrollIntoView({ block: 'center' });
      });
    }
  };

  const studioAvailable = studio !== null;
  useEffect(() => {
    if (!requestedAsset || !studioAvailable || !loaded) return;
    const claim = claimOneShotNonce(requestedAsset.nonce, handledAssetNonce.current);
    if (!claim.claimed) return;
    handledAssetNonce.current = claim.processed;
    const open = async () => {
      try {
        await revealConversation(
          requestedAsset.asset.conversationId ?? currentConversation.id,
          requestedAsset.conversationTitle,
          requestedAsset.draftId,
          requestedAsset.asset.turnId,
          requestedAsset.turnSequence,
        );
        setViewerReturnsToPage(Boolean(requestedAsset.returnPage));
        setViewer({ kind: 'asset', asset: requestedAsset.asset });
      } catch (error) {
        setActionError(String(error));
      }
    };
    void open();
  }, [requestedAsset?.nonce, loaded, studioAvailable]);

  useEffect(() => {
    if (!requestedConversation || !loaded) return;
    const open = async () => {
      try {
        setViewer(null);
        setViewerReturnsToPage(false);
        await revealConversation(
          requestedConversation.conversationId,
          requestedConversation.conversationTitle,
          requestedConversation.draftId,
          requestedConversation.turnId,
          requestedConversation.turnSequence,
        );
        requestAnimationFrame(() => textarea.current?.focus());
      } catch (error) {
        setActionError(String(error));
      }
    };
    void open();
  }, [requestedConversation?.nonce, loaded]);

  useEffect(() => {
    if (!requestedComposer || !loaded) return;
    setViewer(null);
    setPopup(null);
    requestAnimationFrame(() => textarea.current?.focus());
  }, [requestedComposer?.nonce, loaded]);

  useEffect(() => {
    if (!requestedModel || !loaded) return;
    saveQueue.current?.remember(requestedModel.draft);
    selectConversationLocally(requestedModel.conversationId);
    dispatch({
      type: 'params',
      id: requestedModel.draft.id,
      patch: { ...requestedModel.draft.params },
    });
    setPopup(null);
    requestAnimationFrame(() => textarea.current?.focus());
  }, [requestedModel?.nonce, loaded]);

  useEffect(() => {
    if (!requestedPrompt || !loaded) return;
    const claim = claimOneShotNonce(requestedPrompt.nonce, handledPromptNonce.current);
    if (!claim.claimed) return;
    handledPromptNonce.current = claim.processed;
    if (compositions.current.size > 0) {
      textarea.current?.focus();
      setActionError('请先完成中文输入法候选确认，再使用灵感提示词。');
      return;
    }
    beginLibraryPrompt(requestedPrompt);
  }, [requestedPrompt?.nonce, loaded]);

  const handleEditorKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.nativeEvent.isComposing || compositions.current.has(active.id)) return;
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'z') {
      event.preventDefault();
      dispatch({ type: event.shiftKey ? 'redo' : 'undo' });
      return;
    }
    if (shouldSendOnEnter({
      key: event.key,
      shiftKey: event.shiftKey,
      ctrlKey: event.ctrlKey,
      metaKey: event.metaKey,
      altKey: event.altKey,
      repeat: event.repeat,
      isComposing: event.nativeEvent.isComposing,
      keyCode: event.nativeEvent.keyCode,
      compositionEndedAt: compositionEndedAt.current,
      now: performance.now(),
    })) {
      event.preventDefault();
      void sendPrompt();
    }
  };

  const selectedAsset = viewer?.kind === 'asset' ? viewer.asset : null;
  const selectedSample = viewer?.kind === 'sample' ? samples[viewer.index] : null;
  const closeViewer = () => {
    setViewer(null);
    if (viewerReturnsToPage) {
      setViewerReturnsToPage(false);
      onAssetReturn();
    }
  };
  const composerStatus = submittingDraftId !== null
    ? '正在接收'
    : !native
      ? '桌面应用中可生成'
      : !modelReady
        ? '模型未就绪'
        : busyInAnotherConversation
          ? '其他会话正在生成'
          : !validParams
            ? '参数不受当前模型支持'
            : overByteBudget
              ? '提示词超过 16 KiB'
              : !validText
                ? '输入提示词后发送'
                : 'Enter 发送 · Shift+Enter 换行';
  const composerAction = composerActionFor({ submittingDraftId, currentActiveTurn, canSend });
  const sendButton = composerAction.kind === 'stop' && currentActiveTurn
    ? <button className="composer-send is-stop" disabled={currentActiveTurn.state === 'CancelRequested'} aria-label="停止生成" title="停止本次生成" onClick={event => { if (isPrimaryActivation(event.detail)) void cancelTurn(currentActiveTurn); }}>{currentActiveTurn.state === 'CancelRequested' ? <LoaderCircle size={17} className="spin"/> : <Square size={15}/>}</button>
    : <button className="composer-send" disabled={composerAction.disabled} aria-label={composerAction.kind === 'submitting' ? '正在提交' : '发送并生成'} title={composerAction.disabled ? composerStatus : '发送提示词并生成图片'} onClick={event => { if (isPrimaryActivation(event.detail)) void sendPrompt(); }}>{composerAction.kind === 'submitting' ? <LoaderCircle size={17} className="spin"/> : <ArrowUp size={18}/>}</button>;

  return <div className={`workspace conversation-workspace ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`}>
    <aside className={`conversation-sidebar ${sidebarCollapsed ? 'is-collapsed' : ''}`} aria-label="会话列表">
      <div className="conversation-sidebar-heading"><span>会话</span><IconButton label={sidebarCollapsed ? '展开会话列表' : '收起会话列表'} onClick={() => onSidebarCollapsedChange(!sidebarCollapsed)}>{sidebarCollapsed ? <PanelLeft size={17}/> : <PanelLeftClose size={17}/>}</IconButton></div>
      <button className="new-conversation" disabled={!loaded || switching || submittingDraftId !== null || promptActionBusy} onClick={() => void createConversation()}><Plus size={16}/><span>新建会话</span></button>
      <div className="conversation-list">
        {visibleConversations.map(conversation => <div className={`conversation-list-item ${conversation.id === currentConversation.id ? 'is-active' : ''}`} key={conversation.id}>
          {renamingId === conversation.id
            ? <input autoFocus value={renameValue} maxLength={128} aria-label="会话名称" onChange={event => setRenameValue(event.target.value)} onBlur={() => void saveRename()} onKeyDown={event => { if (event.key === 'Enter' && !event.nativeEvent.isComposing) { event.preventDefault(); void saveRename(); } if (event.key === 'Escape') { setRenamingId(null); } }}/>
            : <button className="conversation-open" disabled={!loaded || switching || submittingDraftId !== null} onClick={() => void switchConversation(conversation)}><MessageCircle size={15}/><span>{conversation.title}</span>{conversation.running && <i className="running-dot" title="正在生成"/>}</button>}
          {renamingId !== conversation.id && <button className="conversation-rename" disabled={!loaded} aria-label={`重命名${conversation.title}`} title="重命名会话" onClick={() => beginRename(conversation)}><Pencil size={12}/></button>}
        </div>)}
        {conversationsHasMore && <button className="load-conversations" disabled={loadingConversations} onClick={() => void loadMoreConversations()}>{loadingConversations ? <LoaderCircle size={12} className="spin"/> : <ChevronDown size={12}/>}加载更多会话</button>}
      </div>
      <div className="conversation-sidebar-footer"><button className="quiet" onClick={() => setViewer({ kind: 'sample', index: 0 })}><ImageIcon size={14}/>示例灵感</button></div>
    </aside>
    <main className="conversation-main">
      <header className="conversation-header">
        <button className="sidebar-toggle" aria-label={sidebarCollapsed ? '展开会话列表' : '收起会话列表'} onClick={() => onSidebarCollapsedChange(!sidebarCollapsed)}>{sidebarCollapsed ? <PanelLeft size={18}/> : <PanelLeftClose size={18}/>}</button>
        <div className="conversation-heading"><h1>{currentConversation.title}</h1><span>{studio?.execution.busy && currentActiveTurn ? studio.execution.summary : '本地图片创作'}</span></div>
        <div className="header-actions">{runningConversation && runningConversation.id !== currentConversation.id && <button className="running-session-link" onClick={() => void switchConversation(runningConversation)}><span className="running-dot"/>查看正在生成的会话</button>}<button className="header-new" disabled={!loaded || switching || submittingDraftId !== null || promptActionBusy} onClick={() => void createConversation()}><Plus size={14}/>新建</button></div>
      </header>
      <div className="conversation-timeline" ref={timeline} onScroll={event => { const element = event.currentTarget; nearBottom.current = element.scrollHeight - element.scrollTop - element.clientHeight < 96; }}>
        {turns.length > 0 && serverPage?.hasMore && <div className="history-loader"><button disabled={loadingOlder} onClick={() => void loadOlder()}>{loadingOlder ? <LoaderCircle size={13} className="spin"/> : <ChevronUp size={13}/>}加载更早记录</button></div>}
        {turns.length === 0 ? <section className="conversation-empty">
          <span className="ws-kicker"><Palette size={14}/> IMAGE STUDIO</span>
          <h2>从一个画面开始</h2>
          <p>写下想法并按 Enter。每一轮提示词和生成结果都会留在这个会话里。</p>
          <div className="empty-example-grid">{samples.map((sample, index) => <article key={sample.src}>
            <button className="empty-example-image" onClick={() => setViewer({ kind: 'sample', index })}><img src={sample.src} alt={sample.title}/><span>{sample.tag} · 示例</span></button>
            <div><strong>{sample.title}</strong><button onClick={() => applyPrompt(sample.prompt)}>使用提示词 <ArrowUp size={11}/></button></div>
          </article>)}</div>
        </section> : turns.map(turn => <TurnCard key={`${turn.conversationId}:${turn.submissionKey}`} turn={turn} execution={studio?.execution}
          onCancel={() => void cancelTurn(turn)}
          onRetry={() => void retryTurn(turn)}
          onOpen={() => { const asset = assetFromTurn(turn); if (asset) openAsset(asset); }}
          onExport={() => { const asset = assetFromTurn(turn); if (asset) void exportAsset(asset); }}
          onCopy={() => void copyText(turn.prompt, `turn-${turn.id}`)}
          onEdit={() => requestEdit(turn)}
          onReload={onRefresh}
          onLoadDetails={() => turn.jobId ? api<TurnDetail>('conversation_turn_detail', { turnId: turn.id }) : Promise.resolve(null)}
        />)}
        {(actionError || studio?.execution.cleanupError) && <div className="conversation-action-error" role="alert"><span>{actionError || studio?.execution.cleanupError}</span><button aria-label="关闭错误" onClick={() => setActionError('')}><X size={12}/></button></div>}
      </div>
      <section className={`conversation-composer-area ${active.collapsed ? 'is-collapsed' : ''}`}>
        {replacePromptTurn && <div className="replace-prompt-bar" role="dialog" aria-label="替换当前提示词"><span>当前输入框已有文字。用这条历史提示词替换吗？</span><button onClick={() => applyPrompt(replacePromptTurn.prompt)}>替换</button><button onClick={() => setReplacePromptTurn(null)}>取消</button></div>}
        {replaceLibraryPrompt && <div className="replace-prompt-bar" role="dialog" aria-label="替换为灵感提示词"><span>当前输入已有文字。用「{replaceLibraryPrompt.title}」的完整提示词替换吗？</span><button onClick={confirmLibraryPrompt}>替换</button><button onClick={() => setReplaceLibraryPrompt(null)}>取消</button></div>}
        <button className="collapsed-composer" onClick={() => dispatch({ type: 'collapse' })}><ChevronUp size={15}/><span>{active.prompt || '展开提示词，继续创作'}</span></button>
        <div className="conversation-composer">
          <div className="composer-caption"><span>{currentConversation.title}</span><span>{Array.from(active.prompt).length} 字符<IconButton label="收起提示词" onClick={() => { textarea.current?.blur(); dispatch({ type: 'collapse' }); }}><ChevronDown size={14}/></IconButton></span></div>
          <textarea ref={textarea} value={active.prompt} readOnly={submittingDraftId === active.id} aria-label="图片提示词" placeholder="描述你想创作的画面……" spellCheck={false}
            onChange={event => dispatch({ type: 'edit', value: event.target.value, record: !compositions.current.has(active.id) })}
            onKeyDown={handleEditorKeyDown}
            onCompositionStart={() => { compositionDraftId.current = active.id; compositions.current.set(active.id, active.prompt); }}
            onCompositionEnd={event => {
              const targetId = compositionDraftId.current ?? active.id;
              const before = compositions.current.get(targetId);
              compositionEndedAt.current = performance.now();
              compositions.current.delete(targetId);
              compositionDraftId.current = null;
              dispatch({ type: 'edit', id: targetId, value: event.currentTarget.value, before });
            }}
          />
          <div className="composer-toolbar">
            <div className="composer-left">
              <div className="composer-popup-anchor" data-composer-popup>
                <button className={`composer-chip ${popup === 'params' ? 'is-open' : ''}`} aria-expanded={popup === 'params'} onClick={() => { if (popup === 'params') { setPopup(null); requestAnimationFrame(() => textarea.current?.focus()); } else setPopup('params'); }}><SlidersHorizontal size={15}/><span>{active.params.aspect} · {active.params.count} 张</span></button>
                {popup === 'params' && <div className="composer-popover params-popover" role="dialog" aria-label="创作参数"><div className="popover-heading"><strong>创作参数</strong><span>当前型号 9 步 · q4 · 单张</span></div><span className="popover-label">画幅</span><div className="aspect-options">{['1:1', '4:3', '3:4'].map(aspect => <button key={aspect} className={active.params.aspect === aspect ? 'is-selected' : ''} aria-pressed={active.params.aspect === aspect} onClick={() => setDraftParams({ aspect })}><i style={{ aspectRatio: aspect.replace(':', '/') }}/><span>{aspect}</span></button>)}</div><label className="seed-field"><span>数量</span><input type="number" min={1} max={1} step={1} value={active.params.count} onChange={event => setDraftParams({ count: Number(event.target.value) })}/></label><label className="seed-field"><span>种子</span><input inputMode="numeric" value={active.params.seed} aria-invalid={!validSeed(active.params.seed)} placeholder="留空随机" onChange={event => setDraftParams({ seed: event.target.value })}/></label>{!validSeed(active.params.seed) && <p className="field-error">请输入 0–4294967295 的整数，或留空。</p>}<p className="popover-note">只修改当前会话的下一条请求，不会改变已发送记录。</p></div>}
              </div>
              <IconButton label={copyState === 'composer' ? '已复制提示词' : '复制提示词'} disabled={!active.prompt} onClick={() => void copyText(active.prompt, 'composer')}>{copyState === 'composer' ? <Check size={14}/> : <Copy size={14}/>}</IconButton>
            </div>
            <div className="composer-right">
              <span className="composer-status">{currentActiveTurn ? studio?.execution.summary : composerStatus}</span>
              <div className="composer-popup-anchor" data-composer-popup>
                <button className="model-select" aria-expanded={popup === 'models'} title={selectedModelExecutable ? '当前模型可执行' : '当前模型不可执行，请在模型菜单中选择已就绪型号'} onClick={() => { if (popup === 'models') { setPopup(null); requestAnimationFrame(() => textarea.current?.focus()); } else setPopup('models'); }}><span className="model-mark"><Palette size={13}/></span><span>{selectedModelLabel}</span><ChevronDown size={12}/></button>
                {popup === 'models' && <div className="composer-popover model-popover" role="dialog" aria-label="选择模型"><div className="popover-heading"><strong>图片模型</strong><span>{modelReady ? '1 个可执行模型' : '当前没有可执行模型'}</span></div><button className={`single-model-option ${active.params.modelId === DEFAULT_MODEL_ID ? 'is-selected' : ''}`} onClick={() => { setDraftParams({ modelId: DEFAULT_MODEL_ID }); setPopup(null); requestAnimationFrame(() => textarea.current?.focus()); }}><span className="model-mark"><Palette size={14}/></span><span><strong>Z-Image Turbo</strong><small>{studio?.model.summary ?? '正在读取模型状态'}</small></span>{active.params.modelId === DEFAULT_MODEL_ID && <Check size={14}/>}</button><button className="popover-action" onClick={() => { setPopup(null); onModels(); }}>前往模型中心 <ArrowUp size={12}/></button></div>}
              </div>
              {sendButton}
            </div>
          </div>
        </div>
        {overByteBudget && <p className="composer-warning" role="alert">提示词超过 16 KiB，请缩短后生成；内容完整保留。</p>}
        <div className={`composer-save-note ${saveStatus === 'error' ? 'error-text' : ''}`} role="status">{!native ? '浏览器预览 · 草稿保留于本次页面' : !loaded ? '正在恢复本地会话' : saveStatus === 'error' ? `保存失败：${saveError}` : saveStatus === 'saved' ? '草稿已保存到本机' : '正在保存草稿…'}{saveStatus === 'error' && <button onClick={() => void flushAll().catch(() => {})}>重试</button>}</div>
      </section>
    </main>
    {selectedAsset && <ImageViewer title="生成结果" prompt={selectedAsset.prompt} source={assetUrl(selectedAsset.id)} isAsset backLabel={viewerReturnsToPage ? '返回来源页' : '返回会话'} onClose={closeViewer} onUsePrompt={() => applyPrompt(selectedAsset.prompt)} onExport={() => void exportAsset(selectedAsset)}/>}
    {selectedSample && <ImageViewer title={selectedSample.title} prompt={selectedSample.prompt} source={selectedSample.src} isAsset={false} onClose={closeViewer} onUsePrompt={() => applyPrompt(selectedSample.prompt)}/>}
  </div>;
}
