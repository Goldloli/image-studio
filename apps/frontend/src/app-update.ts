export const UPDATE_PROGRESS_EVENT = 'app-update-progress';
export const AUTO_UPDATE_DELAY_MS = 10_000;
export const AUTO_UPDATE_INTERVAL_MS = 24 * 60 * 60 * 1000;
export const AUTO_UPDATE_CHECKED_AT_KEY = 'image-studio:update-last-auto-check';

export type InstallerKind = 'nsis' | 'dmg' | null;
export type AppVersionInfo = {
  version: string;
  target: string;
  installerKind: InstallerKind;
};
export type UpdateRelease = {
  version: string;
  tagName: string;
  name: string;
  notes: string;
  publishedAt: string | null;
  htmlUrl: string;
  assetName: string | null;
  assetSize: number | null;
};
export type UpdateCheckResult = {
  currentVersion: string;
  updateAvailable: boolean;
  checkedAtMs: number;
  release: UpdateRelease | null;
  releasesUrl: string;
};
export type UpdateDownloadStatus =
  | 'idle'
  | 'preparing'
  | 'downloading'
  | 'verifying'
  | 'cancelling'
  | 'cancelled'
  | 'completed'
  | 'error';
export type UpdateDownloadState = {
  status: UpdateDownloadStatus;
  version: string | null;
  filename: string | null;
  path: string | null;
  downloadedBytes: number;
  totalBytes: number | null;
  progress: number | null;
  bytesPerSecond: number | null;
  etaSeconds: number | null;
  error: string | null;
};

export const idleUpdateDownload: UpdateDownloadState = {
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
};

export function autoCheckDue(now: number, lastCheckedAt: number | null): boolean {
  return lastCheckedAt === null || now - lastCheckedAt >= AUTO_UPDATE_INTERVAL_MS;
}

export function readLastAutoCheck(storage: Pick<Storage, 'getItem'>): number | null {
  try {
    const raw = storage.getItem(AUTO_UPDATE_CHECKED_AT_KEY);
    if (!raw) return null;
    const value = Number(raw);
    return Number.isFinite(value) && value > 0 ? value : null;
  } catch {
    return null;
  }
}

export function markAutoCheckAttempt(storage: Pick<Storage, 'setItem'>, checkedAt: number) {
  try {
    storage.setItem(AUTO_UPDATE_CHECKED_AT_KEY, String(checkedAt));
  } catch {
    // A denied WebView storage does not prevent the in-memory check attempt.
  }
}

export function isDownloadActive(status: UpdateDownloadStatus): boolean {
  return ['preparing', 'downloading', 'verifying', 'cancelling'].includes(status);
}

export function canDownloadUpdate(
  check: UpdateCheckResult | null,
  download: UpdateDownloadState,
): boolean {
  if (!check?.updateAvailable || !check.release?.assetName || !check.release.assetSize) return false;
  if (isDownloadActive(download.status)) return false;
  const sameVersion = normalizeVersion(download.version) === normalizeVersion(check.release.version);
  return !(sameVersion && download.status === 'completed');
}

export function canInstallUpdate(
  download: UpdateDownloadState,
  installerKind: InstallerKind,
  expectedVersion?: string | null,
): boolean {
  return installerKind === 'nsis'
    && download.status === 'completed'
    && !!download.path
    && !download.error
    && matchesExpectedVersion(download.version, expectedVersion);
}

export function canOpenVerifiedInstaller(
  download: UpdateDownloadState,
  installerKind: InstallerKind,
  expectedVersion?: string | null,
): boolean {
  return installerKind === 'dmg'
    && download.status === 'completed'
    && !!download.path
    && !download.error
    && matchesExpectedVersion(download.version, expectedVersion);
}

export function downloadPercent(download: UpdateDownloadState): number | null {
  if (download.status !== 'downloading' && download.status !== 'verifying' && download.status !== 'cancelling') {
    return null;
  }
  if (!download.totalBytes || download.totalBytes <= 0) return null;
  return Math.max(0, Math.min(100, (download.downloadedBytes / download.totalBytes) * 100));
}

export function formatBytesPerSecond(value: number | null): string {
  if (value === null || !Number.isFinite(value) || value <= 0) return '—';
  if (value >= 1e9) return `${(value / 1e9).toFixed(1)} GB/s`;
  if (value >= 1e6) return `${(value / 1e6).toFixed(1)} MB/s`;
  return `${Math.max(1, Math.round(value / 1e3))} KB/s`;
}

export function formatEta(value: number | null): string {
  if (value === null || !Number.isFinite(value) || value < 0) return '—';
  if (value < 60) return `${Math.round(value)} 秒`;
  const minutes = Math.ceil(value / 60);
  return `${minutes} 分钟`;
}

export function formatUpdateTime(value: number | string | null): string {
  if (value === null) return '—';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '—';
  return new Intl.DateTimeFormat('zh-CN', {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date);
}

function normalizeVersion(value: string | null): string {
  return (value ?? '').trim().replace(/^v/i, '');
}

function matchesExpectedVersion(actual: string | null, expected?: string | null): boolean {
  return expected === undefined || normalizeVersion(actual) === normalizeVersion(expected);
}
