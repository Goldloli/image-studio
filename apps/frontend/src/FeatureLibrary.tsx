import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowLeft,
  ArrowUpRight,
  Boxes,
  Image,
  ListChecks,
  LoaderCircle,
  Search,
  Sparkles,
} from 'lucide-react';
import { api, native, type FeatureCatalog, type FeatureCatalogEntry, type StudioSnapshot } from './studio-api';
import { textRecipeForModel } from './model-recipes';

type Props = {
  studio: StudioSnapshot | null;
  onBack: () => void;
  onModels: () => void;
  onStart: (modelId: string, capabilityId: string) => void;
  onError: (value: string) => void;
};

export function FeatureLibrary({ studio, onBack, onModels, onStart, onError }: Props) {
  const [catalog, setCatalog] = useState<FeatureCatalog | null>(null);
  const [selectedId, setSelectedId] = useState('text-to-image');
  const [category, setCategory] = useState('全部');
  const [search, setSearch] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const revision = useRef(0);

  const load = useCallback(async () => {
    if (!native) {
      setError('功能目录仅在桌面应用中读取。');
      return;
    }
    const current = ++revision.current;
    setLoading(true);
    setError('');
    try {
      const value = await api<FeatureCatalog>('feature_catalog');
      if (current !== revision.current) return;
      setCatalog(value);
      setSelectedId(currentId => value.entries.some(entry => entry.featureId === currentId)
        ? currentId
        : value.entries[0]?.featureId ?? '');
    } catch (cause) {
      if (current !== revision.current) return;
      setError(String(cause));
      onError(String(cause));
    } finally {
      if (current === revision.current) setLoading(false);
    }
  }, [onError]);

  useEffect(() => {
    void load();
  }, [load]);

  const categories = useMemo(
    () => ['全部', ...Array.from(new Set(catalog?.entries.map(entry => entry.category) ?? []))],
    [catalog],
  );
  const visible = useMemo(() => {
    const needle = search.trim().toLocaleLowerCase();
    return (catalog?.entries ?? []).filter(entry => {
      const inCategory = category === '全部' || entry.category === category;
      const matches = !needle || [
        entry.title,
        entry.summary,
        entry.description,
        entry.scope,
        ...entry.inputs,
        ...entry.outputs,
      ].some(value => value.toLocaleLowerCase().includes(needle));
      return inCategory && matches;
    });
  }, [catalog, category, search]);
  const selected = visible.find(entry => entry.featureId === selectedId)
    ?? visible[0]
    ?? null;

  const chooseFeature = (entry: FeatureCatalogEntry) => {
    setSelectedId(entry.featureId);
  };

  return <section className="detail-page library-page feature-page" aria-labelledby="feature-page-title">
    <header className="library-heading">
      <div>
        <span className="library-kicker">按想完成的事情进入</span>
        <h1 id="feature-page-title">功能</h1>
        <p>只展示宿主真实登记的能力；计划中功能说明范围，但不会伪装成可执行按钮。</p>
      </div>
      <button className="secondary" onClick={onBack}><ArrowLeft size={14}/>返回创作</button>
    </header>

    {error && <div className="library-error" role="alert"><span>{error}</span><button onClick={() => void load()}>重试</button></div>}
    {loading && <div className="library-loading" role="status"><LoaderCircle className="spin" size={16}/>正在读取功能目录…</div>}

    <div className="feature-shell">
      <aside className="feature-categories" aria-label="功能分类">
        <div className="library-search"><Search size={14}/><input value={search} onChange={event => setSearch(event.target.value)} aria-label="搜索功能" placeholder="搜索能力"/></div>
        {categories.map(item => <button key={item} className={item === category ? 'active' : ''} aria-pressed={item === category} onClick={() => setCategory(item)}>{item}<span>{item === '全部' ? catalog?.entries.length ?? 0 : catalog?.entries.filter(entry => entry.category === item).length ?? 0}</span></button>)}
        <div className="feature-category-note"><Sparkles size={14}/><p>文生图、单图编辑、去背景和 4× 放大均在本机执行。请先安装相应模型与运行环境。</p></div>
      </aside>

      <div className="feature-browser">
        {!visible.length && !loading ? <div className="library-empty"><Search size={25}/><h2>{catalog?.entries.length ? '没有匹配的功能' : '没有已登记功能'}</h2><p>{catalog?.entries.length ? '尝试修改搜索词或切换分类。' : '重新读取功能目录；读取失败不会被显示成“无结果”。'}</p>{catalog?.entries.length ? <button className="secondary" onClick={() => { setSearch(''); setCategory('全部'); }}>清除搜索</button> : <button className="secondary" onClick={() => void load()}>重新读取</button>}</div>
          : <><div className="feature-list" role="listbox" aria-label="功能列表">{visible.map(entry => <button role="option" aria-selected={selected?.featureId === entry.featureId} key={entry.featureId} className={`feature-row ${selected?.featureId === entry.featureId ? 'selected' : ''}`} onClick={() => chooseFeature(entry)}><span className={`feature-symbol state-${entry.state}`}><FeatureIcon feature={entry}/></span><span className="feature-row-copy"><strong>{entry.title}</strong><span>{entry.summary}</span></span><span className={`feature-state state-${entry.state}`}>{entry.stateLabel}</span></button>)}</div>
            {selected && <FeatureDetail feature={selected} studio={studio} onModels={onModels} onStart={onStart}/>}</>}
      </div>
    </div>
  </section>;
}

function FeatureIcon({ feature }: { feature: FeatureCatalogEntry }) {
  if (feature.featureId === 'text-to-image') return <Image size={18}/>;
  if (feature.featureId === 'image-edit') return <Sparkles size={18}/>;
  if (feature.featureId === 'remove-background') return <ListChecks size={18}/>;
  return <Boxes size={18}/>;
}

function FeatureDetail({ feature, studio, onModels, onStart }: {
  feature: FeatureCatalogEntry;
  studio: StudioSnapshot | null;
  onModels: () => void;
  onStart: (modelId: string, capabilityId: string) => void;
}) {
  const planned = feature.productStatus === 'planned';
  return <article className="feature-detail" aria-label={`${feature.title}详情`}>
    <div className="feature-detail-scroll">
      <div className="feature-detail-heading"><div><span className="library-kicker">{feature.category}</span><h2>{feature.title}</h2><p>{feature.description}</p></div><span className={`feature-state large state-${feature.state}`}>{feature.stateLabel}</span></div>
      <div className="feature-facts">
        <section><h3>需要输入</h3>{feature.inputs.map(value => <span key={value}>{value}</span>)}</section>
        <section><h3>产生输出</h3>{feature.outputs.map(value => <span key={value}>{value}</span>)}</section>
      </div>
      <section className="feature-dependencies"><h3>真实依赖</h3>{feature.dependencies.map(dependency => <div key={`${dependency.kind}:${dependency.label}`}><span className="dependency-kind">{dependency.kind === 'model' ? '模型' : dependency.kind === 'runtime' ? '运行环境' : '会话'}</span><span>{dependency.label}</span></div>)}</section>
      <div className={`feature-scope ${planned ? 'planned' : ''}`}><strong>{planned ? '为什么现在不能开始' : feature.blockingReason ?? '能力已满足启动条件'}</strong><p>{feature.blockingReason ?? '模型安装、运行时和当前会话条件均由宿主实时查询。'}</p><small>{planned ? `计划信息不代表已安装、已认证或可执行。${feature.scope}` : feature.scope}</small></div>
    </div>
    <div className="feature-detail-actions">
      {feature.canStart && feature.action === 'open-composer' && feature.modelId && <button className="primary-action" onClick={() => onStart(feature.modelId!, capabilityFor(feature))}>{
        feature.featureId === 'text-to-image' ? '在文生图中开始'
          : feature.featureId === 'image-edit' ? '在图片编辑中开始'
            : feature.featureId === 'remove-background' ? '在去背景中开始'
              : '在放大中开始'
      }<ArrowUpRight size={15}/></button>}
      {!planned && !feature.canStart && <button className="primary-action" onClick={onModels}>查看模型与依赖<ArrowUpRight size={15}/></button>}
      {planned && <span className="planned-note">计划中功能没有启动按钮</span>}
      {studio?.activeConversationId && feature.canStart && <span className="feature-context">将进入当前会话的现有输入框</span>}
    </div>
  </article>;
}

function capabilityFor(feature: FeatureCatalogEntry): string {
  if (feature.featureId === 'image-edit') return 'image.edit.flux2-klein-4b.v1';
  if (feature.featureId === 'remove-background') return 'image.remove-background.birefnet-general-epoch-244.v1';
  if (feature.featureId === 'upscale') return 'image.upscale.realesrgan-x4plus.v1';
  return textRecipeForModel(feature.modelId ?? '')?.capabilityId ?? 'image.text-to-image.z-image-turbo.v1';
}
