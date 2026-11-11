import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Palette, Boxes, Settings2, Sun, Moon, X, CircleHelp, Image, Images, Sparkles, Terminal, Play, Square, LoaderCircle, Lightbulb } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import Workspace from './Workspace';
import { AssetLibrary } from './AssetLibrary';
import { FeatureLibrary } from './FeatureLibrary';
import { ModelLibrary } from './ModelLibrary';
import { PromptLibrary, type PromptUseRequest } from './PromptLibrary';
import { SettingsPage } from './SettingsPage';
import { api, native, type AssetView, type StoredDraft, type StudioSnapshot } from './studio-api';
import { defaultAppearance, resolveTheme, type AppearancePreferences, type PreferencesEnvelope } from './settings-types';
import './local-generation.css';
import './library-pages.css';

type Page = 'create' | 'inspiration' | 'features' | 'models' | 'works' | 'settings';
type SettingsGroup = 'appearance' | 'defaults' | 'storage' | 'runtime' | 'about';
type AppearanceSaveStatus = 'saved' | 'saving' | 'error';
type RequestedModel = { draft: StoredDraft; conversationId: string; nonce: number };
type RequestedPrompt = PromptUseRequest & { nonce: number };
type RequestedAsset = {
  asset: AssetView;
  conversationTitle: string;
  draftId: number | null;
  turnSequence: number | null;
  nonce: number;
  returnPage: Page | null;
};
type HostStatus = { inferenceAvailable: boolean; status: string; supportedModels: number; diagnosticsEnabled: boolean; developmentBuild: boolean; version: string; platform: string };
type DiagnosticState = { enabled: boolean; mode: string; modes: string[]; snapshot: null | { phase: string; summary: string; probePassed: boolean; isRunning: boolean; pid: number | null; errorCode: string | null; logs: string; progressEvents: number } };
const pages: { id: Page; label: string; icon: LucideIcon }[] = [
  { id: 'create', label: '创作', icon: Palette },
  { id: 'inspiration', label: '灵感库', icon: Lightbulb },
  { id: 'features', label: '功能', icon: Sparkles },
  { id: 'models', label: '模型', icon: Boxes },
  { id: 'works', label: '作品', icon: Images },
  { id: 'settings', label: '设置', icon: Settings2 },
];

function BrandArt({ size, decorative = false }: { size: number; decorative?: boolean }) {
  const [failed, setFailed] = useState(false);
  return failed
    ? <Image size={Math.max(16, Math.round(size * .72))} aria-hidden={decorative || undefined} role={decorative ? undefined : 'img'} aria-label={decorative ? undefined : 'Image Studio 应用图标'}/>
    : <img className="brand-art" width={size} height={size} src="/brand/app-icon-ui.png" alt={decorative ? '' : 'Image Studio 应用图标'} draggable={false} onError={() => setFailed(true)}/>;
}

export default function App() {
  const [page, setPage] = useState<Page>('create');
  const [tabs, setTabs] = useState<Page[]>(['create']);
  const [appearance, setAppearance] = useState<AppearancePreferences>(defaultAppearance);
  const [appearanceStatus, setAppearanceStatus] = useState<AppearanceSaveStatus>('saved');
  const [appearanceError, setAppearanceError] = useState('');
  const appearanceRef = useRef(appearance);
  const appearanceQueue = useRef<Promise<void>>(Promise.resolve());
  const appearanceRevision = useRef(0);
  const [host, setHost] = useState<HostStatus | null>(null);
  const [hostError, setHostError] = useState('');
  const [settingsInitialGroup, setSettingsInitialGroup] = useState<SettingsGroup>('appearance');
  const [diagOpen, setDiagOpen] = useState(false);
  const [diag, setDiag] = useState<DiagnosticState | null>(null);
  const [mode, setMode] = useState('normal');
  const [diagError, setDiagError] = useState('');
  const [devMenu, setDevMenu] = useState(false);
  const [studio, setStudio] = useState<StudioSnapshot|null>(null);
  const [requestedAsset, setRequestedAsset] = useState<RequestedAsset | null>(null);
  const [requestedComposer, setRequestedComposer] = useState<{ modelId: string; capabilityId: string; nonce: number } | null>(null);
  const [requestedModel, setRequestedModel] = useState<RequestedModel | null>(null);
  const [requestedPrompt, setRequestedPrompt] = useState<RequestedPrompt | null>(null);
  const requestNonce = useRef(0);
  const refreshStudio = useRef<()=>void>(()=>{});
  const refresh = () => refreshStudio.current();
  const navigate = (next: Page) => { setTabs(old => old.includes(next) ? old : [...old, next]); setPage(next); };
  const commitAppearance = (next: AppearancePreferences) => {
    appearanceRef.current = next;
    setAppearance(next);
    setAppearanceStatus('saving');
    setAppearanceError('');
    const revision = ++appearanceRevision.current;
    if (!native) {
      setAppearanceStatus('saved');
      return;
    }
    const operation = appearanceQueue.current
      .then(() => api<PreferencesEnvelope>('preferences_save', { appearance: next }))
      .then(result => {
        if (revision !== appearanceRevision.current) return;
        appearanceRef.current = result.preferences.appearance;
        setAppearance(result.preferences.appearance);
        setAppearanceStatus('saved');
        setAppearanceError('');
      })
      .catch(error => {
        if (revision !== appearanceRevision.current) return;
        setAppearanceStatus('error');
        setAppearanceError(String(error));
      });
    appearanceQueue.current = operation.catch(() => {});
  };
  const reloadPreferences = async () => {
    const result = await api<PreferencesEnvelope>('preferences_reload');
    appearanceRef.current = result.preferences.appearance;
    setAppearance(result.preferences.appearance);
    setAppearanceStatus('saved');
    setAppearanceError('');
  };
  useEffect(() => {
    const apply = () => {
      const resolved = resolveTheme(appearance.theme);
      document.documentElement.dataset.theme = resolved;
      document.documentElement.style.colorScheme = resolved;
      document.documentElement.dataset.fontScale = appearance.fontScale;
      document.documentElement.dataset.density = appearance.density;
      document.documentElement.dataset.reducedMotion = String(appearance.reducedMotion);
    };
    apply();
    if (appearance.theme !== 'system') return;
    const media = window.matchMedia?.('(prefers-color-scheme: dark)');
    media?.addEventListener('change', apply);
    return () => media?.removeEventListener('change', apply);
  }, [appearance]);
  useEffect(() => {
    if (!native) return;
    let disposed = false;
    api<PreferencesEnvelope>('preferences_snapshot')
      .then(result => {
        if (disposed) return;
        appearanceRef.current = result.preferences.appearance;
        setAppearance(result.preferences.appearance);
        if (result.preferences.loadError) setHostError(result.preferences.loadError);
      })
      .catch(error => { if (!disposed) setHostError(String(error)); });
    return () => { disposed = true; };
  }, []);
  useEffect(() => {
    if (native) invoke<HostStatus>('workspace_status').then(status => { setHost(status); if (status.diagnosticsEnabled) setDiagOpen(true); }).catch(error => setHostError(String(error)));
  }, []);
  useEffect(() => {
    if (!native) return;
    let disposed=false,inflight=false;
    let timer:number|undefined;
    const poll=async()=>{
      if(inflight||disposed)return;
      inflight=true;
      try { const value=await api<StudioSnapshot>('studio_snapshot');if(!disposed){setStudio(value);if(value.error)setHostError(value.error);} }
      catch(error){if(!disposed)setHostError(String(error));}
      finally{inflight=false;if(!disposed)timer=window.setTimeout(()=>void poll(),1000);}
    };
    refreshStudio.current=()=>{window.clearTimeout(timer);void poll();};void poll();
    return()=>{disposed=true;window.clearTimeout(timer);};
  },[]);
  useEffect(() => {
    if (!diagOpen || !native) return;
    let disposed = false;
    let initial = true;
    let timer: number | undefined;
    const poll = async () => {
      try {
        const value = await invoke<DiagnosticState>('diagnostic_snapshot');
        if (!disposed) { setDiag(value); if (initial) { setMode(value.mode); initial = false; } }
      } catch (error) { if (!disposed) setDiagError(String(error)); }
      finally { if (!disposed) timer = window.setTimeout(() => void poll(), 500); }
    };
    void poll();
    return () => { disposed = true; window.clearTimeout(timer); };
  }, [diagOpen]);
  useEffect(() => {
    const close = (event: KeyboardEvent) => { if (event.key === 'Escape') setDevMenu(false); };
    window.addEventListener('keydown', close); return () => window.removeEventListener('keydown', close);
  }, []);
  const closeTab = (target: Page) => { setTabs(old => old.filter(tab => tab !== target)); if (page === target) setPage('create'); };
  const openAsset = (asset: AssetView, conversationTitle: string, draftId: number | null, turnSequence: number | null, returnPage: Page | null) => {
    setRequestedAsset({ asset, conversationTitle, draftId, turnSequence, returnPage, nonce: ++requestNonce.current });
    navigate('create');
  };
  const enableDiagnostics = async () => {
    setDevMenu(false); setDiagError('');
    if (!native) { setHostError('通信诊断仅在桌面应用中可用。'); return; }
    try { await invoke('diagnostic_enable'); setDiagOpen(true); } catch (error) { setHostError(String(error)); }
  };
  const diagnosticAction = async (command: 'diagnostic_start' | 'diagnostic_stop') => {
    setDiagError('');
    try { setDiag(await invoke<DiagnosticState>(command, command === 'diagnostic_start' ? { mode } : undefined)); } catch (error) { setDiagError(String(error)); }
  };
  const platform = host?.platform ?? (navigator.userAgent.includes('Mac') ? 'macos' : 'other');
  const resolvedTheme = resolveTheme(appearance.theme);
  return <div className="app" data-platform={platform}>
    <header className="titlebar">
      <div className="tabs" role="tablist" aria-label="打开的页面">
        {tabs.map(id => { const item = pages.find(item => item.id === id)!; const Icon = item.icon; return <div className={`page-tab ${page === id ? 'active' : ''}`} key={id}>
          <button role="tab" aria-selected={page === id} onClick={() => navigate(id)}><Icon size={15}/><span>{item.label}</span></button>
          {id !== 'create' && <button className="tab-close" aria-label={`关闭${item.label}标签`} onClick={() => closeTab(id)}><X size={12}/></button>}
        </div>; })}
      </div>
      <div className="window-drag" data-tauri-drag-region />
      {!native && <span className="preview-label">界面预览</span>}
      {(host?.developmentBuild || !native) && <div className="dev-menu"><button className="quiet tiny" onClick={() => setDevMenu(!devMenu)} aria-expanded={devMenu}>开发</button>{devMenu && <div className="popover dev-popover"><button onClick={() => void enableDiagnostics()}><Terminal size={14}/>启用通信诊断</button></div>}</div>}
    </header>
    <div className="app-body">
      <nav className="icon-rail" aria-label="主导航">
        <button className="brand-mark" title="Image Studio · 返回创作" aria-label="Image Studio · 返回创作" onClick={() => navigate('create')}><BrandArt size={28} decorative/></button>
        <div className="primary-navigation">{pages.filter(item => item.id !== 'settings').map(({ id, label, icon: Icon }) => <button key={id} className={`rail-button ${page === id ? 'selected' : ''}`} title={label} aria-label={label} aria-current={page === id ? 'page' : undefined} onClick={() => navigate(id)}><Icon size={20} strokeWidth={1.6}/></button>)}</div>
        <div className="rail-footer">
          <button className="rail-button" aria-label={resolvedTheme === 'dark' ? '切换为浅色主题' : '切换为深色主题'} title={resolvedTheme === 'dark' ? '浅色主题' : '深色主题'} onClick={() => commitAppearance({ ...appearanceRef.current, theme: resolvedTheme === 'dark' ? 'light' : 'dark' })}>{resolvedTheme === 'dark' ? <Sun size={19}/> : <Moon size={19}/>}</button>
          <button className="rail-button" aria-label="关于 Image Studio" title="关于" onClick={() => { setSettingsInitialGroup('about'); navigate('settings'); }}><CircleHelp size={19}/></button>
          <button className={`rail-button ${page === 'settings' ? 'selected' : ''}`} aria-label="设置" title="设置" onClick={() => { setSettingsInitialGroup('appearance'); navigate('settings'); }}><Settings2 size={20} strokeWidth={1.6}/></button>
        </div>
      </nav>
      <main className="main-surface">
        {hostError && <div className="host-error" role="status">{hostError}<button aria-label="关闭提示" onClick={() => setHostError('')}><X size={12}/></button></div>}
        <div className="workspace-page" hidden={page !== 'create'}><Workspace onModels={() => navigate('models')} studio={studio} requestedAsset={requestedAsset} requestedComposer={requestedComposer} requestedModel={requestedModel} requestedPrompt={requestedPrompt} sidebarCollapsed={appearance.sidebarCollapsed} onSidebarCollapsedChange={collapsed => commitAppearance({ ...appearanceRef.current, sidebarCollapsed: collapsed })} onRefresh={refresh} onAssetReturn={() => { const returnPage = requestedAsset?.returnPage ?? 'create'; setRequestedAsset(null); navigate(returnPage); }}/></div>
        <div className="workspace-page" hidden={page !== 'inspiration'}><PromptLibrary active={page === 'inspiration'} onBack={() => navigate('create')} onUsePrompt={request => { setRequestedPrompt({ ...request, nonce: ++requestNonce.current }); navigate('create'); }} onError={setHostError}/></div>
        <div className="workspace-page" hidden={page !== 'features'}><FeatureLibrary studio={studio} onBack={() => navigate('create')} onModels={() => navigate('models')} onStart={(modelId, capabilityId) => { setRequestedComposer({ modelId, capabilityId, nonce: Date.now() }); navigate('create'); }} onError={setHostError}/></div>
        {page === 'models' && <ModelLibrary studio={studio} onRefresh={refresh} onBack={()=>navigate('create')} onError={setHostError} onUseModel={(draft, conversationId) => { setRequestedModel({ draft, conversationId, nonce: Date.now() }); navigate('create'); }}/>}
        <div className="workspace-page" hidden={page !== 'works'}><AssetLibrary active={page === 'works'} onBack={() => navigate('create')} onOpenAsset={item => openAsset({ id: item.id, jobId: item.jobId, conversationId: item.conversationId, turnId: item.turnId, prompt: item.prompt, width: item.width, height: item.height, seed: item.seed, bytes: item.byteCount, available: item.available, error: item.availabilityError }, item.conversationTitle, item.draftId, item.turnSequence, 'works')} onError={setHostError}/></div>
        {page === 'settings' && <SettingsPage
          initialGroup={settingsInitialGroup}
          appearance={appearance}
          saveStatus={appearanceStatus}
          saveError={appearanceError}
          diagnosticsOpen={diagOpen}
          onBack={() => navigate('create')}
          onAppearanceChange={commitAppearance}
          onPreferencesReload={reloadPreferences}
          onOpenDiagnostics={() => void enableDiagnostics()}
          onError={setHostError}
        />}
      </main>
      {diagOpen && <aside className="diagnostic-panel"><div className="diag-title"><Terminal size={16}/><strong>开发诊断</strong><button className="icon-button" aria-label="收起通信诊断" onClick={() => setDiagOpen(false)}><X size={15}/></button></div><p className="muted">通信诊断，不执行图片生成</p><label>故障模式<select value={mode} disabled={diag?.snapshot?.isRunning} onChange={event => setMode(event.target.value)}>{(diag?.modes ?? ['normal']).map(value => <option key={value}>{value}</option>)}</select></label><div className="diag-actions"><button className="secondary" disabled={diag?.snapshot?.isRunning} onClick={() => void diagnosticAction('diagnostic_start')}><Play size={13}/>启动诊断</button><button className="secondary" disabled={!diag?.snapshot?.isRunning} onClick={() => void diagnosticAction('diagnostic_stop')}><Square size={12}/>停止</button></div>{diagError && <p role="alert" className="error-text">{diagError}</p>}<div className="diag-status">{diag?.snapshot?.isRunning && <LoaderCircle size={15} className="spin"/>}<strong>{diag?.snapshot?.summary ?? '尚未启动'}</strong></div>{diag?.snapshot && <><p className="muted">状态 {diag.snapshot.phase}<br/>进度事件 {diag.snapshot.progressEvents}<br/>{diag.snapshot.errorCode}</p>{diag.snapshot.probePassed && <span className="success-text">通信探针已通过</span>}<details><summary>诊断日志</summary><pre>{diag.snapshot.logs || '无日志'}</pre></details></>}<div className="viewport-controls"><span>开发窗口</span>{[[960, 640], [1280, 800]].map(([width, height]) => <button key={width} className="secondary" onClick={() => void invoke('diagnostic_viewport', { width, height, scale: 1 }).catch(error => setDiagError(String(error)))}>{width} × {height}</button>)}</div></aside>}
    </div>
  </div>;
}
