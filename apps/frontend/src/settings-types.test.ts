import { describe, expect, it } from 'vitest';
import { commonAvailableBytes } from './settings-types';

describe('settings storage availability', () => {
  it('returns no shared value when no location reports free space', () => {
    expect(commonAvailableBytes([])).toBeNull();
    expect(commonAvailableBytes([{ availableBytes: null }])).toBeNull();
  });

  it('returns one shared value when every known location is on the same volume', () => {
    expect(commonAvailableBytes([
      { availableBytes: 4_000 },
      { availableBytes: null },
      { availableBytes: 4_000 },
    ])).toBe(4_000);
  });

  it('falls back to row-level values when known locations differ', () => {
    expect(commonAvailableBytes([
      { availableBytes: 4_000 },
      { availableBytes: 2_000 },
      { availableBytes: null },
    ])).toBeNull();
  });
});
