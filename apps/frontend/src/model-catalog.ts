export type ModelCapability = { id: string; label: string };
export type ModelProductStatus = 'implemented' | 'planned';
export type ModelEntry = {
  id: string;
  displayName: string;
  variant: string;
  family: string;
  summary: string;
  description: string;
  capabilities: ModelCapability[];
  productStatus: ModelProductStatus;
  runtime: string;
  license: string;
  licenseNotice: string | null;
  source: string;
  revision: string;
  modelBytes: number | null;
  knownArtifactBytes: number | null;
  requirements: string[];
  exclusions: string[];
  verifiedDevices: string[];
  recipe: string;
  installId: string | null;
  installerSupported: boolean;
  /** Hardware thresholds registered by the host catalog (design.md section 10.2). */
  minVramGb: number | null;
  minRamGb: number | null;
  /** The standard-tier VRAM threshold when the entry registers execution tiers. */
  standardMinVramGb: number | null;
  /** Execution tier the current device will use, or null for single-threshold entries. */
  tier: 'standard' | 'compact' | null;
  /** Output edge in pixels for the selected tier (1024 standard, 768/512 compact). */
  resolution: number | null;
  computeDtype: string | null;
  /** Why the compact tier was selected, e.g. "显存 8 GB 低于标准档 12 GB 门槛…". */
  tierReason: string | null;
  tiers: { id: string; minVramGb: number; minRamGb: number; resolution: number; computeDtype: string }[];
  quantization: string | null;
  windowsStatus: string | null;
  installPhase: string;
  componentsReady: boolean;
  runtimeReady: boolean;
  deviceVerified: boolean;
  executable: boolean;
  /** Whether the local device meets (or can prove) the registered thresholds. */
  hardwareEligible: boolean;
  /** Actionable Chinese explanation when `hardwareEligible` is false. */
  hardwareReason: string | null;
  busy: boolean;
  receivedBytes: number;
  installTotalBytes: number;
  runtimeBytes: number;
  requiredBytes: number;
  error: string | null;
  canInstall: boolean;
  canContinue: boolean;
  canStop: boolean;
  canUse: boolean;
};
export type CatalogCounts = {
  total: number;
  supported: number;
  installed: number;
  executable: number;
  planned: number;
  runtimeReady: number;
  hardwareEligible: number;
};
export type CatalogPayload = {
  schemaVersion: number;
  reviewedDate: string;
  entries: ModelEntry[];
  counts: CatalogCounts;
};
export type PrimaryFilter = 'all' | 'installed' | 'executable' | 'planned' | 'runtime' | 'eligible';
export type CapabilityFilter = 'all' | 'text_to_image' | 'image_edit' | 'background_removal' | 'upscale';

export const primaryFilters: { id: PrimaryFilter; label: string }[] = [
  { id: 'all', label: '全部' },
  { id: 'installed', label: '已安装' },
  { id: 'executable', label: '可执行' },
  { id: 'eligible', label: '设备满足' },
  { id: 'planned', label: '计划中' },
  { id: 'runtime', label: '运行环境' },
];

export const capabilityFilters: { id: CapabilityFilter; label: string }[] = [
  { id: 'all', label: '全部能力' },
  { id: 'text_to_image', label: '文生图' },
  { id: 'image_edit', label: '图片编辑' },
  { id: 'background_removal', label: '去背景' },
  { id: 'upscale', label: '放大' },
];

export function filterCatalog(
  entries: ModelEntry[],
  query: string,
  primary: PrimaryFilter,
  capability: CapabilityFilter,
): ModelEntry[] {
  const needle = query.trim().toLocaleLowerCase();
  return entries.filter(entry => {
    const matchesQuery = !needle || [
      entry.id,
      entry.displayName,
      entry.variant,
      entry.family,
      entry.runtime,
    ].some(value => value.toLocaleLowerCase().includes(needle));
    const matchesPrimary = primary === 'all'
      || primary === 'installed' && entry.componentsReady
      || primary === 'executable' && entry.executable
      || primary === 'eligible' && entry.hardwareEligible
      || primary === 'planned' && entry.productStatus === 'planned'
      || primary === 'runtime' && entry.runtimeReady;
    const matchesCapability = capability === 'all'
      || entry.capabilities.some(item => item.id === capability);
    return matchesQuery && matchesPrimary && matchesCapability;
  });
}

export function installProgress(entry: ModelEntry): { value: number; max: number; label: string } | null {
  if (!entry.busy) return null;
  const known = entry.installTotalBytes > 0;
  return {
    value: known ? Math.min(entry.receivedBytes, entry.installTotalBytes) : 0,
    max: known ? entry.installTotalBytes : 1,
    label: known ? '可确定的下载字节' : '当前阶段无法换算百分比',
  };
}

/** True when the registered hardware gate disables install/generation entry points. */
export const hardwareBlocked = (entry: ModelEntry): boolean => !entry.hardwareEligible;

/**
 * Effective entry-point permission. The host already gates `canInstall`/`canUse` with the device
 * facts, but the UI re-checks `hardwareEligible` so a stale catalog payload can never re-enable a
 * model the device cannot run.
 */
export const canStartGeneration = (entry: ModelEntry): boolean =>
  entry.canUse && entry.executable && entry.hardwareEligible;

export const canInstallModel = (entry: ModelEntry): boolean =>
  entry.canInstall && entry.hardwareEligible;

/** Human-readable threshold summary, e.g. "需要显存 ≥ 12 GB · 内存 ≥ 32 GB". */
export function hardwareThresholdLabel(entry: ModelEntry): string {
  const parts: string[] = [];
  if (entry.minVramGb !== null && entry.minVramGb > 0) {
    const standard = entry.standardMinVramGb;
    parts.push(standard !== null && standard > entry.minVramGb
      ? `显存 ≥ ${entry.minVramGb} GB（紧凑档）/ ${standard} GB（标准档）`
      : `显存 ≥ ${entry.minVramGb} GB`);
  }
  if (entry.minRamGb !== null && entry.minRamGb > 0) parts.push(`内存 ≥ ${entry.minRamGb} GB`);
  if (entry.minVramGb === 0) parts.push('CPU 路线（无显存门槛）');
  return parts.length ? `需要${parts.join(' · ')}` : '未登记硬件门槛';
}

/** The tier the device will execute, e.g. "紧凑档 768×768 · float16"; null without tiers. */
export function executionTierLabel(entry: ModelEntry): string | null {
  if (entry.tier === null) return null;
  const name = entry.tier === 'compact' ? '紧凑档' : '标准档';
  const size = entry.resolution === null ? '' : ` ${entry.resolution}×${entry.resolution}`;
  const dtype = entry.computeDtype === null ? '' : ` · ${entry.computeDtype}`;
  return `${name}${size}${dtype}`;
}

/** The license text shown on the model page; the full notice wins over the short license id. */
export const licenseLabel = (entry: ModelEntry): string => entry.licenseNotice ?? entry.license;

export const isNonCommercialLicense = (entry: ModelEntry): boolean =>
  /非商业|non-commercial|research/i.test(licenseLabel(entry));
