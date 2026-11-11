import { describe, expect, it } from 'vitest';
import {
  emptyPromptQuery,
  filterPromptCases,
  missingFavoriteIds,
  promptLicenseLinks,
  promptUsageLabel,
  PROMPT_LIBRARY_UPSTREAM_COMMIT,
  shortCommit,
  toggleSelection,
  type PromptCase,
  type PromptCatalog,
} from './prompt-library-state';

const makeCase = (id: number, overrides: Partial<PromptCase> = {}): PromptCase => ({
  id: `awesome-gpt-image-2:case:${id}`,
  sourceCaseId: id,
  title: `案例 ${id}`,
  prompt: `Full prompt ${id}`,
  imageAlt: `示例 ${id}`,
  featured: false,
  category: { id: 'poster', label: '海报与排版', sourceLabel: 'Posters & Typography' },
  styles: [{ sourceLabel: 'Poster', label: '海报' }],
  scenes: [{ sourceLabel: 'Commerce', label: '商业' }],
  image: {
    path: `data/images/case${id}.jpg`, filename: `case${id}.jpg`,
    sha256: 'a'.repeat(64), bytes: 10, mimeType: 'image/jpeg', width: 100, height: 100,
  },
  source: { label: '@artist', url: null, githubUrl: `https://example.test/${id}` },
  usage: { inputKind: 'text-to-image', hasVariables: false, notes: [], reviewStatus: 'reviewed' },
  model: { upstreamProject: 'awesome-gpt-image-2', verification: 'unverified', exactModelId: null },
  rights: { commercialUse: 'unverified', redistribution: 'unverified', sourceReferenceRequired: true },
  ...overrides,
});

const one = makeCase(1);
const two = makeCase(2, {
  title: '产品写实',
  prompt: 'A hidden token lives only in the complete prompt',
  featured: true,
  category: { id: 'product', label: '产品与电商', sourceLabel: 'Products & E-commerce' },
  styles: [{ sourceLabel: 'Realistic', label: '写实' }, { sourceLabel: 'Poster', label: '海报' }],
  scenes: [{ sourceLabel: 'Education', label: '教育' }],
});
const three = makeCase(3, {
  category: { id: 'product', label: '产品与电商', sourceLabel: 'Products & E-commerce' },
  styles: [{ sourceLabel: 'Realistic', label: '写实' }, { sourceLabel: 'Product', label: '产品' }],
  scenes: [{ sourceLabel: 'Commerce', label: '商业' }],
});

describe('prompt library query', () => {
  it('searches complete prompt text and intersects dimensions', () => {
    const query = {
      ...emptyPromptQuery(),
      search: 'hidden education',
      categoryId: 'product',
      styles: ['Realistic'],
      scenes: ['Education'],
    };
    expect(filterPromptCases([one, two, three], query, new Set()).map(item => item.sourceCaseId)).toEqual([2]);
  });

  it('uses OR within style selection and featured-first default sorting', () => {
    const query = { ...emptyPromptQuery(), styles: ['Poster', 'Product'] };
    expect(filterPromptCases([one, three, two], query, new Set()).map(item => item.sourceCaseId)).toEqual([2, 3, 1]);
  });

  it('filters favorites and reports removed favorite placeholders', () => {
    const favorites = new Set([two.id]);
    expect(filterPromptCases([one, two], { ...emptyPromptQuery(), view: 'favorites' }, favorites)).toEqual([two]);
    const catalog = { cases: [one, two] } as PromptCatalog;
    expect(missingFavoriteIds([one.id, 'removed'], catalog)).toEqual(['removed']);
  });

  it('keeps tags togglable and labels use conditions', () => {
    expect(toggleSelection(['Poster'], 'Poster')).toEqual([]);
    expect(toggleSelection(['Poster'], 'Product')).toEqual(['Poster', 'Product']);
    expect(promptUsageLabel(makeCase(4, {
      usage: { inputKind: 'reference-image', hasVariables: true, notes: [], reviewStatus: 'reviewed' },
    }))).toBe('需要参考图');
  });

  it('keeps the pinned upstream license links available for the about entry', () => {
    const links = promptLicenseLinks({
      upstreamRepository: 'https://example.test/catalog',
      upstreamCommit: 'abc123def4567890',
    });
    expect(links.licenseUrl).toBe('https://example.test/catalog/blob/abc123def4567890/LICENSE');
    expect(links.disclaimerUrl).toBe('https://example.test/catalog/blob/abc123def4567890/docs/disclaimer.md');
    expect(links.repository).toBe('https://example.test/catalog');
    expect(shortCommit('abc123def4567890')).toBe('abc123de');
    expect(shortCommit(PROMPT_LIBRARY_UPSTREAM_COMMIT)).toHaveLength(8);
  });
});
