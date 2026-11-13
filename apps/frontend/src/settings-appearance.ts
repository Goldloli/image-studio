export const ACCENT_PRESETS = [
  { id: 'brand', label: '品牌橙', value: '#D7641F' },
  { id: 'coral', label: '珊瑚', value: '#D3575F' },
  { id: 'rose', label: '玫红', value: '#C94A8D' },
  { id: 'blue', label: '蓝', value: '#3974B8' },
  { id: 'violet', label: '紫', value: '#7357A8' },
  { id: 'graphite', label: '石墨', value: '#6B7280' },
] as const;

export function normalizeAccentColor(value: string): string | null {
  const match = /^#([0-9a-fA-F]{6})$/.exec(value.trim());
  return match ? `#${match[1].toUpperCase()}` : null;
}

export function applyAccentColor(value: string): string | null {
  const normalized = normalizeAccentColor(value);
  if (!normalized) return null;
  document.documentElement.style.setProperty('--accent-base', normalized);
  return normalized;
}
