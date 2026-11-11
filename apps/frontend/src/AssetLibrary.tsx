import { useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowLeft,
  CalendarDays,
  Download,
  Expand,
  FileText,
  Grid2X2,
  Image as ImageIcon,
  Images,
  List,
  LoaderCircle,
  MessageCircle,
  RefreshCw,
  X,
} from 'lucide-react';
import { api, assetUrl, native, type AssetLibraryItem, type AssetLibraryPage } from './studio-api';
import { ProductionRecord } from './ProductionRecord';
import {
  assetCountLabel,
  assetLibraryRequestKey,
  buildAssetQuery,
  formatTime,
  hasAssetFilters,
  modelLabel,
} from './library-state';

type Props = {
  active: boolean;
  onBack: () => void;
  onOpenAsset: (item: AssetLibraryItem) => void;
  onError: (value: string) => void;
};

const emptyFilters = { modelId: '', conversationId: '', size: '', from: '', to: '' };

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
  const requestSeq = useRef(0);
  const filterKey = useMemo(() => JSON.stringify(filters), [filters]);
  const filterKeyRef = useRef(filterKey);
  filterKeyRef.current = filterKey;
  const filtered = hasAssetFilters(filters);
  const requestKey = useMemo(
    () => assetLibraryRequestKey(active, filterKey, refreshNonce),
    [active, filterKey, refreshNonce],
  );

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
        setItems(result.items);
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
        return [...current, ...next.items.filter(item => !known.has(item.id))];
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

  return <section className="detail-page library-page works-page" aria-labelledby="works-page-title">
    <header className="library-heading">
      <div><span className="library-kicker">正式 AssetRef · 跨会话</span><h1 id="works-page-title">作品</h1><p>只浏览已提交的正式图片；示例、候选和临时文件不会进入这里。</p></div>
      <div className="library-heading-actions"><button className="secondary" onClick={() => setRefreshNonce(value => value + 1)}><RefreshCw size={14}/>刷新</button><button className="secondary" onClick={onBack}><ArrowLeft size={14}/>返回创作</button></div>
    </header>

    <div className="works-toolbar">
      <div className="works-filter-row">
        <label>型号<select value={filters.modelId} onChange={event => setFilters(current => ({ ...current, modelId: event.target.value }))}><option value="">全部型号</option>{page?.facets.models.map(item => <option key={item.id} value={item.id}>{modelLabel(item.id)} · {item.count}</option>)}</select></label>
        <label>尺寸<select value={filters.size} onChange={event => setFilters(current => ({ ...current, size: event.target.value }))}><option value="">全部尺寸</option>{page?.facets.sizes.map(item => <option key={`${item.width}x${item.height}`} value={`${item.width}x${item.height}`}>{item.width}×{item.height} · {item.count}</option>)}</select></label>
        <label>会话<select value={filters.conversationId} onChange={event => setFilters(current => ({ ...current, conversationId: event.target.value }))}><option value="">全部会话</option>{page?.facets.conversations.map(item => <option key={item.id} value={item.id}>{item.title} · {item.count}</option>)}</select></label>
        <label className="date-filter"><CalendarDays size={13}/><input type="date" aria-label="作品开始日期" value={filters.from} onChange={event => setFilters(current => ({ ...current, from: event.target.value }))}/></label>
        <span className="date-separator">至</span>
        <label className="date-filter"><input type="date" aria-label="作品结束日期" value={filters.to} onChange={event => setFilters(current => ({ ...current, to: event.target.value }))}/></label>
        {filtered && <button className="filter-clear" onClick={clearFilters}>清除筛选</button>}
      </div>
      <div className="works-view-row"><span>{assetCountLabel(active, loading, page?.total ?? null, Boolean(error))}</span><div className="segmented" role="group" aria-label="作品布局"><button className={viewMode === 'grid' ? 'active' : ''} aria-pressed={viewMode === 'grid'} onClick={() => setViewMode('grid')}><Grid2X2 size={14}/>网格</button><button className={viewMode === 'list' ? 'active' : ''} aria-pressed={viewMode === 'list'} onClick={() => setViewMode('list')}><List size={14}/>列表</button></div></div>
    </div>

    {error && <div className="library-error" role="alert"><span>{error}</span><button onClick={() => setRefreshNonce(value => value + 1)}>重试</button></div>}
    {loading && !items.length && <div className="library-loading" role="status"><LoaderCircle size={16} className="spin"/>正在读取正式作品…</div>}
    {!loading && !error && items.length === 0 && <div className="library-empty"><div className="empty-icon"><Images size={28}/></div><h2>{filtered ? '当前条件没有作品' : page?.facets.conversations.length === 0 ? '尚无正式作品' : '尚无正式作品'}</h2><p>{filtered ? '已保留当前筛选条件，可以清除后查看全部正式作品。' : '完成一次图片生成后，成功提交的 AssetRef 会出现在这里。示例不会计入。'}</p>{filtered ? <button className="secondary" onClick={clearFilters}>清除筛选</button> : <button className="secondary" onClick={onBack}>去创作</button>}</div>}

    {items.length > 0 && <div className={viewMode === 'grid' ? 'works-grid' : 'works-list'} role="list" aria-label="正式作品">
      {items.map((item, index) => <WorkCard key={item.id} item={item} index={index} failed={failedImages.has(item.id)} onFail={() => setFailedImages(current => new Set(current).add(item.id))} onOpen={() => setViewing(item)} viewMode={viewMode}/>) }
    </div>}
    {page?.hasMore && <div className="library-pagination"><button className="secondary" disabled={loadingMore} onClick={() => void loadMore()}>{loadingMore ? <LoaderCircle size={13} className="spin"/> : null}加载更多较早作品</button></div>}

    {viewing && <div className="works-preview" role="dialog" aria-modal="true" aria-label="作品预览">
      <header><div><span>{viewing.conversationTitle}</span><strong>{formatTime(viewing.createdAt)}</strong></div><button aria-label="关闭作品预览" onClick={() => { setViewing(null); setRecordOpen(false); }}><X size={17}/></button></header>
      <div className="works-preview-body">
        <div className="works-preview-image">{viewing.available ? <img src={assetUrl(viewing.id)} alt={viewing.prompt}/> : <div className="resource-error"><ImageIcon size={28}/><h3>图片资源不可用</h3><p>{viewing.availabilityError || 'PNG 可能已被移动、改写或无法解码。'}</p></div>}</div>
        <aside><span className="library-kicker">制作信息</span><h2>{viewing.prompt}</h2><dl><div><dt>会话</dt><dd>{viewing.conversationTitle}</dd></div><div><dt>型号</dt><dd>{modelLabel(viewing.modelId)}</dd></div><div><dt>尺寸</dt><dd>{viewing.width}×{viewing.height}</dd></div><div><dt>种子</dt><dd>{viewing.seed ?? '随机'}</dd></div><div><dt>完成时间</dt><dd>{formatTime(viewing.createdAt)}</dd></div><div><dt>AssetRef</dt><dd className="mono">{viewing.id}</dd></div></dl>
          {exportMessage && <p className="export-message" role="status">{exportMessage}</p>}
          <div className="preview-actions"><button className="primary-action" disabled={!viewing.available || exporting} onClick={() => void exportItem(viewing)}>{exporting ? <LoaderCircle size={14} className="spin"/> : <Download size={14}/>}导出 PNG</button><button className="secondary" aria-expanded={recordOpen} onClick={() => setRecordOpen(value => !value)}><FileText size={14}/>{recordOpen ? '收起记录' : '制作记录'}</button>{viewing.conversationId ? <button className="secondary" onClick={() => onOpenAsset(viewing)}><MessageCircle size={14}/>返回来源会话</button> : <span className="historical-source">历史生成 · 无可靠来源会话</span>}</div>
          {recordOpen && <ProductionRecord jobId={viewing.jobId} onError={onError}/>}
        </aside>
      </div>
    </div>}
  </section>;
}

function WorkCard({ item, index, failed, onFail, onOpen, viewMode }: {
  item: AssetLibraryItem;
  index: number;
  failed: boolean;
  onFail: () => void;
  onOpen: () => void;
  viewMode: 'grid' | 'list';
}) {
  return <article className="work-card" role="listitem">
    <button className="work-card-image" onClick={onOpen} aria-label={`预览作品 ${index + 1}：${item.prompt}`}>
      {item.available && !failed ? <img loading="lazy" decoding="async" src={assetUrl(item.id)} alt={item.prompt} onError={onFail}/> : <span className="work-image-error"><ImageIcon size={23}/><strong>{item.available ? '解码失败' : '资源缺失'}</strong><small>{item.availabilityError || '点击查看来源信息'}</small></span>}
    </button>
    <div className="work-card-body"><div><strong>{item.prompt}</strong><span>{item.conversationTitle}</span></div><p>{modelLabel(item.modelId)} · {item.width}×{item.height} · {formatTime(item.createdAt)}</p><div className="work-card-actions"><button onClick={onOpen}><Expand size={12}/>预览</button><span>{item.available ? '可用' : '资源异常'}</span></div></div>
    {viewMode === 'list' && <span className="work-list-index">{String(index + 1).padStart(2, '0')}</span>}
  </article>;
}
