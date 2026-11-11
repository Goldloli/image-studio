import { describe, expect, it } from 'vitest';
import {
  assetCountLabel,
  assetLibraryRequestKey,
  buildAssetQuery,
  hasAssetFilters,
  localDateEnd,
  localDateStart,
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
