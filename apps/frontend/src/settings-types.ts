import type { CatalogPayload } from './model-catalog';

export type ThemeMode = 'light' | 'dark' | 'system';
export type FontScale = 'small' | 'default' | 'large';
export type Density = 'comfortable' | 'compact';
export type AppearancePreferences = {
  theme: ThemeMode;
  fontScale: FontScale;
  density: Density;
  reducedMotion: boolean;
  sidebarCollapsed: boolean;
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
  fontScale: 'default',
  density: 'comfortable',
  reducedMotion: false,
  sidebarCollapsed: false,
};

export const builtInCreationDefaults: CreationDefaults = {
  modelId: 'z-image-turbo-mflux-v1',
  aspect: '1:1',
  count: 1,
  seed: '',
};

export const defaultsEqual = (left: CreationDefaults, right: CreationDefaults): boolean =>
  left.modelId === right.modelId
  && left.aspect === right.aspect
  && left.count === right.count
  && left.seed === right.seed;

export function resolveTheme(theme: ThemeMode): 'light' | 'dark' {
  if (theme !== 'system') return theme;
  return window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}
