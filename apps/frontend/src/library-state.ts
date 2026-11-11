import type { AssetLibraryItem, AssetLibraryQuery, AssetReferenceConflict } from './studio-api';
import type { AssetView } from './studio-api';
import { capabilityDisplayName, modelDisplayName } from './model-recipes';

export function localDateStart(value: string): number | null {
  if (!value) return null;
  const date = new Date(`${value}T00:00:00`);
  return Number.isFinite(date.getTime()) ? date.getTime() : null;
}

export function localDateEnd(value: string): number | null {
  if (!value) return null;
  const date = new Date(`${value}T23:59:59.999`);
  return Number.isFinite(date.getTime()) ? date.getTime() : null;
}

export type AssetLibraryFilters = {
  modelId: string;
  conversationId: string;
  size: string;
  from: string;
  to: string;
  /** "all" (default) or "favorites"; always combined with the other filters. */
  favorite: 'all' | 'favorites';
};

export function buildAssetQuery(
  filters: AssetLibraryFilters,
  cursor: import('./studio-api').StableCursor | null = null,
  limit = 24,
): AssetLibraryQuery {
  const [width, height] = filters.size
    .split('x')
    .map(value => Number(value));
  return {
    modelId: filters.modelId || null,
    conversationId: filters.conversationId || null,
    width: Number.isInteger(width) && width > 0 ? width : null,
    height: Number.isInteger(height) && height > 0 ? height : null,
    createdFrom: localDateStart(filters.from),
    createdTo: localDateEnd(filters.to),
    favoritesOnly: filters.favorite === 'favorites',
    cursor,
    limit,
  };
}

export function hasAssetFilters(filters: AssetLibraryFilters): boolean {
  return Boolean(filters.modelId || filters.conversationId || filters.size || filters.from || filters.to || filters.favorite === 'favorites');
}

export function assetLibraryRequestKey(active: boolean, filterKey: string, refreshNonce: number): string | null {
  return active ? `${refreshNonce}\u0000${filterKey}` : null;
}

export function assetCountLabel(active: boolean, loading: boolean, total: number | null, unavailable = false): string {
  if (!active || total === null) return '读取作品';
  if (loading) return '正在读取作品';
  if (unavailable) return '作品计数不可用';
  return `${total} 件正式作品`;
}

export function assetItemToView(item: AssetLibraryItem): AssetView {
  return {
    isFavorite: item.isFavorite,
    deletionState: item.deletionState,
    deletionError: item.deletionError,
    id: item.id,
    jobId: item.jobId,
    conversationId: item.conversationId,
    turnId: item.turnId,
    prompt: item.prompt,
    width: item.width,
    height: item.height,
    seed: item.seed,
    bytes: item.byteCount,
    available: item.available,
    error: item.availabilityError,
  };
}

/** Actionable message for a refused deletion; each entry explains its own resolution. */
export function referenceConflictsMessage(conflicts: AssetReferenceConflict[]): string {
  const entries = conflicts.map(conflict => {
    const capability = conflict.kind === 'retained' && conflict.capabilityId
      ? `（对应能力：${capabilityDisplayName(conflict.capabilityId)}）`
      : '';
    return `「${conflict.conversationTitle}」${conflict.detail}${capability}`;
  });
  return `仍被以下位置用作源图，请先解除后重试：${entries.join('；')}`;
}

/** Human-identifiable target for the destructive confirmation; empty prompts fall back. */
export function workDeleteTitle(item: Pick<AssetLibraryItem, 'prompt' | 'conversationTitle'>): string {
  const prompt = item.prompt.trim();
  if (prompt) return prompt.length > 24 ? `${prompt.slice(0, 24)}…` : prompt;
  const conversation = item.conversationTitle.trim();
  return conversation || '该作品';
}

export function formatTime(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return '时间未记录';
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value));
}

export function modelLabel(id: string): string {
  return modelDisplayName(id);
}
