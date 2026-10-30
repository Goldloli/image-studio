import { useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowLeft,
  ChevronDown,
  ChevronRight,
  FileText,
  Image as ImageIcon,
  ListTodo,
  LoaderCircle,
  MessageCircle,
  Play,
  RefreshCw,
  RotateCcw,
  Search,
  Square,
} from 'lucide-react';
import {
  api,
  assetUrl,
  native,
  stateLabel,
  type StudioSnapshot,
  type TaskCenterDetail,
  type TaskCenterItem,
  type TaskCenterPage,
  type TaskStatus,
} from './studio-api';
import { formatTime, modelLabel, taskStatusOptions } from './library-state';

type Props = {
  studio: StudioSnapshot | null;
  focusJob: { id: string; nonce: number } | null;
  onRefresh: () => void;
  onBack: () => void;
  onError: (value: string) => void;
  onOpenAsset: (task: TaskCenterItem) => void;
  onOpenConversation: (task: TaskCenterItem) => void;
};

const activeStates = ['Submitted', 'Preparing', 'Running', 'Committing', 'CancelRequested'];
const retryStates = ['Failed', 'Cancelled', 'Interrupted'];

export function TaskLibrary({ studio, focusJob, onRefresh, onBack, onError, onOpenAsset, onOpenConversation }: Props) {
  const [status, setStatus] = useState<TaskStatus>('all');
  const [search, setSearch] = useState('');
  const [conversationId, setConversationId] = useState('');
  const [modelId, setModelId] = useState('');
  const [page, setPage] = useState<TaskCenterPage | null>(null);
  const [items, setItems] = useState<TaskCenterItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [listError, setListError] = useState('');
  const [refreshNonce, setRefreshNonce] = useState(0);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<TaskCenterDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState('');
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [detailRevision, setDetailRevision] = useState(0);
  const [pendingAction, setPendingAction] = useState<string | null>(null);
  const [imageFailures, setImageFailures] = useState<Set<string>>(() => new Set());
  const requestSeq = useRef(0);
  const loadedQueryKey = useRef<string | null>(null);
  const queryKey = useMemo(() => JSON.stringify({ status, search: search.trim(), conversationId, modelId }), [status, search, conversationId, modelId]);
  const queryKeyRef = useRef(queryKey);
  queryKeyRef.current = queryKey;

  useEffect(() => {
    if (!focusJob) return;
    setStatus('all');
    setSearch('');
    setConversationId('');
    setModelId('');
    setSelectedId(focusJob.id);
  }, [focusJob?.nonce]);

  useEffect(() => {
    if (!native) {
      setListError('任务中心仅在桌面应用中读取本地任务。');
      return;
    }
    const sequence = ++requestSeq.current;
    let disposed = false;
    let timer: number | undefined;
    const poll = async () => {
      if (sequence !== requestSeq.current || disposed) return;
      setLoading(true);
      try {
        const result = await api<TaskCenterPage>('task_center_query', {
          query: { status, search: search.trim(), conversationId: conversationId || null, modelId: modelId || null, limit: 30 },
        });
        if (sequence !== requestSeq.current || disposed) return;
        const preserveLoadedPages = loadedQueryKey.current === queryKey;
        loadedQueryKey.current = queryKey;
        setItems(current => preserveLoadedPages
          ? mergeTasks(current, result.items)
          : result.items);
        setPage(current => preserveLoadedPages && current
          ? { ...result, hasMore: current.hasMore, nextCursor: current.nextCursor }
          : result);
        setListError('');
        setSelectedId(current => preserveLoadedPages
          ? current ?? result.items[0]?.id ?? null
          : result.items[0]?.id ?? null);
        if (result.counts.active > 0) setDetailRevision(value => value + 1);
        const wait = result.counts.active > 0 ? 1500 : 5000;
        timer = window.setTimeout(() => void poll(), wait);
      } catch (cause) {
        if (sequence !== requestSeq.current || disposed) return;
        setListError(String(cause));
        onError(String(cause));
        timer = window.setTimeout(() => void poll(), 5000);
      } finally {
        if (sequence === requestSeq.current && !disposed) setLoading(false);
      }
    };
    const debounce = window.setTimeout(() => void poll(), 160);
    return () => {
      disposed = true;
      window.clearTimeout(debounce);
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [queryKey, refreshNonce, onError]);

  useEffect(() => {
    if (!selectedId || !native) {
      setDetail(null);
      return;
    }
    let disposed = false;
    setDetailLoading(true);
    setDetailError('');
    api<TaskCenterDetail>('task_center_detail', { jobId: selectedId })
      .then(value => { if (!disposed) setDetail(value); })
      .catch(cause => {
        if (disposed) return;
        setDetailError(String(cause));
        onError(String(cause));
      })
      .finally(() => { if (!disposed) setDetailLoading(false); });
    return () => { disposed = true; };
  }, [selectedId, refreshNonce, detailRevision, onError]);

  const loadMore = async () => {
    if (!page?.hasMore || !page.nextCursor || loadingMore) return;
    const sequence = requestSeq.current;
    const key = queryKey;
    setLoadingMore(true);
    try {
      const next = await api<TaskCenterPage>('task_center_query', {
        query: { status, search: search.trim(), conversationId: conversationId || null, modelId: modelId || null, cursor: page.nextCursor, limit: 30 },
      });
      if (sequence !== requestSeq.current || key !== queryKeyRef.current) return;
      setItems(current => mergeTasks(current, next.items));
      setPage(next);
    } catch (cause) {
      if (sequence === requestSeq.current) {
        setListError(String(cause));
        onError(String(cause));
      }
    } finally {
      if (sequence === requestSeq.current) setLoadingMore(false);
    }
  };

  const refresh = () => {
    setRefreshNonce(value => value + 1);
    onRefresh();
  };
  const runAction = async (task: TaskCenterItem, command: 'generation_cancel' | 'generation_retry') => {
    setPendingAction(task.id);
    setDetailError('');
    try {
      await api(command, { jobId: task.id });
      refresh();
    } catch (cause) {
      setDetailError(String(cause));
      onError(String(cause));
    } finally {
      setPendingAction(null);
    }
  };
  const clearFilters = () => {
    setSearch('');
    setConversationId('');
    setModelId('');
  };
  const selected = items.find(item => item.id === selectedId) ?? detail?.task ?? null;
  const filtered = Boolean(search.trim() || conversationId || modelId || status !== 'all');
  const counts = page?.counts ?? { total: 0, active: 0, succeeded: 0, failed: 0, cancelled: 0, interrupted: 0 };

  return <section className="detail-page library-page task-center" aria-labelledby="tasks-page-title">
    <header className="library-heading">
      <div><span className="library-kicker">持久 Job / Attempt 查询</span><h1 id="tasks-page-title">任务</h1><p>查询完整本地记录，先读状态与下一步，再按需展开 manifest 和日志。</p></div>
      <div className="library-heading-actions"><button className="secondary" onClick={refresh}><RefreshCw size={14}/>刷新</button><button className="secondary" onClick={onBack}><ArrowLeft size={14}/>返回创作</button></div>
    </header>

    <div className="task-status-tabs" role="tablist" aria-label="任务状态">
      {taskStatusOptions.map(option => <button key={option.id} role="tab" aria-selected={status === option.id} className={status === option.id ? 'active' : ''} onClick={() => setStatus(option.id)}>{option.label}<b>{counts[option.count]}</b></button>)}
    </div>
    <div className="task-query-bar">
      <label className="task-search"><Search size={14}/><input value={search} onChange={event => setSearch(event.target.value)} aria-label="搜索任务提示词、会话或型号" placeholder="搜索提示词、会话或型号"/></label>
      <select aria-label="按会话筛选" value={conversationId} onChange={event => setConversationId(event.target.value)}><option value="">全部会话</option>{studio?.conversations.map(conversation => <option key={conversation.id} value={conversation.id}>{conversation.title}</option>)}</select>
      <select aria-label="按型号筛选" value={modelId} onChange={event => setModelId(event.target.value)}><option value="">全部型号</option><option value="z-image-turbo-mflux-v1">Z-Image Turbo</option></select>
      {(search || conversationId || modelId) && <button className="filter-clear" onClick={clearFilters}>清除条件</button>}
    </div>

    {listError && <div className="library-error" role="alert"><span>{listError}</span><button onClick={refresh}>重试</button></div>}
    {loading && !items.length && <div className="library-loading" role="status"><LoaderCircle size={16} className="spin"/>正在查询任务…</div>}
    {!loading && !listError && !items.length && <div className="library-empty"><div className="empty-icon"><ListTodo size={28}/></div><h2>{filtered ? '当前条件没有任务' : '这里还没有任务'}</h2><p>{filtered ? '本地已有任务时，筛选无结果会保留条件，不会伪装成首次使用。' : '提交一次图片生成后，Job、Attempt 和正式结果会保存在本机。'}</p>{filtered ? <button className="secondary" onClick={() => { clearFilters(); setStatus('all'); }}>清除筛选</button> : <button className="secondary" onClick={onBack}><Play size={14}/>去创作</button>}</div>}

    {items.length > 0 && <div className="task-center-shell">
      <div className="task-center-list" aria-label="任务列表">
        {items.map(task => <TaskRow key={task.id} task={task} selected={selectedId === task.id} imageFailed={imageFailures.has(task.id)} onImageFail={() => setImageFailures(current => new Set(current).add(task.id))} onSelect={() => { setSelectedId(task.id); setAdvancedOpen(false); }}/>) }
        {page?.hasMore && <div className="library-pagination"><button className="secondary" disabled={loadingMore} onClick={() => void loadMore()}>{loadingMore && <LoaderCircle size={13} className="spin"/>}加载更早任务</button></div>}
      </div>
      <TaskDetail detail={detail} selected={selected} loading={detailLoading} error={detailError} advancedOpen={advancedOpen} onToggleAdvanced={() => setAdvancedOpen(value => !value)} pendingAction={pendingAction} onCancel={task => void runAction(task, 'generation_cancel')} onRetry={task => void runAction(task, 'generation_retry')} onAsset={onOpenAsset} onConversation={onOpenConversation} onRefresh={refresh}/>
    </div>}
  </section>;
}

function TaskRow({ task, selected, imageFailed, onImageFail, onSelect }: {
  task: TaskCenterItem;
  selected: boolean;
  imageFailed: boolean;
  onImageFail: () => void;
  onSelect: () => void;
}) {
  return <button className={`task-center-row ${selected ? 'selected' : ''}`} aria-pressed={selected} onClick={onSelect}>
    <span className="task-row-thumbnail">{task.assetId && task.assetAvailable && !imageFailed ? <img loading="lazy" src={assetUrl(task.assetId)} alt="结果缩略图" onError={onImageFail}/> : <ImageIcon size={20}/>}</span>
    <span className="task-row-main"><span className="task-row-heading"><strong>{task.prompt}</strong><em className={`state-${task.state}`}>{stateLabel(task.state)}</em></span><span>{modelLabel(task.modelId)} · {task.width}×{task.height} · {task.conversationTitle}</span><small>{formatTime(task.createdAt)} · 种子 {task.seed ?? '随机'}</small></span>
    <ChevronRight size={14}/>
  </button>;
}

function TaskDetail({ detail, selected, loading, error, advancedOpen, onToggleAdvanced, pendingAction, onCancel, onRetry, onAsset, onConversation, onRefresh }: {
  detail: TaskCenterDetail | null;
  selected: TaskCenterItem | null;
  loading: boolean;
  error: string;
  advancedOpen: boolean;
  onToggleAdvanced: () => void;
  pendingAction: string | null;
  onCancel: (task: TaskCenterItem) => void;
  onRetry: (task: TaskCenterItem) => void;
  onAsset: (task: TaskCenterItem) => void;
  onConversation: (task: TaskCenterItem) => void;
  onRefresh: () => void;
}) {
  const task = detail?.task ?? selected;
  if (!task) return <aside className="task-detail-pane library-empty"><h2>选择一个任务</h2><p>任务详情会显示请求、来源、Attempt 和可读恢复建议。</p></aside>;
  const active = activeStates.includes(task.state);
  return <aside className="task-detail-pane" aria-label="任务详情">
    <header><div><span className="library-kicker">{stateLabel(task.state)}</span><h2>{task.prompt}</h2></div>{loading && <LoaderCircle size={15} className="spin"/>}</header>
    <div className={`task-next-action ${task.state === 'Failed' || task.state === 'Interrupted' ? 'attention' : ''}`}><strong>下一步</strong><p>{detail?.nextAction ?? (task.error || '正在读取制作记录')}</p></div>
    <div className="task-detail-actions">
      {task.assetId && <button className="primary-action" disabled={!task.assetAvailable} onClick={() => onAsset(task)}><ImageIcon size={14}/>查看作品</button>}
      {task.conversationId && <button className="secondary" onClick={() => onConversation(task)}><MessageCircle size={14}/>回到所在会话</button>}
      {active && <button className="secondary danger-quiet" disabled={task.state === 'CancelRequested' || pendingAction === task.id} onClick={() => onCancel(task)}><Square size={12}/>{task.state === 'CancelRequested' ? '正在取消' : '取消当前 Attempt'}</button>}
      {retryStates.includes(task.state) && <button className="secondary" disabled={pendingAction === task.id} onClick={() => onRetry(task)}>{pendingAction === task.id ? <LoaderCircle size={13} className="spin"/> : <RotateCcw size={13}/>}显式重试</button>}
    </div>
    {error && <div className="library-error" role="alert"><span>{error}</span><button onClick={onRefresh}>重试</button></div>}
    <section className="task-readable">
      <h3>请求</h3><dl><div><dt>会话</dt><dd>{task.conversationTitle}</dd></div><div><dt>型号</dt><dd>{modelLabel(task.modelId)}</dd></div><div><dt>尺寸</dt><dd>{detail ? `${detail.requestedWidth}×${detail.requestedHeight} 请求 · ${task.width}×${task.height} 实际` : `${task.width}×${task.height}`}</dd></div><div><dt>种子</dt><dd>{task.seed ?? '随机'}</dd></div><div><dt>创建</dt><dd>{formatTime(task.createdAt)}</dd></div><div><dt>完成</dt><dd>{formatTime(task.completedAt)}</dd></div><div><dt>Job</dt><dd className="mono">{task.id}</dd></div><div><dt>当前 Attempt</dt><dd className="mono">{task.attemptId}</dd></div></dl></section>
    {task.error && <section className="task-error-card"><h3>{detail?.errorCategory ? `错误 · ${errorCategoryLabel(detail.errorCategory)}` : '错误'}</h3><p>{task.error}</p>{task.assetError && <p>资源异常：{task.assetError}</p>}</section>}
    {detail && <section className="task-attempts"><h3>Attempt 记录</h3>{detail.attempts.map(attempt => <div key={attempt.attemptId}><span className={`task-attempt-dot state-${attempt.state}`}/><div><strong>{stateLabel(attempt.state)} · {formatTime(attempt.startedAt)}</strong><span className="mono">{attempt.attemptId}</span>{attempt.phase && <small>阶段 {attempt.phase}</small>}{attempt.error && <small className="error-text">{attempt.error}</small>}</div><time>{attempt.completedAt ? formatTime(attempt.completedAt) : activeStates.includes(attempt.state) ? '仍活动' : '时间未记录'}</time></div>)}</section>}
    <button className="advanced-toggle" aria-expanded={advancedOpen} onClick={onToggleAdvanced}><FileText size={13}/>{advancedOpen ? '收起完整制作记录' : '展开完整制作记录'}<ChevronDown size={13} className={advancedOpen ? 'rotated' : ''}/></button>
    {advancedOpen && <section className="task-advanced"><div><span>输出哈希</span><code>{detail?.advanced.outputHash ?? '尚无正式输出'}</code></div><h3>RunManifest</h3><pre>{detail?.advanced.manifest ? JSON.stringify(detail.advanced.manifest, null, 2) : '尚无完整 manifest'}</pre><h3>运行日志（最多 64 KiB）</h3><pre>{detail?.advanced.logs || '当前 Attempt 没有可读取日志。'}</pre></section>}
  </aside>;
}

function errorCategoryLabel(category: string): string {
  return {
    validation: '输入或参数',
    memory: '内存',
    runtime: '运行环境',
    cancel: '取消',
    interrupt: '宿主中断',
    storage: '存储',
    other: '其他执行',
  }[category] ?? category;
}

function mergeTasks(current: TaskCenterItem[], incoming: TaskCenterItem[]): TaskCenterItem[] {
  const byId = new Map(current.map(item => [item.id, item]));
  for (const item of incoming) byId.set(item.id, item);
  return [...byId.values()].sort((left, right) => right.createdAt - left.createdAt || right.id.localeCompare(left.id));
}
