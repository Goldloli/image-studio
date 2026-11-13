import { describe, expect, it } from 'vitest';
import settingsSource from './SettingsPage.tsx?raw';

describe('settings center layout contract', () => {
  it('keeps the six preference groups in the intended order', () => {
    for (const label of ['通用与外观', '创作默认值', '存储', '运行环境', '更新', '关于']) {
      expect(settingsSource).toContain(label);
    }
    expect(settingsSource.indexOf('通用与外观')).toBeLessThan(settingsSource.indexOf('创作默认值'));
    expect(settingsSource.indexOf('运行环境')).toBeLessThan(settingsSource.indexOf('更新'));
    expect(settingsSource.indexOf('更新')).toBeLessThan(settingsSource.indexOf('关于'));
  });

  it('uses one row surface instead of repeating per-field cards', () => {
    expect(settingsSource).not.toContain('settings-themes');
    expect(settingsSource).not.toContain('theme-miniature');
    expect(settingsSource).not.toContain('appearance-grid');
    expect(settingsSource).toContain('settings-row');
    expect(settingsSource).toContain('settings-section-card');
  });

  it('gates installation on a verified current package', () => {
    expect(settingsSource).toContain('download.status');
    expect(settingsSource).toContain('verifiedCurrent');
    expect(settingsSource).toContain('canInstallUpdate');
    expect(settingsSource).toContain('安装并重启');
  });

  it('keeps the update route non-modal and keyboard labeled', () => {
    expect(settingsSource).toContain('aria-current');
    expect(settingsSource).toContain('aria-label="设置分类"');
    expect(settingsSource).toContain('aria-label="自动检查更新"');
    expect(settingsSource).toContain('aria-label="更新通知"');
  });

  it('keeps GitHub project and author information in About', () => {
    for (const label of ['GitHub 仓库', '最新发布', 'Issues 与反馈', '开源许可证', '第三方声明', 'Goldloli 小卡塔克', 'Bilibili']) {
      expect(settingsSource).toContain(label);
    }
    expect(settingsSource).toContain("api('about_open_link'");
  });
});
