import { describe, expect, it } from 'vitest';
import {
  AUTO_UPDATE_INTERVAL_MS,
  autoCheckDue,
  canDownloadUpdate,
  canInstallUpdate,
  canOpenVerifiedInstaller,
  downloadPercent,
  formatBytesPerSecond,
  formatEta,
  type UpdateCheckResult,
  type UpdateDownloadState,
} from './app-update';

const checked: UpdateCheckResult = {
  currentVersion: '0.2.2',
  updateAvailable: true,
  checkedAtMs: 1,
  release: {
    version: '0.3.0',
    tagName: 'v0.3.0',
    name: 'Image Studio 0.3.0',
    notes: '更新说明',
    publishedAt: null,
    htmlUrl: 'https://github.com/Goldloli/image-studio/releases/tag/v0.3.0',
    assetName: 'Image-Studio-0.3.0-macos-arm64.dmg',
    assetSize: 1000,
  },
  releasesUrl: 'https://github.com/Goldloli/image-studio/releases',
};

function download(patch: Partial<UpdateDownloadState> = {}): UpdateDownloadState {
  return {
    status: 'idle',
    version: null,
    filename: null,
    path: null,
    downloadedBytes: 0,
    totalBytes: null,
    progress: null,
    bytesPerSecond: null,
    etaSeconds: null,
    error: null,
    ...patch,
  };
}

describe('application update state', () => {
  it('allows one automatic check only after the 24 hour window', () => {
    const now = 2_000_000;
    expect(autoCheckDue(now, null)).toBe(true);
    expect(autoCheckDue(now, now - AUTO_UPDATE_INTERVAL_MS + 1)).toBe(false);
    expect(autoCheckDue(now, now - AUTO_UPDATE_INTERVAL_MS)).toBe(true);
  });

  it('never exposes installation actions for failed or unverified downloads', () => {
    expect(canInstallUpdate(download({ status: 'error', error: '下载失败' }), 'nsis')).toBe(false);
    expect(canInstallUpdate(download({ status: 'verifying' }), 'nsis')).toBe(false);
    expect(canInstallUpdate(download({ status: 'completed', path: '/tmp/setup.exe' }), 'dmg')).toBe(false);
    expect(canInstallUpdate(download({ status: 'completed', path: '/tmp/setup.exe' }), 'nsis')).toBe(true);
    expect(canInstallUpdate(download({ status: 'completed', version: '0.2.9', path: '/tmp/setup.exe' }), 'nsis', '0.3.0')).toBe(false);
    expect(canInstallUpdate(download({ status: 'completed', version: 'v0.3.0', path: '/tmp/setup.exe' }), 'nsis', '0.3.0')).toBe(true);
    expect(canOpenVerifiedInstaller(download({ status: 'completed', path: '/tmp/update.dmg' }), 'dmg')).toBe(true);
    expect(canOpenVerifiedInstaller(download({ status: 'error', error: '下载失败', path: '/tmp/update.dmg' }), 'dmg')).toBe(false);
  });

  it('does not fake a progress percentage when total bytes are unknown', () => {
    expect(downloadPercent(download({ status: 'downloading', downloadedBytes: 50, totalBytes: null }))).toBeNull();
    expect(downloadPercent(download({ status: 'downloading', downloadedBytes: 50, totalBytes: 100 }))).toBe(50);
    expect(downloadPercent(download({ status: 'completed', downloadedBytes: 100, totalBytes: 100 }))).toBeNull();
  });

  it('allows retrying an old or failed version but not the completed current version', () => {
    expect(canDownloadUpdate(checked, download({ status: 'error', version: '0.3.0' }))).toBe(true);
    expect(canDownloadUpdate(checked, download({ status: 'completed', version: '0.2.9', path: '/tmp/old.dmg' }))).toBe(true);
    expect(canDownloadUpdate(checked, download({ status: 'completed', version: '0.3.0', path: '/tmp/current.dmg' }))).toBe(false);
    expect(canDownloadUpdate({ ...checked, release: null }, download())).toBe(false);
  });

  it('formats transfer estimates without inventing missing values', () => {
    expect(formatBytesPerSecond(null)).toBe('—');
    expect(formatBytesPerSecond(1_500_000)).toBe('1.5 MB/s');
    expect(formatEta(null)).toBe('—');
    expect(formatEta(80)).toBe('2 分钟');
  });
});
