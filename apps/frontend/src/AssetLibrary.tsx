import { useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowLeft,
  CalendarDays,
  Download,
  FileText,
  Grid2X2,
  Image as ImageIcon,
  Images,
  List,
  LoaderCircle,
  MessageCircle,
  RefreshCw,
  Star,
  Trash2,
  X,
} from 'lucide-react';
import { api, assetUrl, native, type AssetDeleteAck, type AssetLibraryItem, type AssetLibraryPage } from './studio-api';
import { ConfirmDialog } from './ConfirmDialog';
import { ProductionRecord } from './ProductionRecord';
import {
  assetCountLabel,
  assetLibraryRequestKey,
  buildAssetQuery,
  formatTime,
  hasAssetFilters,
  modelLabel,
  referenceConflictsMessage,
  workDeleteTitle,
  type AssetLibraryFilters,
} from './library-state';

type Props = {
  active: boolean;
  onBack: () => void;
  onOpenAsset: (item: AssetLibraryItem) => void;
  onError: (value: string) => void;
};

const emptyFilters: AssetLibraryFilters = { modelId: '', conversationId: '', size: '', from: '', to: '', favorite: 'all' };

export function AssetLibrary({ active, onBack, onOpenAsset, onError }: Props) {
  const [filters, setFilters] = useState(emptyFilters);
  const [page, setPage] = useState<AssetLibraryPage | null>(null);
  const [items, setItems] = useState<AssetLibraryItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState('');
  const [refreshNonce, setRefreshNonce] = useState(0);
  const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');
  const [viewing, setViewing] = useState<AssetLibraryItem | null>(null);
  const [recordOpen, setRecordOpen] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [failedImages, setFailedImages] = useState<Set<string>>(() => new Set());
  const [exportMessage, setExportMessage] = useState('');
  const [favoriteBusy, setFavoriteBusy] = useState<Set<string>>(() => new Set());
  const [deleteTarget, setDeleteTarget] = useState<AssetLibraryItem | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState('');
  const requestSeq = useRef(0);
  // Confirmed user state outranks any list response that started earlier, so a
  // slow query can never restore an old favourite or a deleted work.
  const favoriteOverrides = useRef(new Map<string, boolean>());
  const deletedIds = useRef(new Set<string>());
  const applyConfirmed = (incoming: AssetLibraryItem[]) => incoming
    .filter(item => !deletedIds.current.has(item.id))
    .map(item => favoriteOverrides.current.has(item.id)
      ? { ...item, isFavorite: favoriteOverrides.current.get(item.id)! }
      : item);
  const filterKey = useMemo(() => JSON.stringify(filters), [filters]);
  const filterKeyRef = useRef(filterKey);
  filterKeyRef.current = filterKey;
  const filtered = hasAssetFilters(filters);
  const requestKey = useMemo(
    () => assetLibraryRequestKey(active, filterKey, refreshNonce),
    [active, filterKey, refreshNonce],
  );
  const favoritesOnly = filters.favorite === 'favorites';
  const otherFilters = Boolean(filters.modelId || filters.conversationId || filters.size || filters.from || filters.to);

  useEffect(() => {
    if (!native) {
      setError('作品库仅在桌面应用中读取本地资产。');
      return;
    }
    const sequence = ++requestSeq.current;
    if (requestKey === null) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setError('');
    const timer = window.setTimeout(async () => {
      try {
        const result = await api<AssetLibraryPage>('asset_library_query', {
          query: buildAssetQuery(filters, null, 24),
        });
        if (sequence !== requestSeq.current) return;
        setPage(result);
        setItems(applyConfirmed(result.items));
        setViewing(current => current && result.items.some(item => item.id === current.id) ? current : null);
      } catch (cause) {
        if (sequence !== requestSeq.current) return;
        setError(String(cause));
        onError(String(cause));
      } finally {
        if (sequence === requestSeq.current) setLoading(false);
      }
    }, 180);
    return () => window.clearTimeout(timer);
  }, [requestKey, onError]);

  const loadMore = async () => {
    if (!page?.hasMore || !page.nextCursor || loadingMore) return;
    const sequence = requestSeq.current;
    const currentFilterKey = filterKey;
    setLoadingMore(true);
    setError('');
    try {
      const next = await api<AssetLibraryPage>('asset_library_query', {
        query: buildAssetQuery(filters, page.nextCursor, 24),
      });
      if (sequence !== requestSeq.current || currentFilterKey !== filterKeyRef.current) return;
      setItems(current => {
        const known = new Set(current.map(item => item.id));
        return [...current, ...applyConfirmed(next.items).filter(item => !known.has(item.id))];
      });
      setPage(next);
    } catch (cause) {
      if (sequence === requestSeq.current) {
        setError(String(cause));
        onError(String(cause));
      }
    } finally {
      if (sequence === requestSeq.current) setLoadingMore(false);
    }
  };

  const clearFilters = () => {
    setFilters(emptyFilters);
    setViewing(null);
  };
  const exportItem = async (item: AssetLibraryItem) => {
    if (exporting) return;
    setExporting(true);
    setExportMessage('');
    try {
      const result = await api<{ cancelled: boolean }>('asset_export', { assetId: item.id });
      setExportMessage(result.cancelled ? '已取消导出，原作品未改变。' : 'PNG 已保存到你选择的位置。');
    } catch (cause) {
      const message = String(cause);
      setExportMessage(`导出失败：${message}`);
      onError(message);
    } finally {
      setExporting(false);
    }
  };

  const refresh = () => setRefreshNonce(value => value + 1);
  const toggleFavorite = async (item: AssetLibraryItem) => {
    if (favoriteBusy.has(item.id) || item.deletionState !== 'active') return;
    const wanted = !item.isFavorite;
    setFavoriteBusy(previous => new Set(previous).add(item.id));
    try {
      const result = await api<{ assetId: string; favorite: boolean }>('asset_set_favorite', { assetId: item.id, favorite: wanted });
      favoriteOverrides.current.set(item.id, result.favorite);
      setItems(previous => previous.map(value => value.id === item.id ? { ...value, isFavorite: result.favorite } : value));
      // Leaving the favourites view after un-favouriting must re-query the whole
      // condition so the total and cursor set stay correct.
      if (filters.favorite === 'favorites' && !result.favorite) refresh();
    } catch (cause) {
      const message = String(cause);
      setError(message);
      onError(message);
      refresh();
    } finally {
      setFavoriteBusy(previous => {
        const next = new Set(previous);
        next.delete(item.id);
        return next;
      });
    }
  };
  const removeFromView = (id: string) => {
    deletedIds.current.add(id);
    setViewing(current => current && current.id === id ? null : current);
    refresh();
  };
  // Retrying an unfinished cleanup reuses the originally confirmed scope, so it
  // never asks again and never widens what is removed.
  const retryDelete = async (item: AssetLibraryItem) => {
    if (deleting) return;
    setDeleting(true);
    setDeleteError('');
    try {
      const result = await api<AssetDeleteAck>('asset_delete', { assetId: item.id });
      if (result.state === 'deleted') {
        removeFromView(item.id);
        return;
      }
      const message = result.state === 'conflict'
        ? referenceConflictsMessage(result.conflicts ?? [])
        : (result.error ?? '删除未完成，请稍后重试');
      setError(message);
      onError(message);
      refresh();
    } catch (cause) {
      const message = String(cause);
      setDeleteError(message);
      onError(message);
    } finally {
      setDeleting(false);
    }
  };
  const requestDelete = (item: AssetLibraryItem) => {
    if (item.deletionState !== 'active') {
      void retryDelete(item);
      return;
    }
    setDeleteError('');
    setDeleteTarget(item);
  };
  const confirmDelete = async () => {
    const target = deleteTarget;
    if (!target || deleting) return;
    setDeleting(true);
    setDeleteError('');
    try {
      const result = await api<AssetDeleteAck>('asset_delete', { assetId: target.id });
      if (result.state === 'deleted') {
        setDeleteTarget(null);
        removeFromView(target.id);
        return;
      }
      if (result.state === 'conflict') {
        setDeleteError(referenceConflictsMessage(result.conflicts ?? []));
        return;
      }
      setDeleteError(result.error ?? '删除未完成，请稍后重试');
      refresh();
    } catch (cause) {
      setDeleteError(String(cause));
    } finally {
      setDeleting(false);
    }
  };
  return <section className="detail-page library-page works-page" aria-labelledby="works-page-title">
    <header className="library-heading">
      <div><h1 id="works-page-title">作品</h1></div>
      <div className="library-heading-actions"><button className="secondary" onClick={() => setRefreshNonce(value => value + 1)}><RefreshCw size={14}/>刷新</button><button className="secondary" onClick={onBack}><ArrowLeft size={14}/>返回创作</button></div>
    </header>

    <div className="works-toolbar">
      <div className="works-filter-row">
        <label>型号<select value={filters.modelId} onChange={event => setFilters(current => ({ ...current, modelId: event.target.value }))}><option value="">全部型号</option>{page?.facets.models.map(item => <option key={item.id} value={item.id}>{modelLabel(item.id)} · {item.count}</option>)}</select></label>
        <label>尺寸<select value={filters.size} onChange={event => setFilters(current => ({ ...current, size: event.target.value }))}><option value="">全部尺寸</option>{page?.facets.sizes.map(item => <option key={`${item.width}x${item.height}`} value={`${item.width}x${item.height}`}>{item.width}×{item.height} · {item.count}</option>)}</select></label>
        <label>会话<select value={filters.conversationId} onChange={event => setFilters(current => ({ ...current, conversationId: event.target.value }))}><option value="">全部会话</option>{page?.facets.conversations.map(item => <option key={item.id} value={item.id}>{item.title}{item.deleted ? '（已删除）' : ''} · {item.count}</option>)}</select></label>
        <div className="date-range">
          <label className="date-filter"><CalendarDays size={13}/><input type="date" aria-label="作品开始日期" value={filters.from} onChange={event => setFilters(current => ({ ...current, from: event.target.value }))}/></label>
          <span className="date-separator">至</span>
          <label className="date-filter"><input type="date" aria-label="作品结束日期" value={filters.to} onChange={event => setFilters(current => ({ ...current, to: event.target.value }))}/></label>
        </div>
        <div className="segmented works-favorite-filter" role="group" aria-label="收藏筛选"><button className={filters.favorite === 'all' ? 'active' : ''} aria-pressed={filters.favorite === 'all'} onClick={() => setFilters(current => ({ ...current, favorite: 'all' }))}>全部作品</button><button className={filters.favorite === 'favorites' ? 'active' : ''} aria-pressed={filters.favorite === 'favorites'} onClick={() => setFilters(current => ({ ...current, favorite: 'favorites' }))}><Star size={12}/>仅收藏</button></div>
        {filtered && <button className="filter-clear" onClick={clearFilters}>清除筛选</button>}
      </div>
      <div className="works-view-row"><span>{assetCountLabel(active, loading, page?.total ?? null, Boolean(error))}</span><div className="segmented" role="group" aria-label="作品布局"><button className={viewMode === 'grid' ? 'active' : ''} aria-pressed={viewMode === 'grid'} onClick={() => setViewMode('grid')}><Grid2X2 size={14}/>网格</button><button className={viewMode === 'list' ? 'active' : ''} aria-pressed={viewMode === 'list'} onClick={() => setViewMode('list')}><List size={14}/>列表</button></div></div>
    </div>

    {error && <div className="library-error" role="alert"><span>{error}</span><button onClick={() => setRefreshNonce(value => value + 1)}>重试</button></div>}
    {loading && !items.length && <div className="library-loading" role="status"><LoaderCircle size={16} className="spin"/>正在读取正式作品…</div>}
    {!loading && !error && items.length === 0 && <div className="library-empty"><div className="empty-icon"><Images size={28}/></div><h2>{favoritesOnly && !otherFilters ? '尚无收藏作品' : filtered ? '当前条件没有作品' : '尚无正式作品'}</h2><p>{favoritesOnly && !otherFilters ? '在作品卡片右下角收藏作品；收藏与灵感案例的收藏相互独立。' : filtered ? '已保留当前筛选条件，可以清除后查看全部正式作品。' : '完成一次图片生成后，成功提交的 AssetRef 会出现在这里。示例不会计入。'}</p>{favoritesOnly && !otherFilters ? <button className="secondary" onClick={() => setFilters(current => ({ ...current, favorite: 'all' }))}>查看全部作品</button> : filtered ? <button className="secondary" onClick={clearFilters}>清除筛选</button> : <button className="secondary" onClick={onBack}>去创作</button>}</div>}

    {items.length > 0 && <div className={viewMode === 'grid' ? 'works-grid' : 'works-list'} role="list" aria-label="正式作品">
      {items.map((item, index) => <WorkCard key={item.id} item={item} index={index} failed={failedImages.has(item.id)} onFail={() => setFailedImages(current => new Set(current).add(item.id))} onOpen={() => setViewing(item)} viewMode={viewMode} deleting={deleting} favoriteBusy={favoriteBusy.has(item.id)} onFavorite={() => void toggleFavorite(item)} onDelete={() => requestDelete(item)}/>) }
    </div>}
    {page?.hasMore && <div className="library-pagination"><button className="secondary" disabled={loadingMore} onClick={() => void loadMore()}>{loadingMore ? <LoaderCircle size={13} className="spin"/> : null}加载更多较早作品</button></div>}

    {viewing && <div className="works-preview" role="dialog" aria-modal="true" aria-label="作品预览">
      <header><span>作品预览</span><button aria-label="关闭作品预览" onClick={() => { setViewing(null); setRecordOpen(false); }}><X size={17}/></button></header>
      <div className="works-preview-body">
        <div className="works-preview-image">{viewing.available ? <img src={assetUrl(viewing.id)} alt={viewing.prompt}/> : <div className="resource-error"><ImageIcon size={28}/><h3>图片资源不可用</h3><p>{viewing.availabilityError || 'PNG 可能已被移动、改写或无法解码。'}</p></div>}</div>
        <aside><h2>{viewing.prompt}</h2><dl><div><dt>会话</dt><dd>{viewing.conversationTitle}{viewing.conversationDeleted ? ' · 已删除' : ''}</dd></div><div><dt>型号</dt><dd>{modelLabel(viewing.modelId)}</dd></div><div><dt>尺寸</dt><dd>{viewing.width}×{viewing.height}</dd></div><div><dt>种子</dt><dd>{viewing.seed ?? '随机'}</dd></div><div><dt>完成时间</dt><dd>{formatTime(viewing.createdAt)}</dd></div></dl>
          <details className="works-preview-technical"><summary>技术信息</summary><div><span>AssetRef</span><code>{viewing.id}</code></div></details>
          {exportMessage && <p className="export-message" role="status">{exportMessage}</p>}
          <div className="preview-actions"><button className="primary-action" disabled={!viewing.available || exporting} onClick={() => void exportItem(viewing)}>{exporting ? <LoaderCircle size={14} className="spin"/> : <Download size={14}/>}导出 PNG</button><button className="secondary" aria-expanded={recordOpen} onClick={() => setRecordOpen(value => !value)}><FileText size={14}/>{recordOpen ? '收起记录' : '制作记录'}</button>{viewing.conversationDeleted ? <span className="historical-source">来源会话已删除 · 作品保留</span> : viewing.conversationId ? <button className="secondary" onClick={() => onOpenAsset(viewing)}><MessageCircle size={14}/>返回来源会话</button> : <span className="historical-source">历史生成 · 无可靠来源会话</span>}</div>
          {recordOpen && <ProductionRecord jobId={viewing.jobId} onError={onError} showSummary={false}/>}
        </aside>
      </div>
    </div>}
    {deleteTarget && <ConfirmDialog
      title={`删除作品「${workDeleteTitle(deleteTarget)}」？`}
      description="应用内原图、同次生成的配套蒙版及专用预览缓存会删除；已导出的外部副本保留。此操作不可撤销。"
      confirmLabel="删除作品"
      busy={deleting}
      error={deleteError}
      onCancel={() => { if (!deleting) { setDeleteTarget(null); setDeleteError(''); } }}
      onConfirm={() => void confirmDelete()}
    />}
  </section>;
}

function WorkCard({ item, index, failed, onFail, onOpen, viewMode, deleting, favoriteBusy, onFavorite, onDelete }: {
  item: AssetLibraryItem;
  index: number;
  failed: boolean;
  onFail: () => void;
  onOpen: () => void;
  viewMode: 'grid' | 'list';
  deleting: boolean;
  favoriteBusy: boolean;
  onFavorite: () => void;
  onDelete: () => void;
}) {
  const manageable = item.deletionState === 'active';
  const status = !manageable
    ? item.deletionState === 'pending' ? '正在删除' : '删除未完成'
    : item.available ? '可用' : '资源异常';
  const promptText = item.prompt.trim();
  const conversationText = item.conversationTitle.trim();
  const conversationRepeatsPrompt = promptText.startsWith(conversationText) || conversationText.startsWith(promptText);
  const conversationLabel = conversationRepeatsPrompt
    ? ''
    : `${item.conversationTitle}${item.conversationDeleted ? ' · 会话已删除' : ''}`;
  return <article className={`work-card ${manageable ? '' : 'is-managing'}`} role="listitem">
    <button className="work-card-image" disabled={!manageable} onClick={onOpen} aria-label={manageable ? `预览作品 ${index + 1}：${item.prompt}` : `作品${status}`}>
      {item.available && !failed && manageable ? <img loading="lazy" decoding="async" src={assetUrl(item.id)} alt={item.prompt} onError={onFail}/> : <span className="work-image-error"><ImageIcon size={23}/><strong>{manageable ? (item.available ? '解码失败' : '资源缺失') : status}</strong><small>{item.deletionState === 'failed' ? (item.deletionError || '可在右下角重试删除') : manageable ? (item.availabilityError || '点击查看来源信息') : (item.deletionError || '清理完成后自动更新')}</small></span>}
    </button>
    <div className="work-card-body">
      <div><strong>{item.prompt}</strong>{conversationLabel && <span>{conversationLabel}</span>}</div>
      <p>{modelLabel(item.modelId)} · {item.width}×{item.height} · {formatTime(item.createdAt)}</p>
      {item.deletionState === 'failed' && <p className="work-deletion-state" role="status">{item.deletionError || '删除未完成，可重试'}</p>}
      <div className="work-card-manage">
        <button className={`work-favorite ${item.isFavorite ? 'is-favorite' : ''}`} disabled={!manageable || favoriteBusy} aria-pressed={item.isFavorite} aria-label={item.isFavorite ? '取消收藏' : '收藏作品'} title={item.isFavorite ? '取消收藏' : '收藏作品'} onClick={onFavorite}><Star size={13} fill={item.isFavorite ? 'currentColor' : 'none'}/></button>
        <button className="work-delete" disabled={deleting} aria-label={manageable ? '删除作品' : '重试删除'} title={manageable ? '删除作品' : '重试删除'} onClick={onDelete}><Trash2 size={13}/></button>
      </div>
    </div>
    {viewMode === 'list' && <span className="work-list-index">{String(index + 1).padStart(2, '0')}</span>}
  </article>;
}
