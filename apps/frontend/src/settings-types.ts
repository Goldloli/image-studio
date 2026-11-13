import type { CatalogPayload } from './model-catalog';
import { Z_IMAGE_CAPABILITY_FALLBACK, Z_IMAGE_MODEL_ID, textRecipeForModel } from './model-recipes';

export type ThemeMode = 'light' | 'dark' | 'system';
export type FontScale = 'small' | 'default' | 'large';
export type Density = 'comfortable' | 'compact';
export type AppearancePreferences = {
  theme: ThemeMode;
  accentColor: string;
  fontScale: FontScale;
  density: Density;
  reducedMotion: boolean;
  sidebarCollapsed: boolean;
};
export type UpdatePreferences = {
  autoCheck: boolean;
  notifications: boolean;
};
export type CreationDefaults = {
  modelId: string;
  aspect: string;
  count: number;
  seed: string;
};
export type PreferencesSnapshot = {
  schemaVersion: number;
  appearance: AppearancePreferences;
  creationDefaults: CreationDefaults;
  updates: UpdatePreferences;
  loadError: string | null;
  saveError: string | null;
  warnings: string[];
};
export type PreferencesEnvelope = {
  preferences: PreferencesSnapshot;
  models: { id: string; displayName: string; family: string; productStatus: string; capabilities: { id: string; label: string }[]; runtime: string }[];
};
export type StorageEntry = {
  id: string;
  label: string;
  path: string;
  bytes: number;
  availableBytes: number | null;
  status: 'ready' | 'missing' | 'partial' | 'error';
  error: string | null;
};
export type RuntimeSummary = {
  phase: string;
  summary: string;
  runtimeReady: boolean;
  modelReady?: boolean;
  deviceVerified: boolean;
  lastError: string | null;
};
export type RuntimeEntry = RuntimeSummary & {
  installId: string;
  label: string;
  /** Registered runtime lock id (Windows reports the v1 runtime, shared v2 runtime or ncnn). */
  runtimeLockId: string | null;
  hardwareEligible: boolean;
  hardwareReason: string | null;
  minVramGb: number | null;
  minRamGb: number | null;
};
export type SettingsSnapshot = {
  preferences: PreferencesSnapshot;
  catalog: CatalogPayload;
  storage: { entries: StorageEntry[]; migrationSupported: false; deletionSupported: false };
  environment: {
    platform: string;
    architecture: string;
    runtimeName: string;
    runtimeVersion: string;
    pythonVersion: string;
    diagnosticsEnabled: boolean;
    runtime: RuntimeSummary;
    runtimes: RuntimeEntry[];
    /** Human-readable local device facts, e.g. "显存 12 GB · 内存 32 GB". */
    deviceFacts: string;
    deviceVramGb: number | null;
    deviceRamGb: number;
  };
  about: {
    version: string;
    productName: string;
    localProcessing: boolean;
    autoUpdateSupported: boolean;
    onlineProviderSupported: boolean;
    storageMigrationSupported: boolean;
    examples: string;
    fontLicense: string;
    modelSources: string[];
    capabilityScope: string;
  };
};

export const defaultAppearance: AppearancePreferences = {
  theme: 'light',
  accentColor: '#D7641F',
  fontScale: 'default',
  density: 'comfortable',
  reducedMotion: false,
  sidebarCollapsed: false,
};

export const defaultUpdatePreferences: UpdatePreferences = {
  autoCheck: true,
  notifications: true,
};

export const builtInCreationDefaults: CreationDefaults = {
  modelId: Z_IMAGE_MODEL_ID,
  aspect: '1:1',
  count: 1,
  seed: '',
};

export const capabilityForTextModel = (modelId: string): string =>
  textRecipeForModel(modelId)?.capabilityId ?? Z_IMAGE_CAPABILITY_FALLBACK;

export const defaultsEqual = (left: CreationDefaults, right: CreationDefaults): boolean =>
  left.modelId === right.modelId
  && left.aspect === right.aspect
  && left.count === right.count
  && left.seed === right.seed;

export function resolveTheme(theme: ThemeMode): 'light' | 'dark' {
  if (theme !== 'system') return theme;
  return window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}
