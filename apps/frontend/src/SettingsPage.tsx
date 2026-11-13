import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { listen } from '@tauri-apps/api/event';
import {
  AlertTriangle,
  Check,
  ChevronLeft,
  CircleHelp,
  Cpu,
  Download,
  ExternalLink,
  FolderOpen,
  HardDrive,
  LoaderCircle,
  Monitor,
  Moon,
  RefreshCw,
  SlidersHorizontal,
  Sun,
  UserRound,
  X,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { api, bytesLabel, native } from './studio-api';
import {
  ACCENT_PRESETS,
  normalizeAccentColor,
} from './settings-appearance';
import {
  UPDATE_PROGRESS_EVENT,
  canDownloadUpdate,
  canInstallUpdate,
  canOpenVerifiedInstaller,
  downloadPercent,
  formatBytesPerSecond,
  formatEta,
  formatUpdateTime,
  idleUpdateDownload,
  isDownloadActive,
  type AppVersionInfo,
  type UpdateCheckResult,
  type UpdateDownloadState,
} from './app-update';
import {
  builtInCreationDefaults,
  commonAvailableBytes,
  defaultAppearance,
  defaultsEqual,
  type AppearancePreferences,
  type CreationDefaults,
  type PreferencesEnvelope,
  type SettingsSnapshot,
  type UpdatePreferences,
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
import './settings-center.css';

type Group = 'appearance' | 'defaults' | 'storage' | 'runtime' | 'updates' | 'about';
type SaveStatus = 'saved' | 'saving' | 'error';
type PendingState = 'idle' | 'checking' | 'downloading' | 'cancelling' | 'opening' | 'installing';
type AboutLinkId =
  | 'repository'
  | 'releases'
  | 'issues'
  | 'author'
  | 'bilibili'
  | 'license'
  | 'thirdPartyNotices';

const groups: { id: Group; label: string; icon: LucideIcon }[] = [
  { id: 'appearance', label: '通用与外观', icon: Sun },
  { id: 'defaults', label: '创作默认值', icon: SlidersHorizontal },
  { id: 'storage', label: '存储', icon: HardDrive },
  { id: 'runtime', label: '运行环境', icon: Cpu },
  { id: 'updates', label: '更新', icon: Download },
  { id: 'about', label: '关于', icon: CircleHelp },
];

const validSeed = (value: string) => value === '' || (/^\d+$/.test(value) && Number(value) <= 4294967295);
const projectLinks: { id: AboutLinkId; label: string }[] = [
  { id: 'releases', label: '最新发布' },
  { id: 'issues', label: 'Issues 与反馈' },
  { id: 'license', label: '开源许可证' },
  { id: 'thirdPartyNotices', label: '第三方声明' },
];

export function SettingsPage({
  initialGroup = 'appearance',
  appearance,
  saveStatus,
  saveError,
  updatePreferences,
  updateCheck,
  updateCheckError,
  updateChecking,
  lastUpdateCheckAt,
  onBack,
  onAppearanceChange,
  onPreferencesReload,
  onUpdatePreferencesChange,
  onCheckUpdate,
  onUpdateViewed,
  onError,
}: {
  initialGroup?: Group;
  appearance: AppearancePreferences;
  saveStatus: SaveStatus;
  saveError: string;
  updatePreferences: UpdatePreferences;
  updateCheck: UpdateCheckResult | null;
  updateCheckError: string;
  updateChecking: boolean;
  lastUpdateCheckAt: number | null;
  onBack: () => void;
  onAppearanceChange: (value: AppearancePreferences) => void;
  onPreferencesReload: () => Promise<void>;
  onUpdatePreferencesChange: (value: UpdatePreferences) => void;
  onCheckUpdate: () => Promise<UpdateCheckResult>;
  onUpdateViewed: () => void;
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
  const [updateSaveStatus, setUpdateSaveStatus] = useState<'saved' | 'saving' | 'error'>('saved');
  const [updateSaveError, setUpdateSaveError] = useState('');
  const [updateInfo, setUpdateInfo] = useState<AppVersionInfo | null>(null);
  const [updateDownload, setUpdateDownload] = useState<UpdateDownloadState>(idleUpdateDownload);
  const [updatePending, setUpdatePending] = useState<PendingState>('idle');
  const [updateActionError, setUpdateActionError] = useState('');
  const [accentInput, setAccentInput] = useState(appearance.accentColor);
  const [accentError, setAccentError] = useState('');
  const headerRef = useRef<HTMLDivElement>(null);

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

  useEffect(() => {
    void load(true);
    const frame = window.requestAnimationFrame(() => headerRef.current?.focus());
    return () => window.cancelAnimationFrame(frame);
  }, []);

  useEffect(() => {
    setGroup(initialGroup);
  }, [initialGroup]);

  useEffect(() => {
    if (group === 'updates') onUpdateViewed();
  }, [group, onUpdateViewed]);

  useEffect(() => {
    if (!native) return;
    let disposed = false;
    let removeListener: (() => void) | undefined;
    void api<AppVersionInfo>('app_update_info')
      .then(value => { if (!disposed) setUpdateInfo(value); })
      .catch(error => { if (!disposed) setUpdateActionError(String(error)); });
    void api<UpdateDownloadState>('app_update_state')
      .then(value => { if (!disposed) setUpdateDownload(value); })
      .catch(error => { if (!disposed) setUpdateActionError(String(error)); });
    void listen<UpdateDownloadState>(UPDATE_PROGRESS_EVENT, event => {
      if (!disposed) setUpdateDownload(event.payload);
    }).then(unlisten => {
      if (disposed) unlisten();
      else removeListener = unlisten;
    }).catch(error => { if (!disposed) setUpdateActionError(String(error)); });
    return () => {
      disposed = true;
      removeListener?.();
    };
  }, []);

  useEffect(() => {
    setAccentInput(appearance.accentColor);
    setAccentError('');
  }, [appearance.accentColor]);

  const dirty = !defaultsEqual(defaults, savedDefaults);
  const selectedTextRecipe = textRecipeForModel(defaults.modelId);
  const validationError = useMemo(() => {
    if (!validSeed(defaults.seed)) return '种子须为 0–4294967295 的整数，或留空随机。';
    if (defaults.count !== 1) return '当前产品每轮只接受 1 张图片。';
    if (!['1:1', '4:3', '3:4'].includes(defaults.aspect)) return '当前产品不支持该画幅。';
    if (selectedTextRecipe && !selectedTextRecipe.aspects.includes(defaults.aspect)) {
      return `所选模型仅支持 ${selectedTextRecipe.aspects.join('、')}。`;
    }
    return '';
  }, [defaults, selectedTextRecipe]);
  const storageEntries = snapshot?.storage.entries ?? [];
  const sharedAvailableBytes = commonAvailableBytes(storageEntries);
  const storageBytes = storageEntries.reduce(
    (total, entry) => total + (entry.status === 'ready' || entry.status === 'partial' ? entry.bytes : 0),
    0,
  );

  const chooseGroup = (next: Group) => {
    if (next === group) {
      if (next === 'updates') onUpdateViewed();
      return;
    }
    if (group === 'defaults' && dirty) {
      setPendingGroup(next);
      return;
    }
    if (next === 'updates') onUpdateViewed();
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

  const applyAccent = (value: string) => {
    const normalized = normalizeAccentColor(value);
    if (!normalized) {
      setAccentError('请输入六位十六进制颜色。');
      return false;
    }
    setAccentError('');
    setAccentInput(normalized);
    updateAppearance({ accentColor: normalized });
    return true;
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
        if (nextGroup === 'updates') onUpdateViewed();
        setGroup(nextGroup);
      }
      return true;
    } catch (cause) {
      setDefaultsError(String(cause));
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
    if (target) {
      if (target === 'updates') onUpdateViewed();
      setGroup(target);
    }
  };

  const reloadAll = async () => {
    setLoading(true);
    setLoadError('');
    try {
      await onPreferencesReload();
      await load();
      if (native) setUpdateDownload(await api<UpdateDownloadState>('app_update_state'));
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

  const openAboutLink = async (linkId: AboutLinkId) => {
    try { await api('about_open_link', { linkId }); }
    catch (cause) { onError(String(cause)); }
  };

  const saveUpdatePreferences = async (patch: Partial<UpdatePreferences>) => {
    const previous = updatePreferences;
    const next = { ...previous, ...patch };
    onUpdatePreferencesChange(next);
    setSnapshot(current => current ? {
      ...current,
      preferences: { ...current.preferences, updates: next },
    } : current);
    setUpdateSaveStatus('saving');
    setUpdateSaveError('');
    try {
      const result = await api<PreferencesEnvelope>('update_preferences_save', { preferences: next });
      const saved = result.preferences.updates;
      onUpdatePreferencesChange(saved);
      setSnapshot(current => current ? {
        ...current,
        preferences: result.preferences,
        catalog: current.catalog,
      } : current);
      setUpdateSaveStatus('saved');
    } catch (cause) {
      onUpdatePreferencesChange(previous);
      setSnapshot(current => current ? {
        ...current,
        preferences: { ...current.preferences, updates: previous },
      } : current);
      setUpdateSaveStatus('error');
      setUpdateSaveError(String(cause));
    }
  };

  const checkUpdate = async () => {
    setUpdatePending('checking');
    setUpdateActionError('');
    try {
      await onCheckUpdate();
    } catch (cause) {
      setUpdateActionError(String(cause));
    } finally {
      setUpdatePending('idle');
    }
  };

  const startDownload = async (version: string) => {
    setUpdatePending('downloading');
    setUpdateActionError('');
    try {
      setUpdateDownload(await api<UpdateDownloadState>('app_update_download_start', { version }));
    } catch (cause) {
      setUpdateActionError(String(cause));
    } finally {
      setUpdatePending('idle');
    }
  };

  const cancelDownload = async () => {
    setUpdatePending('cancelling');
    setUpdateActionError('');
    try {
      setUpdateDownload(await api<UpdateDownloadState>('app_update_download_cancel'));
    } catch (cause) {
      setUpdateActionError(String(cause));
    } finally {
      setUpdatePending('idle');
    }
  };

  const revealDownload = async () => {
    setUpdatePending('opening');
    setUpdateActionError('');
    try {
      await api('app_update_open_location');
    } catch (cause) {
      setUpdateActionError(String(cause));
    } finally {
      setUpdatePending('idle');
    }
  };

  const openInstaller = async () => {
    setUpdatePending('opening');
    setUpdateActionError('');
    try {
      await api('app_update_open_installer');
    } catch (cause) {
      setUpdateActionError(String(cause));
    } finally {
      setUpdatePending('idle');
    }
  };

  const installUpdate = async () => {
    setUpdatePending('installing');
    setUpdateActionError('');
    try {
      await api('app_update_install');
    } catch (cause) {
      setUpdateActionError(String(cause));
      setUpdatePending('idle');
    }
  };

  const activeSaveError = group === 'updates'
    ? updateSaveError
    : group === 'defaults'
      ? defaultsError
      : saveError;
  const activeSaveState = group === 'updates'
    ? updateSaveStatus
    : group === 'defaults'
      ? defaultsSaveStatus === 'error'
        ? 'error'
        : defaultsSaveStatus === 'saving'
          ? 'saving'
          : defaultsSaveStatus === 'saved'
            ? 'saved'
            : null
      : group === 'appearance'
        ? saveStatus
        : null;

  return <div className="settings-center">
    <aside className="settings-nav-panel">
      <div className="settings-nav-title">设置</div>
      <button className="settings-back" onClick={onBack}><ChevronLeft size={15}/>返回创作</button>
      <nav className="settings-groups" aria-label="设置分类">
        {groups.map(({ id, label, icon: Icon }) => <button
          key={id}
          className={`settings-group ${group === id ? 'is-selected' : ''}`}
          aria-current={group === id ? 'page' : undefined}
          onClick={() => chooseGroup(id)}
        >
          <Icon size={16}/><span>{label}</span>
          {id === 'defaults' && dirty && <i className="settings-unsaved-dot" aria-label="有未保存的创作默认值"/>}
        </button>)}
      </nav>
    </aside>

    <section className="settings-content-panel">
      <header className="settings-content-header">
        <div ref={headerRef} tabIndex={-1} className="settings-content-heading">
          <h1>{groups.find(item => item.id === group)?.label}</h1>
          {activeSaveState && <SaveIndicator status={activeSaveState} error={activeSaveError}/>}
        </div>
        <button className="settings-icon-button" aria-label="重新读取设置" title="重新读取设置" disabled={loading} onClick={() => void reloadAll()}>
          <RefreshCw size={15} className={loading ? 'spin' : ''}/>
        </button>
      </header>

      <div className="settings-content-scroll">
        {loading && !snapshot
          ? <div className="settings-loading"><LoaderCircle className="spin" size={19}/>正在读取设置…</div>
          : <>
            {loadError && <div className="settings-alert is-error" role="alert">
              <AlertTriangle size={14}/><span>{loadError}</span><button onClick={() => void reloadAll()}>重试</button>
            </div>}
            {snapshot?.preferences.loadError && <div className="settings-alert is-error" role="alert">
              <AlertTriangle size={14}/><span>{snapshot.preferences.loadError}</span><button onClick={() => void reloadAll()}>重试</button>
            </div>}
            {!!snapshot?.preferences.warnings.length && <div className="settings-alert is-warning">
              <AlertTriangle size={14}/><span>{snapshot.preferences.warnings[0]}</span>
            </div>}

            {group === 'appearance' && <AppearanceSettings
              appearance={appearance}
              accentInput={accentInput}
              accentError={accentError}
              status={saveStatus}
              error={saveError}
              onAccentInput={value => { setAccentInput(value); applyAccent(value); }}
              onAccentCommit={applyAccent}
              onChange={updateAppearance}
              onReset={() => {
                const next = { ...defaultAppearance };
                setAccentInput(next.accentColor);
                setAccentError('');
                updateAppearance(next);
              }}
            />}

            {group === 'defaults' && <div className="settings-page-stack">
              <SettingsSection title="新会话默认">
                <SettingsRow label="模型" alignTop>
                  <div className="settings-control-stack">
                    <select value={defaults.modelId} aria-label="默认模型" onChange={event => setDefaults({ ...defaults, modelId: event.target.value })}>
                      {(snapshot?.catalog.entries ?? []).filter(model => model.productStatus === 'implemented' && model.capabilities.some(capability => capability.id === 'text_to_image')).map(model => <option key={model.id} value={model.id} disabled={!model.hardwareEligible}>
                        {model.displayName}
                      </option>)}
                    </select>
                    <ModelStatus snapshot={snapshot} modelId={defaults.modelId}/>
                  </div>
                </SettingsRow>
                <SettingsRow label="画幅">
                  <Segmented
                    value={defaults.aspect}
                    options={(selectedTextRecipe?.aspects ?? ['1:1', '4:3', '3:4']).map(aspect => ({ value: aspect, label: aspect }))}
                    onChange={aspect => setDefaults({ ...defaults, aspect })}
                    label="默认画幅"
                  />
                </SettingsRow>
                <SettingsRow label="每轮数量">
                  <input className="settings-number-input" type="number" min={1} max={1} value={defaults.count} onChange={event => setDefaults({ ...defaults, count: Number(event.target.value) })} aria-label="每轮图片数量"/>
                </SettingsRow>
                <SettingsRow label="随机种子">
                  <input className="settings-text-input" inputMode="numeric" value={defaults.seed} aria-invalid={!validSeed(defaults.seed)} placeholder="留空" onChange={event => setDefaults({ ...defaults, seed: event.target.value })} aria-label="随机种子"/>
                </SettingsRow>
              </SettingsSection>
              {(validationError || defaultsError) && <div className="settings-inline-error" role="alert">{defaultsError || validationError}</div>}
              <div className="settings-page-actions">
                <button className="primary-action" disabled={!dirty || !!validationError || defaultsSaveStatus === 'saving'} onClick={() => void persistDefaults(defaults)}>
                  {defaultsSaveStatus === 'saving' ? <LoaderCircle className="spin" size={14}/> : <Check size={14}/>}
                  保存
                </button>
                <button className="secondary" disabled={!dirty || defaultsSaveStatus === 'saving'} onClick={() => { setDefaults(savedDefaults); setDefaultsError(''); setDefaultsSaveStatus('idle'); }}>放弃</button>
                <button className="secondary" onClick={() => void persistDefaults(builtInCreationDefaults)}>恢复默认</button>
              </div>
            </div>}

            {group === 'storage' && <div className="settings-page-stack">
              <div className="storage-summary">
                <div><span>总占用</span><strong>{bytesLabel(storageBytes)}</strong></div>
                <div><span>可用空间</span><strong>{sharedAvailableBytes === null ? '—' : bytesLabel(sharedAvailableBytes)}</strong></div>
              </div>
              <SettingsSection title="存储位置">
                {storageEntries.length > 0
                  ? storageEntries.map(entry => <StorageLocation
                    key={entry.id}
                    entry={entry}
                    sharedAvailableBytes={sharedAvailableBytes}
                    pending={storagePending === entry.id}
                    onOpen={() => void openStorage(entry.id)}
                  />)
                  : <div className="settings-empty-row">尚未读取到登记目录</div>}
              </SettingsSection>
              <div className="settings-boundary-row">
                <span>迁移与删除</span><StatusBadge tone="muted" label="未提供"/>
              </div>
            </div>}

            {group === 'runtime' && <div className="settings-page-stack">
              <SettingsSection title="平台与设备">
                <FactRow label="平台" value={`${snapshot?.environment.platform ?? '—'} · ${snapshot?.environment.architecture ?? '—'}`}/>
                <FactRow label="设备" value={snapshot?.environment.deviceFacts ?? '尚未读取'}/>
                <FactRow label="运行时" value={`${snapshot?.environment.runtimeName ?? '—'} ${snapshot?.environment.runtimeVersion ?? ''} · Python ${snapshot?.environment.pythonVersion ?? '—'}`}/>
                <SettingsRow label="运行时就绪">
                  <StatusBadge tone={snapshot?.environment.runtime.runtimeReady ? 'positive' : 'danger'} label={snapshot?.environment.runtime.runtimeReady ? '已就绪' : '未就绪'}/>
                </SettingsRow>
                <SettingsRow label="设备探针">
                  <StatusBadge tone={snapshot?.environment.runtime.deviceVerified ? 'positive' : 'muted'} label={snapshot?.environment.runtime.deviceVerified ? '已验证' : '未验证'}/>
                </SettingsRow>
                {snapshot?.environment.runtime.lastError && <FactRow label="错误" value={snapshot.environment.runtime.lastError} isError/>}
              </SettingsSection>
              <SettingsSection title="模型运行组合">
                {(snapshot?.environment.runtimes ?? []).length > 0
                  ? snapshot!.environment.runtimes.map(entry => <RuntimeCombination key={entry.installId} entry={entry}/>)
                  : <div className="settings-empty-row">尚未读取到运行组合</div>}
              </SettingsSection>
            </div>}

            {group === 'updates' && <UpdateSettings
              snapshot={snapshot}
              updateInfo={updateInfo}
              updatePreferences={updatePreferences}
              updateCheck={updateCheck}
              checkError={updateCheckError}
              checking={updateChecking}
              actionError={updateActionError}
              lastUpdateCheckAt={lastUpdateCheckAt}
              download={updateDownload}
              pending={updatePending}
              saveStatus={updateSaveStatus}
              saveError={updateSaveError}
              onCheck={() => void checkUpdate()}
              onSavePreferences={patch => void saveUpdatePreferences(patch)}
              onDownload={version => void startDownload(version)}
              onCancel={() => void cancelDownload()}
              onReveal={() => void revealDownload()}
              onOpenInstaller={() => void openInstaller()}
              onInstall={() => void installUpdate()}
            />}

            {group === 'about' && <div className="settings-page-stack">
              <BrandSummary
                onOpenRepository={() => void openAboutLink('repository')}
                onOpenLink={id => void openAboutLink(id)}
              />
              <SettingsSection title="版本与构建">
                <FactRow label="版本" value={snapshot?.about.version ?? updateInfo?.version ?? '—'}/>
                <FactRow label="构建" value={`Tauri 2 · ${snapshot?.environment.platform ?? '—'} · ${snapshot?.environment.architecture ?? '—'}`}/>
                <FactRow label="许可" value="Apache-2.0"/>
              </SettingsSection>
              <SettingsSection title="作者">
                <div className="settings-author-row">
                  <span className="settings-author-mark"><UserRound size={17}/></span>
                  <div>
                    <strong>Goldloli 小卡塔克</strong>
                    <span>© 2026</span>
                  </div>
                </div>
                <div className="settings-author-actions">
                  <button className="secondary" onClick={() => void openAboutLink('author')}><GithubMark/>GitHub</button>
                  <button className="secondary" onClick={() => void openAboutLink('bilibili')}>Bilibili</button>
                </div>
              </SettingsSection>
              <SettingsSection title="能力与证据">
                <FactRow label="处理方式" value={snapshot?.about.localProcessing ? '本机受管 worker' : '状态未知'}/>
                <FactRow label="当前能力" value={snapshot?.about.capabilityScope ?? '正在读取'}/>
                <FactRow label="示例来源" value={snapshot?.about.examples ?? '正在读取'}/>
                <details className="settings-evidence about-evidence">
                  <summary>模型、字体与内容许可</summary>
                  <FactRow label="字体许可" value={snapshot?.about.fontLicense ?? '正在读取'}/>
                  <div className="settings-source-list">
                    {(snapshot?.about.modelSources ?? []).map(source => <code key={source}>{source}</code>)}
                  </div>
                  <PromptLicenseSources/>
                </details>
              </SettingsSection>
              <SettingsSection title="服务边界">
                <SettingsRow label="在线模型服务"><StatusBadge tone="muted" label={snapshot?.about.onlineProviderSupported ? '可用' : '未提供'}/></SettingsRow>
                <SettingsRow label="存储迁移"><StatusBadge tone="muted" label={snapshot?.about.storageMigrationSupported ? '可用' : '未提供'}/></SettingsRow>
                <button className="settings-link-row" onClick={() => chooseGroup('updates')}>
                  <span>应用更新</span><span>{updateCheck?.updateAvailable && updateCheck.release ? `有可用版本 ${updateCheck.release.version}` : '查看更新状态'}</span><ChevronLeft className="settings-link-chevron" size={14}/>
                </button>
              </SettingsSection>
            </div>}
          </>}
      </div>
    </section>

    {pendingGroup && <div className="settings-confirm-backdrop">
      <div className="settings-confirm" role="dialog" aria-modal="true" aria-label="处理未保存的创作默认值">
        <h2>保存创作默认值？</h2>
        <p>切换分类前需要保存或放弃当前修改。</p>
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
  accentInput,
  accentError,
  status,
  error,
  onAccentInput,
  onAccentCommit,
  onChange,
  onReset,
}: {
  appearance: AppearancePreferences;
  accentInput: string;
  accentError: string;
  status: SaveStatus;
  error: string;
  onAccentInput: (value: string) => void;
  onAccentCommit: (value: string) => boolean;
  onChange: (patch: Partial<AppearancePreferences>) => void;
  onReset: () => void;
}) {
  return <div className="settings-page-stack">
    <SettingsSection title="外观">
      <SettingsRow label="主题">
        <Segmented
          value={appearance.theme}
          options={[
            { value: 'light', label: <><Sun size={13}/>浅色</> },
            { value: 'dark', label: <><Moon size={13}/>深色</> },
            { value: 'system', label: <><Monitor size={13}/>系统</> },
          ]}
          onChange={value => onChange({ theme: value as AppearancePreferences['theme'] })}
          label="主题模式"
        />
      </SettingsRow>
      <SettingsRow label="强调色" alignTop>
        <div className="settings-accent-editor">
          <div className="settings-swatches" role="group" aria-label="强调色预设">
            {ACCENT_PRESETS.map(preset => <button
              key={preset.id}
              className={`settings-swatch ${appearance.accentColor === preset.value ? 'is-selected' : ''}`}
              style={{ background: preset.value }}
              aria-label={preset.label}
              aria-pressed={appearance.accentColor === preset.value}
              title={preset.label}
              onClick={() => onAccentCommit(preset.value)}
            />)}
          </div>
          <div className="settings-color-input">
            <input type="color" value={normalizeAccentColor(accentInput) ?? appearance.accentColor} aria-label="自定义强调色取色器" onChange={event => onAccentInput(event.target.value)}/>
            <input value={accentInput} aria-label="自定义强调色" aria-invalid={!!accentError} spellCheck={false} onChange={event => onAccentInput(event.target.value)} onBlur={() => onAccentCommit(accentInput)}/>
          </div>
          {accentError && <span className="settings-field-error" role="alert">{accentError}</span>}
        </div>
      </SettingsRow>
      <SettingsRow label="字号">
        <select value={appearance.fontScale} aria-label="字号" onChange={event => onChange({ fontScale: event.target.value as AppearancePreferences['fontScale'] })}>
          <option value="small">紧凑</option>
          <option value="default">标准</option>
          <option value="large">较大</option>
        </select>
      </SettingsRow>
      <SettingsRow label="界面密度">
        <Segmented
          value={appearance.density}
          options={[{ value: 'comfortable', label: '舒适' }, { value: 'compact', label: '紧凑' }]}
          onChange={value => onChange({ density: value as AppearancePreferences['density'] })}
          label="界面密度"
        />
      </SettingsRow>
    </SettingsSection>
    <SettingsSection title="交互">
      <SettingsRow label="减弱动效">
        <input className="settings-switch" type="checkbox" checked={appearance.reducedMotion} aria-label="减弱动效" onChange={event => onChange({ reducedMotion: event.target.checked })}/>
      </SettingsRow>
      <SettingsRow label="折叠会话侧栏">
        <input className="settings-switch" type="checkbox" checked={appearance.sidebarCollapsed} aria-label="折叠会话侧栏" onChange={event => onChange({ sidebarCollapsed: event.target.checked })}/>
      </SettingsRow>
    </SettingsSection>
    <div className="settings-page-actions">
      <button className="secondary" onClick={onReset}><RefreshCw size={13}/>恢复默认</button>
      {status === 'error' && <button className="secondary" onClick={() => onChange({})}>重试保存</button>}
      {status === 'error' && <span className="settings-inline-error" role="alert">{error}</span>}
    </div>
  </div>;
}

function UpdateSettings({
  snapshot,
  updateInfo,
  updatePreferences,
  updateCheck,
  checkError,
  checking,
  actionError,
  lastUpdateCheckAt,
  download,
  pending,
  saveStatus,
  saveError,
  onCheck,
  onSavePreferences,
  onDownload,
  onCancel,
  onReveal,
  onOpenInstaller,
  onInstall,
}: {
  snapshot: SettingsSnapshot | null;
  updateInfo: AppVersionInfo | null;
  updatePreferences: UpdatePreferences;
  updateCheck: UpdateCheckResult | null;
  checkError: string;
  checking: boolean;
  actionError: string;
  lastUpdateCheckAt: number | null;
  download: UpdateDownloadState;
  pending: PendingState;
  saveStatus: 'saved' | 'saving' | 'error';
  saveError: string;
  onCheck: () => void;
  onSavePreferences: (patch: Partial<UpdatePreferences>) => void;
  onDownload: (version: string) => void;
  onCancel: () => void;
  onReveal: () => void;
  onOpenInstaller: () => void;
  onInstall: () => void;
}) {
  const currentVersion = updateInfo?.version ?? snapshot?.about.version ?? '—';
  const target = updateInfo?.target ?? `${snapshot?.environment.platform ?? '—'} · ${snapshot?.environment.architecture ?? '—'}`;
  const release = updateCheck?.updateAvailable ? updateCheck.release : null;
  const canDownload = canDownloadUpdate(updateCheck, download);
  const verifiedCurrent = !!release
    && download.status === 'completed'
    && normalizeVersion(download.version) === normalizeVersion(release.version);
  const installable = verifiedCurrent && canInstallUpdate(download, updateInfo?.installerKind ?? null, release?.version);
  const openable = verifiedCurrent && canOpenVerifiedInstaller(download, updateInfo?.installerKind ?? null, release?.version);
  const cancellable = isDownloadActive(download.status);
  const percent = downloadPercent(download);
  const busy = pending !== 'idle' || checking;
  const checkFailed = !!checkError || (!!actionError && !release);
  const status = checking
    ? { tone: 'muted' as const, label: '检查中' }
    : checkFailed
    ? { tone: 'danger' as const, label: '检查失败' }
    : updateCheck?.updateAvailable
      ? { tone: 'positive' as const, label: '可更新' }
      : updateCheck
        ? { tone: 'muted' as const, label: '已是最新' }
        : { tone: 'muted' as const, label: '尚未检查' };

  return <div className="settings-page-stack">
    <SettingsSection title="版本与检查">
      <FactRow label="当前版本" value={currentVersion}/>
      <FactRow label="平台" value={target}/>
      <SettingsRow label="状态">
        <div className="settings-status-with-time">
          <StatusBadge tone={status.tone} label={status.label}/>
          {!checking && <span>{formatUpdateTime(lastUpdateCheckAt)}</span>}
        </div>
      </SettingsRow>
      <SettingsRow label="自动检查">
        <input className="settings-switch" type="checkbox" checked={updatePreferences.autoCheck} disabled={saveStatus === 'saving'} aria-label="自动检查更新" onChange={event => onSavePreferences({ autoCheck: event.target.checked })}/>
      </SettingsRow>
      <SettingsRow label="更新通知">
        <input className="settings-switch" type="checkbox" checked={updatePreferences.notifications} disabled={saveStatus === 'saving'} aria-label="更新通知" onChange={event => onSavePreferences({ notifications: event.target.checked })}/>
      </SettingsRow>
      <div className="settings-row-actions">
        <button className="primary-action" disabled={busy} onClick={onCheck}>
          {checking || pending === 'checking' ? <LoaderCircle className="spin" size={14}/> : <RefreshCw size={14}/>}
          检查更新
        </button>
        {!release && <a className="settings-text-link" href={updateCheck?.releasesUrl ?? 'https://github.com/Goldloli/image-studio/releases'} target="_blank" rel="noreferrer">发布页<ExternalLink size={11}/></a>}
      </div>
      {saveStatus === 'error' && <div className="settings-inline-error" role="alert">{saveError}</div>}
      {(checkError || (checkError === '' && actionError && !release)) && <div className="settings-inline-error" role="alert">{checkError || actionError}</div>}
    </SettingsSection>

    {release && <SettingsSection title={`可用版本 ${release.version}`}>
      <FactRow label="发布时间" value={formatUpdateTime(release.publishedAt)}/>
      <FactRow label="安装包" value={`${release.assetName ?? '资产不完整'} · ${release.assetSize === null ? '—' : bytesLabel(release.assetSize)}`}/>
      <div className="settings-release-notes">{release.notes || '无发布说明'}</div>
      <div className="settings-row-actions">
        <a className="settings-text-link" href={release.htmlUrl} target="_blank" rel="noreferrer">查看发布说明<ExternalLink size={11}/></a>
        {canDownload && <button className="primary-action" disabled={busy} onClick={() => onDownload(release.version)}><Download size={14}/>下载</button>}
      </div>
    </SettingsSection>}

    {!!release && <SettingsSection title="下载与安装">
      <DownloadStatus download={download} percent={percent}/>
      {cancellable && <div className="settings-row-actions">
        <button className="secondary" disabled={pending === 'cancelling'} onClick={onCancel}>
          {pending === 'cancelling' ? <LoaderCircle className="spin" size={13}/> : <X size={13}/>}取消下载
        </button>
      </div>}
      {download.status === 'completed' && verifiedCurrent && <div className="settings-row-actions">
        <button className="secondary" disabled={busy} onClick={onReveal}><FolderOpen size={13}/>打开文件位置</button>
        {openable && <button className="primary-action" disabled={busy} onClick={onOpenInstaller}><ExternalLink size={13}/>打开 DMG</button>}
        {installable && <button className="primary-action" disabled={busy} onClick={onInstall}>
          {pending === 'installing' ? <LoaderCircle className="spin" size={13}/> : <Download size={13}/>}
          安装并重启
        </button>}
      </div>}
      {openable && <div className="settings-boundary-row"><span>安装方式</span><strong>需在 DMG 中手动拖入“应用程序”</strong></div>}
      {actionError && release && <div className="settings-inline-error" role="alert">{actionError}</div>}
    </SettingsSection>}
  </div>;
}

function DownloadStatus({ download, percent }: { download: UpdateDownloadState; percent: number | null }) {
  if (download.status === 'idle') {
    return <div className="settings-quiet-status"><Download size={13}/>尚未下载</div>;
  }
  if (download.status === 'error') {
    return <div className="settings-download-status is-error"><AlertTriangle size={14}/><span>{download.error ?? '下载失败'}</span></div>;
  }
  if (download.status === 'cancelled') {
    return <div className="settings-download-status"><X size={14}/><span>下载已取消</span></div>;
  }
  if (download.status === 'completed') {
    return <div className="settings-download-status is-positive"><Check size={14}/><span>SHA-256 校验通过 · {download.filename ?? '安装包'}</span></div>;
  }
  const label = download.status === 'preparing'
    ? '正在准备下载'
    : download.status === 'verifying'
      ? '正在校验 SHA-256'
      : download.status === 'cancelling'
        ? '正在取消'
        : '正在下载';
  return <div className="settings-download-progress">
    <div className="settings-progress-heading">
      <span>{label}</span>
      <span>{percent === null ? `${bytesLabel(download.downloadedBytes)}` : `${percent.toFixed(1)}%`}</span>
    </div>
    <progress max={100} value={percent ?? undefined} aria-label={label}/>
    <div className="settings-progress-meta">
      <span>{bytesLabel(download.downloadedBytes)} / {download.totalBytes === null ? '—' : bytesLabel(download.totalBytes)}</span>
      <span>{formatBytesPerSecond(download.bytesPerSecond)} · {formatEta(download.etaSeconds)}</span>
    </div>
  </div>;
}

function StorageLocation({
  entry,
  sharedAvailableBytes,
  pending,
  onOpen,
}: {
  entry: SettingsSnapshot['storage']['entries'][number];
  sharedAvailableBytes: number | null;
  pending: boolean;
  onOpen: () => void;
}) {
  const status = entry.status === 'ready'
    ? { tone: 'positive' as const, label: '可用' }
    : entry.status === 'missing'
      ? { tone: 'muted' as const, label: '未创建' }
      : entry.status === 'partial'
        ? { tone: 'warning' as const, label: '部分可读' }
        : { tone: 'danger' as const, label: '读取失败' };
  return <div className="storage-location">
    <div className="storage-location-copy">
      <div><strong>{entry.label}</strong><StatusBadge {...status}/></div>
      <code>{entry.path}</code>
      {entry.error && <span className="settings-field-error">{entry.error}</span>}
    </div>
    <div className="storage-location-meta">
      <strong>{entry.status === 'error' ? '—' : bytesLabel(entry.bytes)}</strong>
      {sharedAvailableBytes === null && <span>{entry.availableBytes === null ? '可用空间未知' : `可用 ${bytesLabel(entry.availableBytes)}`}</span>}
    </div>
    <button className="settings-icon-button" aria-label={`定位${entry.label}`} title={`定位${entry.label}`} disabled={entry.status === 'missing' || pending} onClick={onOpen}>
      {pending ? <LoaderCircle className="spin" size={13}/> : <FolderOpen size={13}/>}
    </button>
  </div>;
}

function RuntimeCombination({ entry }: { entry: SettingsSnapshot['environment']['runtimes'][number] }) {
  const ready = entry.phase === 'ready';
  const phase = runtimePhaseLabel(entry.phase);
  return <div className="runtime-combination">
    <div className="runtime-combination-heading">
      <span>{entry.label}</span>
      <StatusBadge tone={ready ? 'positive' : entry.lastError ? 'danger' : 'muted'} label={phase}/>
    </div>
    {entry.runtimeLockId && <code>{entry.runtimeLockId}</code>}
    <div className="runtime-badges">
      <StatusBadge tone={entry.modelReady ? 'positive' : 'muted'} label={entry.modelReady ? '模型就绪' : '模型未就绪'}/>
      <StatusBadge tone={entry.runtimeReady ? 'positive' : 'muted'} label={entry.runtimeReady ? '环境就绪' : '环境未就绪'}/>
      <StatusBadge tone={entry.deviceVerified ? 'positive' : 'muted'} label={entry.deviceVerified ? '设备已验证' : '设备未验证'}/>
      <StatusBadge tone={entry.hardwareEligible ? 'positive' : 'danger'} label={entry.hardwareEligible ? '硬件达标' : '硬件不足'} title={entry.hardwareReason ?? undefined}/>
    </div>
    {entry.lastError && <div className="settings-field-error" role="alert">{entry.lastError}</div>}
  </div>;
}

function runtimePhaseLabel(phase: string): string {
  return {
    not_installed: '未安装',
    preparing: '准备中',
    preparing_model: '准备模型',
    downloading: '下载中',
    verifying: '校验中',
    installing_runtime: '安装运行时',
    probing: '探测中',
    ready: '已就绪',
    failed: '安装失败',
    cancelled: '已取消',
  }[phase] ?? phase;
}

function BrandSummary({
  onOpenRepository,
  onOpenLink,
}: {
  onOpenRepository: () => void;
  onOpenLink: (id: AboutLinkId) => void;
}) {
  return <div className="settings-brand-summary">
    <div className="settings-brand-main">
      <img src="/brand/app-icon-ui.png" alt="Image Studio 应用图标"/>
      <div>
        <div className="settings-brand-name">Image Studio</div>
        <div className="settings-brand-badges">
          <StatusBadge tone="positive" label="本地处理"/>
        </div>
      </div>
    </div>
    <button className="settings-github-button" onClick={onOpenRepository}><GithubMark/>GitHub<ExternalLink size={11}/></button>
    <div className="settings-brand-links" role="group" aria-label="项目链接">
      {projectLinks.map(link => <button key={link.id} onClick={() => onOpenLink(link.id)}>{link.label}</button>)}
    </div>
  </div>;
}

function GithubMark() {
  return <svg className="settings-github-mark" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 .5C5.65.5.5 5.65.5 12c0 5.08 3.29 9.39 7.86 10.91.58.11.79-.25.79-.55 0-.27-.01-1.17-.02-2.12-3.2.7-3.88-1.36-3.88-1.36-.52-1.33-1.28-1.68-1.28-1.68-1.04-.71.08-.7.08-.7 1.15.08 1.76 1.19 1.76 1.19 1.03 1.76 2.7 1.25 3.36.96.1-.75.4-1.25.72-1.54-2.55-.29-5.23-1.28-5.23-5.68 0-1.26.45-2.28 1.19-3.09-.12-.29-.52-1.46.11-3.05 0 0 .97-.31 3.18 1.18a11.1 11.1 0 0 1 5.8 0c2.2-1.49 3.17-1.18 3.17-1.18.63 1.59.23 2.76.11 3.05.74.81 1.19 1.83 1.19 3.09 0 4.41-2.69 5.38-5.25 5.67.41.35.77 1.05.77 2.12 0 1.53-.01 2.76-.01 3.14 0 .3.2.67.8.55A11.51 11.51 0 0 0 23.5 12C23.5 5.65 18.35.5 12 .5Z"/></svg>;
}

function ModelStatus({ snapshot, modelId }: { snapshot: SettingsSnapshot | null; modelId: string }) {
  const model = snapshot?.catalog.entries.find(entry => entry.id === modelId);
  if (!model) return <StatusBadge tone="danger" label="模型目录不可用"/>;
  if (!model.hardwareEligible) return <StatusBadge tone="danger" label="硬件不足" title={model.hardwareReason ?? undefined}/>;
  if (model.productStatus === 'planned') return <StatusBadge tone="muted" label="尚未接入"/>;
  if (!model.executable) return <StatusBadge tone="muted" label="环境未就绪"/>;
  if (isNonCommercialLicense(model)) return <StatusBadge tone="warning" label="非商业许可"/>;
  return <StatusBadge tone="positive" label="可执行"/>;
}

function SettingsSection({ title, children }: { title: string; children: ReactNode }) {
  return <section className="settings-section-card">
    <h2>{title}</h2>
    <div className="settings-section-rows">{children}</div>
  </section>;
}

function SettingsRow({ label, children, alignTop = false }: { label: string; children: ReactNode; alignTop?: boolean }) {
  return <div className={`settings-row ${alignTop ? 'is-top' : ''}`}>
    <span>{label}</span>
    <div className="settings-row-control">{children}</div>
  </div>;
}

function FactRow({ label, value, isError = false }: { label: string; value: string; isError?: boolean }) {
  return <div className={`settings-row is-fact ${isError ? 'is-error' : ''}`}>
    <span>{label}</span>
    <strong>{value}</strong>
  </div>;
}

function Segmented({
  value,
  options,
  onChange,
  label,
}: {
  value: string;
  options: { value: string; label: ReactNode }[];
  onChange: (value: string) => void;
  label: string;
}) {
  return <div className="settings-segmented" role="group" aria-label={label}>
    {options.map(option => <button
      key={option.value}
      className={option.value === value ? 'is-selected' : ''}
      aria-pressed={option.value === value}
      onClick={() => onChange(option.value)}
    >{option.label}</button>)}
  </div>;
}

function StatusBadge({ tone, label, title }: { tone: 'positive' | 'warning' | 'danger' | 'muted'; label: string; title?: string }) {
  return <span className={`settings-status-badge is-${tone}`} title={title}>{label}</span>;
}

function SaveIndicator({ status, error }: { status: SaveStatus; error: string }) {
  if (status === 'saving') return <span className="settings-save-indicator"><LoaderCircle className="spin" size={11}/>保存中</span>;
  if (status === 'error') return <span className="settings-save-indicator is-error" title={error}><AlertTriangle size={11}/>保存失败</span>;
  return <span className="settings-save-indicator"><Check size={11}/>已保存</span>;
}

function normalizeVersion(value: string | null): string {
  return (value ?? '').trim().replace(/^v/i, '');
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
  return <div className="settings-license-sources">
    <span>灵感库固定自上游 <code>{shortCommit(links.commit)}</code> 只读快照</span>
    <div>
      <a href={links.licenseUrl} target="_blank" rel="noreferrer">MIT 许可<ExternalLink size={10}/></a>
      <a href={links.disclaimerUrl} target="_blank" rel="noreferrer">第三方内容声明<ExternalLink size={10}/></a>
      <a href={links.repository} target="_blank" rel="noreferrer">上游仓库<ExternalLink size={10}/></a>
    </div>
    {loadError && <span className="settings-field-error">许可来源刷新失败：{loadError}<button type="button" onClick={() => setRevision(value => value + 1)}>重试</button></span>}
  </div>;
}
