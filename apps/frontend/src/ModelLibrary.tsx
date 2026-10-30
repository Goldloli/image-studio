import { useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowLeft,
  Boxes,
  Check,
  ChevronDown,
  ChevronUp,
  Cpu,
  Download,
  FolderOpen,
  Image,
  LoaderCircle,
  Play,
  RefreshCw,
  Search,
  Square,
} from 'lucide-react';
import { api, bytesLabel, native, type StoredDraft, type StudioSnapshot } from './studio-api';
import {
  capabilityFilters,
  filterCatalog,
  installProgress,
  primaryFilters,
  type CapabilityFilter,
  type CatalogPayload,
  type ModelEntry,
  type PrimaryFilter,
} from './model-catalog';
import './model-settings.css';

type Props = {
  studio: StudioSnapshot | null;
  onRefresh: () => void;
  onBack: () => void;
  onError: (value: string) => void;
  onUseModel: (draft: StoredDraft, conversationId: string) => void;
};

const phaseLabels: Record<string, string> = {
  not_installed: '尚未安装',
  preparing: '准备安装',
  downloading: '正在下载',
  verifying: '正在校验模型',
  installing_runtime: '准备隔离运行环境',
  probing: '检查运行环境与设备',
  ready: '模型已就绪',
  interrupted: '安装已中断，可继续',
  cancelled: '安装已停止，可继续',
  unavailable: '安装文件缺失或身份异常',
  failed: '安装失败，可修复重试',
  not_supported: '计划中，尚不可安装',
};

function statusText(entry: ModelEntry): string {
  if (entry.executable) return '当前可执行';
  if (entry.busy) return phaseLabels[entry.installPhase] ?? entry.installPhase;
  if (entry.componentsReady && !entry.runtimeReady) return '组件已安装，运行环境未就绪';
  if (entry.componentsReady && !entry.deviceVerified) return '组件已安装，设备尚未验证';
  return phaseLabels[entry.installPhase] ?? entry.installPhase;
}

function statusTone(entry: ModelEntry): string {
  if (entry.executable) return 'is-ready';
  if (entry.busy) return 'is-active';
  if (entry.error) return 'is-error';
  return 'is-neutral';
}

export function ModelLibrary({ studio, onRefresh, onBack, onError, onUseModel }: Props) {
  const [catalog, setCatalog] = useState<CatalogPayload | null>(null);
  const [loading, setLoading] = useState(native);
  const [error, setError] = useState('');
  const [query, setQuery] = useState('');
  const [primary, setPrimary] = useState<PrimaryFilter>('all');
  const [capability, setCapability] = useState<CapabilityFilter>('all');
  const [expanded, setExpanded] = useState<string | null>(null);
  const [pending, setPending] = useState('');
  const queryGeneration = useRef(0);
  const disposed = useRef(false);
  const pollTimer = useRef<number | undefined>(undefined);

  const load = async (initial = false) => {
    if (!native) {
      setLoading(false);
      setError('模型目录只能在桌面应用中读取。');
      return;
    }
    const generation = ++queryGeneration.current;
    if (initial) setLoading(true);
    try {
      const value = await api<CatalogPayload>('model_catalog');
      if (disposed.current || generation !== queryGeneration.current) return;
      setCatalog(value);
      setError('');
    } catch (cause) {
      if (disposed.current || generation !== queryGeneration.current) return;
      setError(String(cause));
    } finally {
      if (!disposed.current && generation === queryGeneration.current) setLoading(false);
    }
    if (!disposed.current) {
      window.clearTimeout(pollTimer.current);
      pollTimer.current = window.setTimeout(() => void load(false), 1500);
    }
  };

  useEffect(() => {
    disposed.current = false;
    void load(true);
    return () => {
      disposed.current = true;
      queryGeneration.current += 1;
      window.clearTimeout(pollTimer.current);
    };
  }, []);

  const visible = useMemo(
    () => filterCatalog(catalog?.entries ?? [], query, primary, capability),
    [catalog?.entries, query, primary, capability],
  );
  const currentConversation = studio?.conversations.find(
    conversation => conversation.id === studio.activeConversationId,
  );

  const action = async (entry: ModelEntry, command: 'model_install' | 'model_cancel') => {
    if (!entry.installId) return;
    setPending(entry.id);
    try {
      await api(command, command === 'model_install' ? { installId: entry.installId } : undefined);
      onRefresh();
      await load();
    } catch (cause) {
      onError(String(cause));
    } finally {
      setPending('');
    }
  };

  const useModel = async (entry: ModelEntry) => {
    if (!currentConversation) {
      onError('当前没有可用的会话，请先新建会话。');
      return;
    }
    setPending(entry.id);
    try {
      const result = await api<{ conversationId: string; draft: StoredDraft }>(
        'conversation_apply_model',
        { conversationId: currentConversation.id, modelId: entry.id },
      );
      onRefresh();
      onUseModel(result.draft, result.conversationId);
    } catch (cause) {
      onError(String(cause));
    } finally {
      setPending('');
    }
  };

  const clearFilters = () => {
    setQuery('');
    setPrimary('all');
    setCapability('all');
  };

  return (
    <div className="split-page model-page">
      <aside className="section-sidebar model-sidebar">
        <h2>模型</h2>
        <div className="sidebar-search" aria-hidden="true"><Boxes size={15}/><span>本地模型库</span></div>
        {primaryFilters.map(filter => (
          <button
            key={filter.id}
            className={primary === filter.id ? 'active' : ''}
            aria-pressed={primary === filter.id}
            onClick={() => setPrimary(filter.id)}
          >
            {filter.id === 'runtime' ? <Cpu size={17}/> : filter.id === 'installed' ? <FolderOpen size={17}/> : <Boxes size={17}/>}
            <span>{filter.label}</span>
            <span className="count">
              {filter.id === 'all' ? catalog?.counts.total ?? '—'
                : filter.id === 'installed' ? catalog?.counts.installed ?? '—'
                  : filter.id === 'executable' ? catalog?.counts.executable ?? '—'
                    : filter.id === 'planned' ? catalog?.counts.planned ?? '—'
                      : catalog?.counts.runtimeReady ?? '—'}
            </span>
          </button>
        ))}
        <p className="sidebar-note">已接入、文件已安装、运行时就绪、当前可执行和设备验证分别显示。<br/>计划中型号不会提供安装或生成入口。</p>
      </aside>

      <section className="detail-page model-detail">
        <div className="page-heading model-heading">
          <div><h1>模型</h1><p>查找、理解并管理本地图片模型</p></div>
          <button className="secondary" disabled={loading} onClick={() => void load(true)}>
            <RefreshCw size={13} className={loading ? 'spin' : ''}/>刷新
          </button>
        </div>

        <div className="catalog-summary" aria-label="模型目录统计">
          <span>目录 <strong>{catalog?.counts.total ?? '—'}</strong></span>
          <span>正式接入 <strong>{catalog?.counts.supported ?? '—'}</strong></span>
          <span>已安装 <strong>{catalog?.counts.installed ?? '—'}</strong></span>
          <span>可执行 <strong>{catalog?.counts.executable ?? '—'}</strong></span>
        </div>

        <div className="catalog-controls">
          <label className="catalog-search">
            <Search size={15}/>
            <input
              value={query}
              onChange={event => setQuery(event.target.value)}
              placeholder="搜索名称、型号、家族或运行时"
              aria-label="搜索模型"
            />
            {query && <button aria-label="清除模型搜索" onClick={() => setQuery('')}>×</button>}
          </label>
          <label className="catalog-capability">
            <span>能力</span>
            <select value={capability} onChange={event => setCapability(event.target.value as CapabilityFilter)}>
              {capabilityFilters.map(filter => <option value={filter.id} key={filter.id}>{filter.label}</option>)}
            </select>
          </label>
        </div>

        {error && (
          <div className="catalog-error" role="alert">
            <span>{catalog ? `目录刷新失败，保留上次结果：${error}` : `模型目录读取失败：${error}`}</span>
            <button className="secondary" onClick={() => void load(true)}>重试</button>
          </div>
        )}

        {loading && !catalog
          ? <div className="catalog-loading" role="status"><LoaderCircle className="spin" size={19}/>正在读取模型目录与安装事实…</div>
          : visible.length === 0
            ? <div className="catalog-empty">
              <Boxes size={29}/>
              <h2>{catalog ? '无匹配模型' : '模型目录不可用'}</h2>
              <p>{catalog ? '当前搜索与筛选条件没有结果，目录本身未被清空。' : '请重试读取；页面不会把失败显示为空目录。'}</p>
              {catalog && <button className="secondary" onClick={clearFilters}>清除筛选</button>}
            </div>
            : <div className="catalog-list" aria-live="polite">
              {visible.map(entry => {
                const open = expanded === entry.id;
                const progress = installProgress(entry);
                return <article className={`catalog-model ${open ? 'is-open' : ''}`} key={entry.id}>
                  <button
                    className="catalog-model-row"
                    aria-expanded={open}
                    onClick={() => setExpanded(open ? null : entry.id)}
                  >
                    <span className="model-glyph"><Image size={21}/></span>
                    <span className="catalog-model-name">
                      <strong>{entry.displayName}</strong>
                      <small>{entry.variant}</small>
                    </span>
                    <span className="catalog-capabilities">
                      {entry.capabilities.map(item => <i key={item.id}>{item.label}</i>)}
                    </span>
                    <span className={`catalog-status ${statusTone(entry)}`}>{statusText(entry)}</span>
                    {open ? <ChevronUp size={14}/> : <ChevronDown size={14}/>}
                  </button>

                  {entry.busy && progress && <div className="catalog-progress" role="status">
                    <div><LoaderCircle className="spin" size={13}/><span>{phaseLabels[entry.installPhase] ?? entry.installPhase}</span><small>{bytesLabel(entry.receivedBytes)} / {entry.installTotalBytes ? bytesLabel(entry.installTotalBytes) : '阶段未知'}</small></div>
                    <progress value={progress.value} max={progress.max}/>
                  </div>}

                  {open && <div className="catalog-model-details">
                    <p className="catalog-description">{entry.description}</p>
                    <div className="model-facts">
                      <span>{entry.family}</span>
                      <span>{entry.recipe}</span>
                      <span>{entry.license}</span>
                      {entry.modelBytes !== null && <span>原始模型 {bytesLabel(entry.modelBytes)}</span>}
                      {entry.runtimeBytes > 0 && <span>运行时 {bytesLabel(entry.runtimeBytes)}</span>}
                      {entry.requiredBytes > 0 && <span>安装预算 {bytesLabel(entry.requiredBytes)}</span>}
                    </div>
                    <div className="catalog-state-grid">
                      <StateFact label="产品接入" value={entry.productStatus === 'implemented' ? '已接入' : '计划中'} positive={entry.productStatus === 'implemented'}/>
                      <StateFact label="模型组件" value={entry.componentsReady ? '文件齐备' : entry.installerSupported ? '尚未齐备' : '未接入'} positive={entry.componentsReady}/>
                      <StateFact label="运行时" value={entry.runtimeReady ? '已就绪' : '未就绪'} positive={entry.runtimeReady}/>
                      <StateFact label="设备探针" value={entry.deviceVerified ? '已通过' : '未通过'} positive={entry.deviceVerified}/>
                      <StateFact label="当前可执行" value={entry.executable ? '是' : '否'} positive={entry.executable}/>
                    </div>
                    <div className="catalog-detail-columns">
                      <DetailList title="运行与范围" items={[entry.runtime, ...entry.requirements]}/>
                      <DetailList title="不包含" items={entry.exclusions}/>
                    </div>
                    <div className="catalog-source">
                      <strong>版本与来源</strong>
                      <code>{entry.source}</code>
                      <span>revision / release：{entry.revision}</span>
                      <span>{entry.verifiedDevices.length ? `实机验证：${entry.verifiedDevices.join('；')}` : '尚未完成目标设备实机验证。'}</span>
                    </div>
                    {entry.error && <p className="error-text" role="alert">{entry.error}</p>}
                    <div className="catalog-actions">
                      {entry.canUse && <button className="primary-action" disabled={pending === entry.id || !currentConversation} onClick={() => void useModel(entry)}>
                        {pending === entry.id ? <LoaderCircle size={14} className="spin"/> : <Check size={14}/>}用于当前会话
                      </button>}
                      {entry.canStop && <button className="secondary" disabled={pending === entry.id} onClick={() => void action(entry, 'model_cancel')}><Square size={12}/>停止安装</button>}
                      {entry.canInstall && <button className="primary-action" disabled={pending === entry.id} onClick={() => void action(entry, 'model_install')}>
                        {pending === entry.id ? <LoaderCircle size={14} className="spin"/> : <Download size={14}/>}
                        {entry.canContinue ? '继续安装' : entry.installPhase === 'unavailable' || entry.installPhase === 'failed' ? '修复安装' : '安装模型与运行环境'}
                      </button>}
                      {entry.executable && <button className="secondary" onClick={onBack}><Play size={13}/>前往创作</button>}
                      {!entry.executable && !entry.canInstall && !entry.canStop && <span className="catalog-boundary">当前阶段不提供下载、生成或编辑动作。</span>}
                      <button className="secondary" onClick={onBack}><ArrowLeft size={13}/>返回创作</button>
                    </div>
                  </div>}
                </article>;
              })}
            </div>}
      </section>
    </div>
  );
}

function StateFact({ label, value, positive }: { label: string; value: string; positive: boolean }) {
  return <div><span>{label}</span><strong className={positive ? 'is-positive' : ''}>{value}</strong></div>;
}

function DetailList({ title, items }: { title: string; items: string[] }) {
  return <section><h3>{title}</h3><ul>{items.map(item => <li key={item}>{item}</li>)}</ul></section>;
}
