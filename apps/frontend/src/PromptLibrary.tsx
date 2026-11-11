import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowLeft, ArrowUp, Check, ChevronDown, Copy, ExternalLink, Heart,
  Image as ImageIcon, LoaderCircle, RefreshCw, Search, SlidersHorizontal,
  Sparkles, Trash2, X,
} from 'lucide-react';
import { api, bytesLabel, native, promptImageUrl } from './studio-api';
import {
  emptyPromptQuery, filterPromptCases, missingFavoriteIds, promptUsageLabel,
  toggleSelection, PROMPT_LIBRARY_UPSTREAM_COMMIT, PROMPT_LIBRARY_UPSTREAM_REPOSITORY,
  type PromptCase, type PromptCatalog, type PromptLibrarySnapshot, type PromptQuery,
} from './prompt-library-state';
import './prompt-library.css';

export type PromptUseRequest = {
  caseId: string;
  title: string;
  prompt: string;
  target: 'new' | 'current';
};

type ImageState = { status: 'idle' | 'loading' | 'loaded' | 'error'; error?: string };
type PromptTab = PromptQuery['view'];

const emptyCatalog: PromptCatalog = {
  schemaVersion: 1,
  catalogVersion: '',
  upstreamRepository: PROMPT_LIBRARY_UPSTREAM_REPOSITORY,
  upstreamCommit: PROMPT_LIBRARY_UPSTREAM_COMMIT,
  importedAt: '',
  sourceCount: 0,
  importedCount: 0,
  categories: [],
  styles: [],
  scenes: [],
  sourceRights: {},
  imageCachePolicy: { bundled: false, maxFileBytes: 0, maxCacheBytes: 0, maxPixels: 0, allowedMimeTypes: [] },
  cases: [],
};

const previewSnapshot = (catalog: PromptCatalog): PromptLibrarySnapshot => ({
  catalog,
  favorites: readPreviewFavorites(),
  favoritesLoadError: null,
  favoritesSaveError: null,
  cache: { bytes: 0, count: 0, limitBytes: 268435456 },
});

function readPreviewFavorites(): string[] {
  try {
    const value = JSON.parse(localStorage.getItem('image-studio-preview-prompt-favorites') ?? '[]');
    return Array.isArray(value) ? value.filter(item => typeof item === 'string') : [];
  } catch {
    return [];
  }
}

function writePreviewFavorites(ids: string[]) {
  localStorage.setItem('image-studio-preview-prompt-favorites', JSON.stringify(ids));
}

function previewImageUrl(item: PromptCase) {
  return `https://raw.githubusercontent.com/freestylefly/awesome-gpt-image-2/${PROMPT_LIBRARY_UPSTREAM_COMMIT}/${item.image.path}`;
}

function PromptImage({
  item, active, state, onLoad, onImageError, compact = false, allowRetry = true,
}: {
  item: PromptCase;
  active: boolean;
  state: ImageState | undefined;
  onLoad: () => void;
  onImageError: () => void;
  compact?: boolean;
  allowRetry?: boolean;
}) {
  const holder = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!active || state?.status === 'loaded' || state?.status === 'loading' || state?.status === 'error') return;
    const element = holder.current;
    if (!element) return;
    const observer = new IntersectionObserver(entries => {
      if (entries.some(entry => entry.isIntersecting)) onLoad();
    }, { rootMargin: '240px 0px' });
    observer.observe(element);
    return () => observer.disconnect();
  }, [active, onLoad, state?.status]);
  const loaded = state?.status === 'loaded';
  return <div ref={holder} className={`prompt-image-frame ${compact ? 'is-compact' : ''}`} style={{ aspectRatio: `${item.image.width} / ${item.image.height}` }}>
    {loaded && <img src={native ? promptImageUrl(item.sourceCaseId) : previewImageUrl(item)} alt={item.imageAlt} loading="lazy" onError={onImageError}/>}
    {!loaded && <div className={`prompt-image-placeholder ${state?.status === 'error' ? 'is-error' : ''}`}>
      {state?.status === 'loading' ? <LoaderCircle size={20} className="spin"/> : <ImageIcon size={21}/>}
      <span>{state?.status === 'error' ? '图片加载失败' : state?.status === 'loading' ? '正在加载示例图' : '联网后加载示例图'}</span>
      {state?.status === 'error' && (allowRetry ? <button type="button" onClick={onLoad}><RefreshCw size={11}/>重试</button> : <span>打开详情后重试</span>)}
    </div>}
    <span className="prompt-example-badge">上游示例</span>
  </div>;
}

function PromptCard({
  item, favorite, active, image, onOpen, onFavorite, onLoadImage, onImageError,
}: {
  item: PromptCase;
  favorite: boolean;
  active: boolean;
  image: ImageState | undefined;
  onOpen: () => void;
  onFavorite: () => void;
  onLoadImage: () => void;
  onImageError: () => void;
}) {
  return <article className="prompt-card">
    <button className="prompt-card-main" type="button" onClick={onOpen} aria-label={`查看${item.title}`}>
      <PromptImage item={item} active={active} state={image} onLoad={onLoadImage} onImageError={onImageError} allowRetry={false}/>
      <div className="prompt-card-copy">
        <span>{item.category.label}</span>
        <strong>{item.title}</strong>
        <small>{promptUsageLabel(item)}</small>
      </div>
    </button>
    <button className={`prompt-favorite ${favorite ? 'is-favorite' : ''}`} type="button" aria-label={favorite ? `取消收藏${item.title}` : `收藏${item.title}`} aria-pressed={favorite} onClick={onFavorite}>
      <Heart size={14} fill={favorite ? 'currentColor' : 'none'}/>
    </button>
  </article>;
}

export function PromptLibrary({
  active, onBack, onUsePrompt, onError,
}: {
  active: boolean;
  onBack: () => void;
  onUsePrompt: (request: PromptUseRequest) => void;
  onError: (value: string) => void;
}) {
  const [snapshot, setSnapshot] = useState<PromptLibrarySnapshot>(() => ({
    catalog: emptyCatalog,
    favorites: native ? [] : readPreviewFavorites(),
    favoritesLoadError: null,
    favoritesSaveError: null,
    cache: { bytes: 0, count: 0, limitBytes: 268435456 },
  }));
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState('');
  const [query, setQuery] = useState<PromptQuery>(emptyPromptQuery);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const [cacheArmed, setCacheArmed] = useState(false);
  const [images, setImages] = useState<Record<string, ImageState>>({});
  const requests = useRef(new Set<string>());
  const detailDialog = useRef<HTMLElement>(null);
  const detailClose = useRef<HTMLButtonElement>(null);
  const catalog = snapshot.catalog;
  const favoriteSet = useMemo(() => new Set(snapshot.favorites), [snapshot.favorites]);
  const cases = useMemo(() => filterPromptCases(catalog.cases, query, favoriteSet), [catalog.cases, query, favoriteSet]);
  const selected = selectedId ? catalog.cases.find(item => item.id === selectedId) ?? null : null;
  const missingFavorites = missingFavoriteIds(snapshot.favorites, catalog);

  useEffect(() => {
    let disposed = false;
    setLoading(true);
    const request = native
      ? api<PromptLibrarySnapshot>('prompt_library_snapshot')
      : import('../../../resources/prompt-library/catalog.json').then(module => previewSnapshot(module.default as PromptCatalog));
    request
      .then(value => {
        if (disposed) return;
        setSnapshot(value);
        setLoadError('');
      })
      .catch(error => { if (!disposed) setLoadError(String(error)); })
      .finally(() => { if (!disposed) setLoading(false); });
    return () => { disposed = true; };
  }, []);

  useEffect(() => {
    if (!selected || !active) return;
    const close = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !event.isComposing) setSelectedId(null);
      if (event.key !== 'Tab' || event.isComposing || !detailDialog.current) return;
      const focusable = [...detailDialog.current.querySelectorAll<HTMLElement>('button:not(:disabled),a[href],select:not(:disabled),input:not(:disabled),textarea:not(:disabled),[tabindex]:not([tabindex="-1"])')];
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener('keydown', close);
    requestAnimationFrame(() => detailClose.current?.focus());
    return () => window.removeEventListener('keydown', close);
  }, [active, selected]);

  useEffect(() => {
    if (!cacheArmed) return;
    const timer = window.setTimeout(() => setCacheArmed(false), 2600);
    return () => window.clearTimeout(timer);
  }, [cacheArmed]);

  const loadImage = useCallback((item: PromptCase) => {
    if (!active || requests.current.has(item.id) || images[item.id]?.status === 'loaded') return;
    requests.current.add(item.id);
    setImages(previous => ({ ...previous, [item.id]: { status: 'loading' } }));
    if (!native) {
      requests.current.delete(item.id);
      setImages(previous => ({ ...previous, [item.id]: { status: 'loaded' } }));
      return;
    }
    api<{ cache: PromptLibrarySnapshot['cache'] }>('prompt_library_image', { caseId: item.id })
      .then(result => {
        setImages(previous => ({ ...previous, [item.id]: { status: 'loaded' } }));
        setSnapshot(current => ({ ...current, cache: result.cache }));
      })
      .catch(error => {
        setImages(previous => ({ ...previous, [item.id]: { status: 'error', error: String(error) } }));
      })
      .finally(() => requests.current.delete(item.id));
  }, [active, images, native]);

  const favorite = async (item: PromptCase) => {
    const nextFavorite = !favoriteSet.has(item.id);
    const previous = snapshot.favorites;
    const next = nextFavorite ? [...previous, item.id] : previous.filter(id => id !== item.id);
    setSnapshot(current => ({ ...current, favorites: next }));
    try {
      if (native) {
        const result = await api<{ favorites: string[]; favoritesSaveError: string | null }>('prompt_library_favorite', { caseId: item.id, favorite: nextFavorite });
        setSnapshot(current => ({ ...current, favorites: result.favorites, favoritesSaveError: result.favoritesSaveError }));
      } else {
        writePreviewFavorites(next);
      }
    } catch (error) {
      setSnapshot(current => ({ ...current, favorites: previous }));
      onError(String(error));
    }
  };

  const removeMissingFavorite = async (id: string) => {
    const previous = snapshot.favorites;
    const next = previous.filter(value => value !== id);
    setSnapshot(current => ({ ...current, favorites: next }));
    try {
      if (native) {
        const result = await api<{ favorites: string[]; favoritesSaveError: string | null }>('prompt_library_favorite', { caseId: id, favorite: false });
        setSnapshot(current => ({ ...current, favorites: result.favorites, favoritesSaveError: result.favoritesSaveError }));
      } else {
        writePreviewFavorites(next);
      }
    } catch (error) {
      setSnapshot(current => ({ ...current, favorites: previous }));
      onError(String(error));
    }
  };

  const copyPrompt = async (item: PromptCase) => {
    try {
      await navigator.clipboard.writeText(item.prompt);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1400);
    } catch {
      onError('无法访问剪贴板，请手动选择完整提示词复制。');
    }
  };

  const clearCache = async () => {
    if (!cacheArmed) {
      setCacheArmed(true);
      return;
    }
    setCacheArmed(false);
    if (!native) {
      setSnapshot(current => ({ ...current, cache: { ...current.cache, bytes: 0, count: 0 } }));
      return;
    }
    try {
      const result = await api<{ cache: PromptLibrarySnapshot['cache'] }>('prompt_library_clear_cache');
      setSnapshot(current => ({ ...current, cache: result.cache }));
      onError('');
    } catch (error) {
      onError(String(error));
    }
  };

  const tab = (value: PromptTab, label: string, count: number) => <button className={query.view === value ? 'active' : ''} aria-pressed={query.view === value} onClick={() => setQuery(current => ({ ...current, view: value }))}>{label}<span>{count}</span></button>;

  return <section className="prompt-library-page" aria-label="灵感库">
    <header className="prompt-library-heading">
      <div>
        <button className="prompt-back" type="button" onClick={onBack}><ArrowLeft size={14}/>返回创作</button>
        <span className="library-kicker"><Sparkles size={12}/> PROMPT INSPIRATION</span>
        <h1>灵感库</h1>
        <p>看图找灵感，选一句开始创作。示例来自上游社区，不代表本机生成或模型验证结果。</p>
      </div>
      <div className="prompt-heading-actions">
        <span className="prompt-source-version">目录 {catalog.catalogVersion}<br/>{catalog.importedCount} 条 · 上游 {catalog.upstreamCommit.slice(0, 8)}</span>
        <button type="button" className="secondary" onClick={() => void clearCache()} title="清理由灵感库独立管理的图片缓存">
          <Trash2 size={13}/>{cacheArmed ? '再次点击确认' : `缓存 ${bytesLabel(snapshot.cache.bytes)}`}
        </button>
      </div>
    </header>
    {(loadError || snapshot.favoritesLoadError || snapshot.favoritesSaveError) && <div className="prompt-error" role="alert"><span>{loadError || snapshot.favoritesLoadError || snapshot.favoritesSaveError}</span><button type="button" onClick={() => setLoadError('')} aria-label="关闭提示"><X size={12}/></button></div>}
    {!native && <div className="prompt-preview-note">界面预览：收藏保存在浏览器本地；桌面应用会使用受限宿主缓存。</div>}

    <div className="prompt-toolbar">
      <label className="prompt-search"><Search size={14}/><input value={query.search} placeholder="搜索标题、完整提示词、分类或来源" onChange={event => setQuery(current => ({ ...current, search: event.target.value }))}/>{query.search && <button type="button" aria-label="清除搜索" onClick={() => setQuery(current => ({ ...current, search: '' }))}><X size={11}/></button>}</label>
      <div className="prompt-tabs" role="group" aria-label="案例视图">
        {tab('all', '全部', catalog.cases.length)}
        {tab('featured', '精选', catalog.cases.filter(item => item.featured).length)}
        {tab('favorites', '我的收藏', snapshot.favorites.length)}
      </div>
      <select className="prompt-compact-category" aria-label="主分类" value={query.categoryId} onChange={event => setQuery(current => ({ ...current, categoryId: event.target.value }))}><option value="">全部分类</option>{catalog.categories.map(category => <option key={category.id} value={category.id}>{category.label} · {category.count}</option>)}</select>
      <button className={`prompt-filter-button ${filtersOpen ? 'active' : ''}`} type="button" aria-expanded={filtersOpen} onClick={() => setFiltersOpen(value => !value)}><SlidersHorizontal size={13}/>风格与场景{(query.styles.length + query.scenes.length) ? ` · ${query.styles.length + query.scenes.length}` : ''}</button>
      <label className="prompt-sort"><span>排序</span><select value={query.sort} onChange={event => setQuery(current => ({ ...current, sort: event.target.value as PromptQuery['sort'] }))}><option value="default">精选优先</option><option value="id-desc">编号倒序</option><option value="id-asc">编号正序</option></select><ChevronDown size={11}/></label>
    </div>

    {filtersOpen && <div className="prompt-filters">
      <div><strong>风格</strong>{catalog.styles.map(style => <button key={style.sourceLabel} className={query.styles.includes(style.sourceLabel) ? 'active' : ''} aria-pressed={query.styles.includes(style.sourceLabel)} onClick={() => setQuery(current => ({ ...current, styles: toggleSelection(current.styles, style.sourceLabel) }))}>{style.label}</button>)}</div>
      <div><strong>场景</strong>{catalog.scenes.map(scene => <button key={scene.sourceLabel} className={query.scenes.includes(scene.sourceLabel) ? 'active' : ''} aria-pressed={query.scenes.includes(scene.sourceLabel)} onClick={() => setQuery(current => ({ ...current, scenes: toggleSelection(current.scenes, scene.sourceLabel) }))}>{scene.label}</button>)}</div>
      <button className="prompt-clear-filters" type="button" onClick={() => setQuery(emptyPromptQuery())}>清除筛选</button>
    </div>}

    <div className="prompt-library-body">
      <aside className="prompt-categories" aria-label="案例分类">
        <button className={!query.categoryId ? 'active' : ''} onClick={() => setQuery(current => ({ ...current, categoryId: '' }))}><span>全部分类</span><b>{catalog.cases.length}</b></button>
        {catalog.categories.map(category => <button key={category.id} className={query.categoryId === category.id ? 'active' : ''} onClick={() => setQuery(current => ({ ...current, categoryId: query.categoryId === category.id ? '' : category.id }))}><span>{category.label}</span><b>{category.count}</b></button>)}
      </aside>
      <div className="prompt-results">
        <div className="prompt-result-line"><span>{loading ? '正在读取目录' : `${cases.length} 个案例`}</span>{query.view === 'favorites' && missingFavorites.length > 0 && <small>{missingFavorites.length} 个已移除收藏以占位显示</small>}</div>
        {loading ? <div className="prompt-loading"><LoaderCircle size={18} className="spin"/>正在恢复灵感库</div> : cases.length === 0 ? <div className="prompt-empty"><ImageIcon size={26}/><h2>{query.view === 'favorites' ? '还没有匹配的收藏' : '没有匹配的案例'}</h2><p>可以清除关键词或筛选后继续浏览。</p><button className="secondary" onClick={() => setQuery(emptyPromptQuery())}>清除筛选</button></div> : <div className="prompt-grid">
          {cases.map(item => <PromptCard key={item.id} item={item} favorite={favoriteSet.has(item.id)} active={active} image={images[item.id]} onOpen={() => setSelectedId(item.id)} onFavorite={() => void favorite(item)} onLoadImage={() => loadImage(item)} onImageError={() => setImages(previous => ({ ...previous, [item.id]: { status: 'error', error: '图片解码失败' } }))}/>)}
        </div>}
        {query.view === 'favorites' && missingFavorites.length > 0 && <div className="prompt-removed-favorites"><strong>目录中已移除的收藏</strong>{missingFavorites.map(id => <div key={id}><span>{id}</span><button type="button" onClick={() => void removeMissingFavorite(id)}>取消收藏</button></div>)}</div>}
      </div>
    </div>

    {selected && <div className="prompt-detail-backdrop" role="presentation" onPointerDown={event => { if (event.target === event.currentTarget) setSelectedId(null); }}>
      <article ref={detailDialog} className="prompt-detail" role="dialog" aria-modal="true" aria-label={selected.title}>
        <button ref={detailClose} className="prompt-detail-close" type="button" aria-label="关闭案例详情" onClick={() => setSelectedId(null)}><X size={16}/></button>
        <div className="prompt-detail-image"><PromptImage item={selected} active={active} state={images[selected.id]} onLoad={() => loadImage(selected)} onImageError={() => setImages(previous => ({ ...previous, [selected.id]: { status: 'error', error: '图片解码失败' } }))} compact/></div>
        <div className="prompt-detail-content">
          <div className="prompt-detail-meta"><span>{selected.category.label}</span><span>案例 #{selected.sourceCaseId}</span><span>上游示例 · 非本机生成</span></div>
          <h2>{selected.title}</h2>
          <div className="prompt-detail-tags">{selected.styles.map(style => <span key={style.sourceLabel}>{style.label}</span>)}{selected.scenes.map(scene => <span key={scene.sourceLabel}>{scene.label}</span>)}</div>
          <div className="prompt-usage"><strong>{promptUsageLabel(selected)}</strong>{selected.usage.notes.length ? selected.usage.notes.map(note => <p key={note}>{note}</p>) : <p>未发现参考图或编辑依赖；仍请按当前模型的实际能力验证效果。</p>}<small>上游未提供可信的精确模型版本、种子或生成参数，本应用不作复现保证。</small></div>
          <div className="prompt-copy-heading"><strong>完整提示词</strong><button type="button" onClick={() => void copyPrompt(selected)}>{copied ? <Check size={12}/> : <Copy size={12}/>}{copied ? '已复制' : '复制原文'}</button></div>
          <pre className="prompt-full-text">{selected.prompt}</pre>
          <div className="prompt-sources"><span>来源 {selected.source.label}</span>{selected.source.url ? <a href={selected.source.url} target="_blank" rel="noreferrer">原帖<ExternalLink size={11}/></a> : <span>未提供原帖</span>}<a href={selected.source.githubUrl} target="_blank" rel="noreferrer">上游案例<ExternalLink size={11}/></a></div>
          <div className="prompt-detail-actions">
            <button className="primary-action" type="button" onClick={() => onUsePrompt({ caseId: selected.id, title: selected.title, prompt: selected.prompt, target: 'new' })}><ArrowUp size={13}/>新建会话并填入</button>
            <button className="secondary" type="button" onClick={() => onUsePrompt({ caseId: selected.id, title: selected.title, prompt: selected.prompt, target: 'current' })}>用于当前会话</button>
            <button className={`secondary prompt-detail-favorite ${favoriteSet.has(selected.id) ? 'is-favorite' : ''}`} type="button" aria-pressed={favoriteSet.has(selected.id)} onClick={() => void favorite(selected)}><Heart size={13} fill={favoriteSet.has(selected.id) ? 'currentColor' : 'none'}/>{favoriteSet.has(selected.id) ? '已收藏' : '收藏'}</button>
          </div>
          <small className="prompt-action-note">填入提示词不会自动发送，也不会切换为上游展示模型。</small>
        </div>
      </article>
    </div>}
  </section>;
}
