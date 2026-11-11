import { describe, expect, it } from 'vitest';
import { dimensionText, errorCategoryLabel, manifestText } from './production-record';

describe('production record formatting', () => {
  it('labels every known failure category and preserves unknown ones', () => {
    expect(errorCategoryLabel('validation')).toBe('输入或参数');
    expect(errorCategoryLabel('memory')).toBe('内存');
    expect(errorCategoryLabel('runtime')).toBe('运行环境');
    expect(errorCategoryLabel('cancel')).toBe('取消');
    expect(errorCategoryLabel('interrupt')).toBe('宿主中断');
    expect(errorCategoryLabel('storage')).toBe('存储');
    expect(errorCategoryLabel('other')).toBe('其他执行');
    expect(errorCategoryLabel('')).toBe('');
    expect(errorCategoryLabel(null)).toBe('');
    expect(errorCategoryLabel('custom')).toBe('custom');
  });

  it('only shows the actual size when it differs from the requested size', () => {
    expect(dimensionText(1024, 1024, 1024, 1024)).toBe('1024×1024 请求');
    expect(dimensionText(1024, 1024, 768, 768)).toBe('1024×1024 请求 · 768×768 实际');
    expect(dimensionText(1024, 768, 1024, 768)).toBe('1024×768 请求');
  });

  it('renders manifests as readable JSON without inventing missing records', () => {
    expect(manifestText(null)).toBe('尚无完整 manifest');
    expect(manifestText(undefined)).toBe('尚无完整 manifest');
    expect(manifestText({ run: 'x', steps: 9 })).toBe('{\n  "run": "x",\n  "steps": 9\n}');
    const circular: Record<string, unknown> = {};
    circular.self = circular;
    expect(manifestText(circular)).toBe('制作记录无法序列化');
  });
});
