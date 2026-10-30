import { describe, expect, it } from 'vitest';
import {
  assetCountLabel,
  assetLibraryRequestKey,
  buildAssetQuery,
  hasAssetFilters,
  localDateEnd,
  localDateStart,
  taskItemToAsset,
} from './library-state';
import type { TaskCenterItem } from './studio-api';

const filters = { modelId: '', conversationId: '', size: '', from: '', to: '' };

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
      cursor: null,
      limit: 24,
    });
    expect(hasAssetFilters(filters)).toBe(false);
  });

  it('maps combined filters to inclusive local date bounds', () => {
    const query = buildAssetQuery(
      { modelId: 'z-image-turbo-mflux-v1', conversationId: 'conv-a', size: '1024x768', from: '2026-10-01', to: '2026-10-30' },
      { createdAt: 12, id: 'asset-a' },
      10,
    );
    expect(query.width).toBe(1024);
    expect(query.height).toBe(768);
    expect(query.createdFrom).toBe(localDateStart('2026-10-01'));
    expect(query.createdTo).toBe(localDateEnd('2026-10-30'));
    expect(query.cursor).toEqual({ createdAt: 12, id: 'asset-a' });
    expect(hasAssetFilters({ modelId: 'model', conversationId: '', size: '', from: '', to: '' })).toBe(true);
  });

  it('only converts a task into a viewer asset when a formal asset id exists', () => {
    const task: TaskCenterItem = {
      id: 'job-a',
      turnId: 'turn-a',
      turnSequence: 1,
      conversationId: 'conv-a',
      conversationTitle: '会话 A',
      draftId: 2,
      attemptId: 'attempt-a',
      state: 'Succeeded',
      prompt: '提示词',
      modelId: 'z-image-turbo-mflux-v1',
      width: 1024,
      height: 1024,
      seed: 42,
      createdAt: 1,
      completedAt: 2,
      error: null,
      assetId: null,
      assetAvailable: false,
      assetError: null,
    };
    expect(taskItemToAsset(task)).toBeNull();
    expect(taskItemToAsset({ ...task, assetId: 'asset-a', assetAvailable: true })?.id).toBe('asset-a');
  });
});
