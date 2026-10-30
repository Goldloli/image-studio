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
  installPhase: string;
  componentsReady: boolean;
  runtimeReady: boolean;
  deviceVerified: boolean;
  executable: boolean;
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
};
export type CatalogPayload = {
  schemaVersion: number;
  reviewedDate: string;
  entries: ModelEntry[];
  counts: CatalogCounts;
};
export type PrimaryFilter = 'all' | 'installed' | 'executable' | 'planned' | 'runtime';
export type CapabilityFilter = 'all' | 'text_to_image' | 'image_edit' | 'background_removal' | 'upscale';

export const primaryFilters: { id: PrimaryFilter; label: string }[] = [
  { id: 'all', label: '全部' },
  { id: 'installed', label: '已安装' },
  { id: 'executable', label: '可执行' },
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
