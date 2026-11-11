import { useEffect, useMemo, useState } from 'react';
import {
  ChevronLeft,
  CircleHelp,
  Cpu,
  Database,
  ExternalLink,
  FolderOpen,
  HardDrive,
  LoaderCircle,
  Monitor,
  Moon,
  RefreshCw,
  SlidersHorizontal,
  Sun,
} from 'lucide-react';
import { api, bytesLabel, native } from './studio-api';
import {
  builtInCreationDefaults,
  defaultAppearance,
  defaultsEqual,
  type AppearancePreferences,
  type CreationDefaults,
  type PreferencesEnvelope,
  type SettingsSnapshot,
} from './settings-types';
import { textRecipeForModel } from './model-recipes';
import { isNonCommercialLicense } from './model-catalog';
import {
  PROMPT_LIBRARY_UPSTREAM_COMMIT,
  PROMPT_LIBRARY_UPSTREAM_REPOSITORY,
  promptLicenseLinks,
  shortCommit,
  type PromptLicenseLinks,
  type PromptLibrarySnapshot,
} from './prompt-library-state';
import './model-settings.css';

type Group = 'appearance' | 'defaults' | 'storage' | 'runtime' | 'about';
type SaveStatus = 'saved' | 'saving' | 'error';

const groups: { id: Group; label: string; icon: typeof Sun }[] = [
  { id: 'appearance', label: '通用与外观', icon: Sun },
  { id: 'defaults', label: '创作默认值', icon: SlidersHorizontal },
  { id: 'storage', label: '存储', icon: HardDrive },
  { id: 'runtime', label: '运行环境', icon: Cpu },
  { id: 'about', label: '关于', icon: CircleHelp },
];

const validSeed = (value: string) => value === '' || (/^\d+$/.test(value) && Number(value) <= 4294967295);

export function SettingsPage({
  initialGroup = 'appearance',
  appearance,
  saveStatus,
  saveError,
  onBack,
  onAppearanceChange,
  onPreferencesReload,
  onError,
}: {
  initialGroup?: Group;
  appearance: AppearancePreferences;
  saveStatus: SaveStatus;
  saveError: string;
  onBack: () => void;
  onAppearanceChange: (value: AppearancePreferences) => void;
  onPreferencesReload: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const [group, setGroup] = useState<Group>(initialGroup);
  const [snapshot, setSnapshot] = useState<SettingsSnapshot | null>(null);
  const [loading, setLoading] = useState(native);
  const [loadError, setLoadError] = useState('');
  const [defaults, setDefaults] = useState<CreationDefaults>(builtInCreationDefaults);
  const [savedDefaults, setSavedDefaults] = useState<CreationDefaults>(builtInCreationDefaults);
  const [defaultsError, setDefaultsError] = useState('');
  const [defaultsSaveStatus, setDefaultsSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');
  const [pendingGroup, setPendingGroup] = useState<Group | null>(null);
  const [storagePending, setStoragePending] = useState('');

  const load = async (initial = false) => {
    if (!native) {
      setLoading(false);
      setLoadError('设置只能在桌面应用中读取和保存。');
      return;
    }
    if (initial) setLoading(true);
    try {
      const value = await api<SettingsSnapshot>('settings_snapshot');
      setSnapshot(value);
      setDefaults(value.preferences.creationDefaults);
      setSavedDefaults(value.preferences.creationDefaults);
      setDefaultsError('');
      setLoadError('');
      setDefaultsSaveStatus('idle');
    } catch (cause) {
      setLoadError(String(cause));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { void load(true); }, []);
  useEffect(() => { setGroup(initialGroup); }, [initialGroup]);

  const dirty = !defaultsEqual(defaults, savedDefaults);
  const selectedTextRecipe = textRecipeForModel(defaults.modelId);
  const validationError = useMemo(() => {
    if (!validSeed(defaults.seed)) return '种子须为 0–4294967295 的整数，或留空随机。';
    if (defaults.count !== 1) return '当前产品每轮只接受 1 张图片。';
    if (!['1:1', '4:3', '3:4'].includes(defaults.aspect)) return '当前产品不支持该画幅。';
    if (selectedTextRecipe && !selectedTextRecipe.aspects.includes(defaults.aspect)) {
      return `所选模型仅支持 ${selectedTextRecipe.aspects.join('、')}；请明确选择有效画幅。`;
    }
    return '';
  }, [defaults, selectedTextRecipe]);

  const chooseGroup = (next: Group) => {
    if (next === group) return;
    if (group === 'defaults' && dirty) {
      setPendingGroup(next);
      return;
    }
    setGroup(next);
  };

  const updateAppearance = (patch: Partial<AppearancePreferences>) => {
    const next = { ...appearance, ...patch };
    setSnapshot(current => current ? {
      ...current,
      preferences: { ...current.preferences, appearance: next },
    } : current);
    onAppearanceChange(next);
  };

  const persistDefaults = async (value: CreationDefaults, nextGroup?: Group) => {
    if (!native) {
      onError('设置只能在桌面应用中保存。');
      return false;
    }
    const recipe = textRecipeForModel(value.modelId);
    if (!validSeed(value.seed) || value.count !== 1 || !['1:1', '4:3', '3:4'].includes(value.aspect) || (recipe && !recipe.aspects.includes(value.aspect))) {
      setDefaultsError('创作默认值整组无效，请修正标出的字段。');
      setDefaultsSaveStatus('error');
      return false;
    }
    setDefaultsSaveStatus('saving');
    setDefaultsError('');
    try {
      const result = await api<PreferencesEnvelope>('creation_defaults_save', { defaults: value });
      setSnapshot(current => current ? {
        ...current,
        preferences: result.preferences,
        catalog: current.catalog,
      } : current);
      setDefaults(value);
      setSavedDefaults(value);
      setDefaultsSaveStatus('saved');
      if (nextGroup) {
        setPendingGroup(null);
        setGroup(nextGroup);
      }
      return true;
    } catch (cause) {
      const message = String(cause);
      setDefaultsError(message);
      setDefaultsSaveStatus('error');
      return false;
    }
  };

  const saveBeforeSwitch = async () => {
    const target = pendingGroup;
    if (!target) return;
    await persistDefaults(defaults, target);
  };

  const discardAndSwitch = () => {
    const target = pendingGroup;
    setDefaults(savedDefaults);
    setDefaultsError('');
    setDefaultsSaveStatus('idle');
    setPendingGroup(null);
    if (target) setGroup(target);
  };

  const reloadAll = async () => {
    setLoading(true);
    setLoadError('');
    try {
      await onPreferencesReload();
      await load();
    } catch (cause) {
      setLoadError(String(cause));
    } finally {
      setLoading(false);
    }
  };

  const openStorage = async (id: string) => {
    setStoragePending(id);
    try { await api('storage_open', { locationId: id }); }
    catch (cause) { onError(String(cause)); }
    finally { setStoragePending(''); }
  };

  return <div className="split-page settings-page settings-page-v2">
    <aside className="section-sidebar settings-sidebar">
      <h2>设置</h2>
      <button className="back-button" onClick={onBack}><ChevronLeft size={16}/>返回创作</button>
      <span className="section-label">应用配置</span>
      {groups.map(({ id, label, icon: Icon }) => (
        <button key={id} className={group === id ? 'active' : ''} onClick={() => chooseGroup(id)}>
          <Icon size={17}/>{label}
          {id === 'defaults' && dirty && <i className="unsaved-dot" title="有未保存的创作默认值"/>}
        </button>
      ))}
      <p className="sidebar-note">界面偏好即时保存；创作默认值显式整组保存。<br/>应用偏好与项目作品分开存储。</p>
    </aside>

    <section className="detail-page settings-detail settings-detail-v2">
      <div className="settings-topline">
        <div><h1>{groups.find(item => item.id === group)?.label}</h1><p>真实生效并可跨重启恢复的应用设置</p></div>
        <button className="secondary" disabled={loading} onClick={() => void reloadAll()}><RefreshCw size={13} className={loading ? 'spin' : ''}/>重新读取</button>
      </div>

      {loading && !snapshot
        ? <div className="settings-loading"><LoaderCircle className="spin" size={19}/>正在读取应用偏好、存储与运行环境…</div>
        : <>
          {loadError && <div className="catalog-error" role="alert"><span>设置读取失败：{loadError}</span><button className="secondary" onClick={() => void reloadAll()}>重试</button></div>}
          {snapshot?.preferences.loadError && <div className="settings-warning" role="alert">
            <span>{snapshot.preferences.loadError} 当前显示逐字段内置回退，原文件未被覆盖。</span>
            <button onClick={() => void reloadAll()}>重试读取</button>
          </div>}
          {!!snapshot?.preferences.warnings.length && <div className="settings-warning">
            {snapshot.preferences.warnings.map(warning => <span key={warning}>{warning}</span>)}
          </div>}

          {group === 'appearance' && <AppearanceSettings
            appearance={appearance}
            status={saveStatus}
            error={saveError}
            onChange={updateAppearance}
            onReset={() => updateAppearance(defaultAppearance)}
          />}

          {group === 'defaults' && <div className="settings-section">
            <SectionHeading title="新会话默认值" text="只影响以后新建的会话；不会复制或改写当前会话的瞬时选择。" />
            <div className="defaults-form">
              <label>
                <span>默认模型</span>
                <select value={defaults.modelId} onChange={event => setDefaults({ ...defaults, modelId: event.target.value })}>
                  {(snapshot?.catalog.entries ?? []).filter(model => model.productStatus === 'implemented' && model.capabilities.some(capability => capability.id === 'text_to_image')).map(model => <option key={model.id} value={model.id} disabled={!model.hardwareEligible}>
                    {model.displayName}{model.productStatus === 'planned' ? '（计划中）' : ''}{model.hardwareEligible ? '' : '（设备不满足）'}
                  </option>)}
                </select>
                <small>{modelAvailabilityNote(snapshot, defaults.modelId)}</small>
              </label>
              <fieldset>
                <legend>默认画幅</legend>
                <div className="settings-segments">
                  {(selectedTextRecipe?.aspects ?? ['1:1', '4:3', '3:4']).map(aspect => <button key={aspect} className={defaults.aspect === aspect ? 'is-selected' : ''} aria-pressed={defaults.aspect === aspect} onClick={() => setDefaults({ ...defaults, aspect })}>{aspect}</button>)}
                </div>
              </fieldset>
              <label>
                <span>每轮图片数量</span>
                <input type="number" min={1} max={1} value={defaults.count} onChange={event => setDefaults({ ...defaults, count: Number(event.target.value) })}/>
                <small>当前固定为 1 张。</small>
              </label>
              <label>
                <span>种子策略</span>
                <input inputMode="numeric" value={defaults.seed} aria-invalid={!validSeed(defaults.seed)} placeholder="留空随机" onChange={event => setDefaults({ ...defaults, seed: event.target.value })}/>
                <small>填写 0–4294967295 时固定种子。</small>
              </label>
            </div>
            {(validationError || defaultsError) && <p className="error-text" role="alert">{defaultsError || validationError}</p>}
            <div className="defaults-actions">
              <button className="primary-action" disabled={!dirty || !!validationError || defaultsSaveStatus === 'saving'} onClick={() => void persistDefaults(defaults)}>
                {defaultsSaveStatus === 'saving' ? <LoaderCircle className="spin" size={14}/> : <CheckIcon/>}
                保存创作默认值
              </button>
              <button className="secondary" disabled={!dirty || defaultsSaveStatus === 'saving'} onClick={() => { setDefaults(savedDefaults); setDefaultsError(''); setDefaultsSaveStatus('idle'); }}>放弃更改</button>
              <button className="secondary" onClick={() => void persistDefaults(builtInCreationDefaults)}>恢复本组默认</button>
            </div>
            <p className={`save-status ${defaultsSaveStatus === 'error' ? 'is-error' : ''}`} role="status">
              {defaultsSaveStatus === 'saving' ? '正在原子保存整组默认值…'
                : defaultsSaveStatus === 'saved' ? '创作默认值已保存，只影响以后新建的会话。'
                  : dirty ? '有尚未保存的创作默认值。' : '当前表单与已保存默认值一致。'}
            </p>
          </div>}

          {group === 'storage' && <div className="settings-section">
            <SectionHeading title="本地存储" text="路径和占用由宿主读取；未知值不会显示成 0 或“不支持”。" />
            <div className="storage-list">
              {snapshot?.storage.entries.map(entry => <article key={entry.id} className={`storage-row status-${entry.status}`}>
                <span className="storage-icon"><Database size={17}/></span>
                <div>
                  <strong>{entry.label}</strong>
                  <code>{entry.path}</code>
                  {entry.error && <p className={entry.error ? 'error-text' : ''}>{entry.error || (entry.status === 'missing' ? '目录尚未创建。' : '')}</p>}
                </div>
                <span className="storage-size">{entry.status === 'error' ? '读取失败' : bytesLabel(entry.bytes)}</span>
                <span className="storage-free">{entry.availableBytes === null ? '可用空间未知' : `可用 ${bytesLabel(entry.availableBytes)}`}</span>
                <button className="secondary" disabled={entry.status === 'missing' || storagePending === entry.id} onClick={() => void openStorage(entry.id)}>
                  {storagePending === entry.id ? <LoaderCircle className="spin" size={12}/> : <FolderOpen size={12}/>}定位
                </button>
              </article>)}
            </div>
            <p className="settings-boundary">当前不提供目录迁移、删除或任意路径输入。相关操作需要独立的空间预检、引用检查、忙碌互斥和回滚设计。</p>
          </div>}

          {group === 'runtime' && <div className="settings-section">
            <SectionHeading title="运行环境" text="设备探针、整模型运行与实机验收是不同事实。" />
            <div className="runtime-summary">
              <InfoRow label="平台" value={`${snapshot?.environment.platform ?? '—'} · ${snapshot?.environment.architecture ?? '—'}`}/>
              <InfoRow label="运行时" value={`${snapshot?.environment.runtimeName ?? '—'} ${snapshot?.environment.runtimeVersion ?? ''} · Python ${snapshot?.environment.pythonVersion ?? '—'}`}/>
              <InfoRow label="本地设备" value={snapshot?.environment.deviceFacts ?? '尚未读取'}/>
              <InfoRow label="当前状态" value={snapshot?.environment.runtime.summary ?? '尚未读取'}/>
              <InfoRow label="运行时就绪" value={snapshot?.environment.runtime.runtimeReady ? '是' : '否'}/>
              <InfoRow label="设备探针" value={snapshot?.environment.runtime.deviceVerified ? '已通过' : '尚未通过'}/>
              <InfoRow label="最后错误" value={snapshot?.environment.runtime.lastError ?? '无已知错误'}/>
            </div>
            <div className="runtime-summary">
              {(snapshot?.environment.runtimes ?? []).map(entry => <InfoRow
                key={entry.installId}
                label={`${entry.label} · ${entry.runtimeLockId ?? '未登记运行时'}`}
                value={`${entry.phase} · 模型${entry.modelReady ? '就绪' : '未就绪'} · 环境${entry.runtimeReady ? '就绪' : '未就绪'} · 设备${entry.deviceVerified ? '已验证' : '未验证'} · 硬件门槛${entry.hardwareEligible ? '满足' : `不满足（${entry.hardwareReason ?? '显存或内存不足'}）`}${entry.lastError ? ` · ${entry.lastError}` : ''}`}
              />)}
            </div>
            <p className="settings-boundary">打开本页不会启动后台进程，也不会生成性能排名；这里只展示宿主实际报告的运行环境事实。</p>
          </div>}

          {group === 'about' && <div className="settings-section about-v2">
            <div className="about-brand">
              <div className="about-copy"><span className="neutral-badge">{snapshot?.about.version ?? '0.1.1'} · Preview</span><h2>Image Studio</h2><p>本地图片创作工作台</p></div>
            </div>
            <InfoRow label="平台" value={`${snapshot?.about.productName ?? 'Image Studio'} · ${snapshot?.environment.platform ?? '—'}`}/>
            <InfoRow label="处理方式" value={snapshot?.about.localProcessing ? '模型推理在本机受管 worker 中执行' : '状态未知'}/>
            <InfoRow label="当前能力" value={snapshot?.about.capabilityScope ?? '正在读取'}/>
            <InfoRow label="示例来源" value={snapshot?.about.examples ?? '正在读取'}/>
            <InfoRow label="字体许可" value={snapshot?.about.fontLicense ?? '正在读取'}/>
            <div className="about-sources"><strong>模型与依赖来源</strong>{snapshot?.about.modelSources.map(source => <code key={source}>{source}</code>)}</div>
            <PromptLicenseSources/>
            <div className="about-boundaries">
              <span>自动更新：{snapshot?.about.autoUpdateSupported ? '可用' : '未提供'}</span>
              <span>在线模型服务：{snapshot?.about.onlineProviderSupported ? '可用' : '未提供'}</span>
              <span>存储迁移：{snapshot?.about.storageMigrationSupported ? '可用' : '未提供'}</span>
            </div>
          </div>}
        </>}
    </section>

    {pendingGroup && <div className="settings-confirm-backdrop">
      <div className="settings-confirm" role="dialog" aria-modal="true" aria-label="处理未保存的创作默认值">
        <h2>保存创作默认值？</h2>
        <p>你修改了默认画幅、模型或种子，尚未保存。切换分组前请选择如何处理。</p>
        <div>
          <button className="primary-action" onClick={() => void saveBeforeSwitch()}>保存并继续</button>
          <button className="secondary" onClick={discardAndSwitch}>放弃更改</button>
          <button className="secondary" onClick={() => setPendingGroup(null)}>取消</button>
        </div>
      </div>
    </div>}
  </div>;
}

function AppearanceSettings({
  appearance,
  status,
  error,
  onChange,
  onReset,
}: {
  appearance: AppearancePreferences;
  status: SaveStatus;
  error: string;
  onChange: (patch: Partial<AppearancePreferences>) => void;
  onReset: () => void;
}) {
  return <div className="settings-section">
    <SectionHeading title="界面偏好" text="改动会立即应用到界面，并写入独立的版本化应用偏好。" />
    <div className="theme-options settings-themes">
      {([
        ['light', '浅色', Sun],
        ['dark', '深色', Moon],
        ['system', '跟随系统', Monitor],
      ] as const).map(([value, label, Icon]) => <button key={value} className={`theme-option ${appearance.theme === value ? 'chosen' : ''}`} aria-pressed={appearance.theme === value} onClick={() => onChange({ theme: value })}>
        <div className={`theme-miniature ${value === 'dark' ? 'dark' : ''}`}><i/><div><span/><section/></div></div>
        <span><Icon size={13}/>{label}{appearance.theme === value && <span className="theme-check">✓</span>}</span>
      </button>)}
    </div>
    <div className="appearance-grid">
      <label><span>字号</span><select value={appearance.fontScale} onChange={event => onChange({ fontScale: event.target.value as AppearancePreferences['fontScale'] })}><option value="small">紧凑</option><option value="default">标准</option><option value="large">较大</option></select></label>
      <label><span>界面密度</span><select value={appearance.density} onChange={event => onChange({ density: event.target.value as AppearancePreferences['density'] })}><option value="comfortable">舒适</option><option value="compact">紧凑</option></select></label>
      <label className="toggle-row"><span><strong>减弱动效</strong><small>关闭非必要过渡与旋转</small></span><input type="checkbox" checked={appearance.reducedMotion} onChange={event => onChange({ reducedMotion: event.target.checked })}/></label>
      <label className="toggle-row"><span><strong>折叠会话侧栏</strong><small>跨重启保留侧栏状态</small></span><input type="checkbox" checked={appearance.sidebarCollapsed} onChange={event => onChange({ sidebarCollapsed: event.target.checked })}/></label>
    </div>
    <div className="defaults-actions">
      <button className="secondary" onClick={onReset}><RefreshCw size={13}/>恢复本组默认</button>
      {status === 'error' && <button className="secondary" onClick={() => onChange({})}>重试保存</button>}
      <span className={`save-status ${status === 'error' ? 'is-error' : ''}`} role="status">
        {status === 'saving' ? '正在保存外观偏好…' : status === 'error' ? `保存失败：${error}` : '外观偏好已保存到本机'}
      </span>
    </div>
  </div>;
}

function SectionHeading({ title, text }: { title: string; text: string }) {
  return <div className="settings-section-heading"><h2>{title}</h2><p>{text}</p></div>;
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return <div className="info-row"><span>{label}</span><strong>{value}</strong></div>;
}

function CheckIcon() {
  return <svg width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12 4 4L19 6" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/></svg>;
}

function PromptLicenseSources() {
  const [links, setLinks] = useState<PromptLicenseLinks>(() => promptLicenseLinks({
    upstreamRepository: PROMPT_LIBRARY_UPSTREAM_REPOSITORY,
    upstreamCommit: PROMPT_LIBRARY_UPSTREAM_COMMIT,
  }));
  const [loadError, setLoadError] = useState('');
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    if (!native) return;
    let disposed = false;
    api<PromptLibrarySnapshot>('prompt_library_snapshot')
      .then(value => {
        if (disposed) return;
        setLinks(promptLicenseLinks(value.catalog));
        setLoadError('');
      })
      .catch(cause => { if (!disposed) setLoadError(String(cause)); });
    return () => { disposed = true; };
  }, [revision]);
  return <div className="about-licenses">
    <h3>灵感库来源与许可</h3>
    <p>目录固定自上游 <code>{shortCommit(links.commit)}</code> 的只读快照；上游代码与目录采用 MIT 许可，第三方示例图片和提示词的商业使用、再分发条件独立且未统一验证。</p>
    <div className="about-license-links">
      <a href={links.licenseUrl} target="_blank" rel="noreferrer">上游 MIT 许可<ExternalLink size={10}/></a>
      <a href={links.disclaimerUrl} target="_blank" rel="noreferrer">第三方内容声明<ExternalLink size={10}/></a>
      <a href={links.repository} target="_blank" rel="noreferrer">上游仓库<ExternalLink size={10}/></a>
    </div>
    {loadError && <p className="error-text" role="alert">许可来源刷新失败：{loadError}<button type="button" onClick={() => setRevision(value => value + 1)}>重试</button></p>}
  </div>;
}

function modelAvailabilityNote(snapshot: SettingsSnapshot | null, modelId: string): string {
  const model = snapshot?.catalog.entries.find(entry => entry.id === modelId);
  if (!model) return '模型目录不可用；保存会被拒绝。';
  if (!model.hardwareEligible) return `当前设备不满足该型号的硬件门槛，保存会被拒绝：${model.hardwareReason ?? '显存或内存不足'}。`;
  if (model.productStatus === 'planned') return '该模型已登记但尚未接入；可作为默认选择保存，新会话会保留原因且不会自动替换。';
  if (!model.executable) return '该模型已登记但当前组件或运行环境未就绪；新会话会保留选择。';
  if (isNonCommercialLicense(model)) return '该模型当前可执行；许可见模型页，仅限非商业用途。';
  return '该模型当前可执行。';
}
