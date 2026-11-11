import { describe, expect, it } from 'vitest';
import {
  assetCountLabel,
  assetLibraryRequestKey,
  buildAssetQuery,
  hasAssetFilters,
  localDateEnd,
  localDateStart,
  referenceConflictsMessage,
  workDeleteTitle,
} from './library-state';

const filters = { modelId: '', conversationId: '', size: '', from: '', to: '', favorite: 'all' as const };

describe('library query state', () => {
  it('creates a fresh request key for each active page activation and invalidates inactive views', () => {
    const filterKey = JSON.stringify({ modelId: '', conversationId: '', size: '', from: '', to: '' });
    expect(assetLibraryRequestKey(false, filterKey, 0)).toBeNull();
    const first = assetLibraryRequestKey(true, filterKey, 0);
    expect(first).not.toBeNull();
    expect(assetLibraryRequestKey(false, filterKey, 0)).toBeNull();
    expect(assetLibraryRequestKey(true, filterKey, 0)).toBe(first);
    expect(assetLibraryRequestKey(true, filterKey, 1)).not.toBe(first);
    expect(assetLibraryRequestKey(true, JSON.stringify({ modelId: 'z-image-turbo-mflux-v1' }), 0)).not.toBe(first);
  });

  it('does not present an old total as current while an active page is refreshing', () => {
    expect(assetCountLabel(true, true, 3)).toBe('正在读取作品');
    expect(assetCountLabel(true, false, 3)).toBe('3 件正式作品');
    expect(assetCountLabel(true, false, 3, true)).toBe('作品计数不可用');
    expect(assetCountLabel(false, false, 3)).toBe('读取作品');
    expect(assetCountLabel(true, false, null)).toBe('读取作品');
  });

  it('builds an empty formal asset query without invented dates or dimensions', () => {
    expect(buildAssetQuery(filters)).toEqual({
      modelId: null,
      conversationId: null,
      width: null,
      height: null,
      createdFrom: null,
      createdTo: null,
      favoritesOnly: false,
      cursor: null,
      limit: 24,
    });
    expect(hasAssetFilters(filters)).toBe(false);
  });

  it('maps combined filters to inclusive local date bounds', () => {
    const query = buildAssetQuery(
      { modelId: 'z-image-turbo-mflux-v1', conversationId: 'conv-a', size: '1024x768', from: '2026-10-01', to: '2026-10-30', favorite: 'all' as const },
      { createdAt: 12, id: 'asset-a' },
      10,
    );
    expect(query.width).toBe(1024);
    expect(query.height).toBe(768);
    expect(query.createdFrom).toBe(localDateStart('2026-10-01'));
    expect(query.createdTo).toBe(localDateEnd('2026-10-30'));
    expect(query.cursor).toEqual({ createdAt: 12, id: 'asset-a' });
    expect(hasAssetFilters({ modelId: 'model', conversationId: '', size: '', from: '', to: '', favorite: 'all' })).toBe(true);
  });

});

describe('favourite filtering', () => {
  const base = { modelId: '', conversationId: '', size: '', from: '', to: '', favorite: 'all' as const };
  it('defaults to all works and only adds the host filter when favourites are selected', () => {
    expect(buildAssetQuery(base).favoritesOnly).toBe(false);
    expect(buildAssetQuery({ ...base, favorite: 'favorites' }).favoritesOnly).toBe(true);
    expect(hasAssetFilters(base)).toBe(false);
    expect(hasAssetFilters({ ...base, favorite: 'favorites' })).toBe(true);
  });
  it('combines favourites with the other filters instead of replacing them', () => {
    const query = buildAssetQuery({ ...base, favorite: 'favorites', modelId: 'z-image-turbo-mflux-v1', size: '1024x768' });
    expect(query.favoritesOnly).toBe(true);
    expect(query.modelId).toBe('z-image-turbo-mflux-v1');
    expect(query.width).toBe(1024);
    expect(query.height).toBe(768);
  });
});

describe('work delete confirmation title', () => {
  it('uses a readable prompt and truncates long ones', () => {
    expect(workDeleteTitle({ prompt: '一只橘猫', conversationTitle: '会话' })).toBe('一只橘猫');
    const long = '一'.repeat(30);
    expect(workDeleteTitle({ prompt: long, conversationTitle: '会话' })).toBe(`${'一'.repeat(24)}…`);
  });
  it('falls back to the source conversation for empty prompts', () => {
    expect(workDeleteTitle({ prompt: '', conversationTitle: '去背景' })).toBe('去背景');
    expect(workDeleteTitle({ prompt: '   ', conversationTitle: '  去背景  ' })).toBe('去背景');
  });
  it('uses a generic label when both are empty', () => {
    expect(workDeleteTitle({ prompt: '', conversationTitle: '' })).toBe('该作品');
  });
});

describe('reference conflict guidance', () => {
  const base = { conversationId: 'c1', conversationTitle: '会话 A', capabilityId: null };
  it('renders each kind with its own actionable detail', () => {
    const message = referenceConflictsMessage([
      { ...base, kind: 'draft', detail: '请在对应会话中移除源图或更换源图后重试' },
      { ...base, conversationId: 'c2', conversationTitle: '会话 B', kind: 'active', detail: '请等待该请求完成或取消后再试' },
    ]);
    expect(message).toContain('请先解除后重试');
    expect(message).toContain('「会话 A」请在对应会话中移除源图或更换源图后重试');
    expect(message).toContain('「会话 B」请等待该请求完成或取消后再试');
  });
  it('labels retained slots with the capability display name and falls back for unknown ids', () => {
    const known = referenceConflictsMessage([{
      ...base, kind: 'retained', capabilityId: 'image.text-to-image.z-image-turbo.v1',
      detail: '请在对应会话切回该能力后移除源图，或更换源图后重试',
    }]);
    expect(known).toContain('对应能力：Z-Image Turbo');
    const unknown = referenceConflictsMessage([{
      ...base, kind: 'retained', capabilityId: 'image.future.unknown.v1',
      detail: '请在对应会话切回该能力后移除源图，或更换源图后重试',
    }]);
    expect(unknown).toContain('对应能力：image.future.unknown.v1');
  });
  it('answers with the guidance header for an empty list instead of crashing', () => {
    expect(referenceConflictsMessage([])).toContain('请先解除后重试');
  });
});
