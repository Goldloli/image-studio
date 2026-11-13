import { describe, expect, it } from 'vitest';

const sources = Object.values(import.meta.glob<string>([
  './Workspace.tsx',
  './PromptLibrary.tsx',
  './ModelLibrary.tsx',
  './AssetLibrary.tsx',
  './SettingsPage.tsx',
], { query: '?raw', import: 'default', eager: true }));

describe('layout-first UI copy', () => {
  it('does not restore decorative page labels or repeated status copy', () => {
    const combined = sources.join('\n');
    for (const removed of [
      'PROMPT INSPIRATION',
      '正式 AssetRef · 跨会话',
      '本地图片创作',
      '本地模型库',
      '真实生效并可跨重启恢复的应用设置',
      '应用配置',
      '当前表单与已保存默认值一致',
      '外观偏好已保存到本机',
    ]) {
      expect(combined).not.toContain(removed);
    }
  });

  it('keeps technical evidence behind explicit disclosures', () => {
    const combined = sources.join('\n');
    for (const disclosure of [
      'prompt-catalog-details',
      'prompt-model-evidence',
      'catalog-evidence',
      'works-preview-technical',
      'about-evidence',
    ]) {
      expect(combined).toContain(disclosure);
    }
  });
});
