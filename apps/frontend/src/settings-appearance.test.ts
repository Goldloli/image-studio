import { afterEach, describe, expect, it, vi } from 'vitest';
import { ACCENT_PRESETS, applyAccentColor, normalizeAccentColor } from './settings-appearance';

describe('settings appearance accents', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('normalizes valid hex colors', () => {
    expect(normalizeAccentColor('#d7641f')).toBe('#D7641F');
    expect(normalizeAccentColor(' #3974b8 ')).toBe('#3974B8');
  });

  it('rejects malformed or shorthand colors', () => {
    for (const value of ['#fff', 'd7641f', '#GGGGGG', '#1234567']) {
      expect(normalizeAccentColor(value)).toBeNull();
    }
  });

  it('applies only normalized colors to the root accent token', () => {
    let token = '';
    vi.stubGlobal('document', {
      documentElement: {
        style: {
          setProperty: (name: string, value: string) => {
            if (name === '--accent-base') token = value;
          },
          getPropertyValue: (name: string) => name === '--accent-base' ? token : '',
          removeProperty: (name: string) => {
            if (name === '--accent-base') token = '';
          },
        },
      },
    });
    document.documentElement.style.removeProperty('--accent-base');
    expect(applyAccentColor('nope')).toBeNull();
    expect(document.documentElement.style.getPropertyValue('--accent-base')).toBe('');
    expect(applyAccentColor('#3b82f6')).toBe('#3B82F6');
    expect(document.documentElement.style.getPropertyValue('--accent-base')).toBe('#3B82F6');
    document.documentElement.style.removeProperty('--accent-base');
  });

  it('does not include a green preset or duplicate colors', () => {
    const ids: readonly string[] = ACCENT_PRESETS.map(item => item.id);
    const values: readonly string[] = ACCENT_PRESETS.map(item => item.value);
    expect(ids.includes('green')).toBe(false);
    expect(values.includes('#10B981')).toBe(false);
    expect(new Set(ACCENT_PRESETS.map(item => item.value)).size).toBe(ACCENT_PRESETS.length);
  });
});
