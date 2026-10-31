export type PromptTag = { sourceLabel: string; label: string };
export type PromptInputKind = 'text-to-image' | 'image-edit' | 'reference-image' | 'unknown';
export type PromptCase = {
  id: string;
  sourceCaseId: number;
  title: string;
  prompt: string;
  imageAlt: string;
  featured: boolean;
  category: { id: string; label: string; sourceLabel: string };
  styles: PromptTag[];
  scenes: PromptTag[];
  image: {
    path: string; filename: string; sha256: string; bytes: number;
    mimeType: 'image/jpeg' | 'image/png'; width: number; height: number;
  };
  source: { label: string; url: string | null; githubUrl: string };
  usage: {
    inputKind: PromptInputKind; hasVariables: boolean; notes: string[];
    reviewStatus: string;
  };
  model: { upstreamProject: string; verification: string; exactModelId: string | null };
  rights: { commercialUse: string; redistribution: string; sourceReferenceRequired: boolean };
};
export type PromptCategory = {
  id: string; label: string; sourceLabel: string; count: number;
};
export type PromptCatalog = {
  schemaVersion: number;
  catalogVersion: string;
  upstreamRepository: string;
  upstreamCommit: string;
  importedAt: string;
  sourceCount: number;
  importedCount: number;
  categories: PromptCategory[];
  styles: PromptTag[];
  scenes: PromptTag[];
  sourceRights: Record<string, string>;
  imageCachePolicy: {
    bundled: boolean; maxFileBytes: number; maxCacheBytes: number;
    maxPixels: number; allowedMimeTypes: string[];
  };
  cases: PromptCase[];
};
export type PromptLibrarySnapshot = {
  catalog: PromptCatalog;
  favorites: string[];
  favoritesLoadError: string | null;
  favoritesSaveError: string | null;
  cache: { bytes: number; count: number; limitBytes: number };
};
export type PromptView = 'all' | 'featured' | 'favorites';
export type PromptSort = 'default' | 'id-asc' | 'id-desc';
export type PromptQuery = {
  search: string;
  categoryId: string;
  styles: string[];
  scenes: string[];
  view: PromptView;
  sort: PromptSort;
};

export const emptyPromptQuery = (): PromptQuery => ({
  search: '',
  categoryId: '',
  styles: [],
  scenes: [],
  view: 'all',
  sort: 'default',
});

const normalize = (value: string) => value.toLocaleLowerCase();

const caseTokens = (item: PromptCase): string => normalize([
  item.title,
  item.prompt,
  item.category.label,
  item.category.sourceLabel,
  ...item.styles.flatMap(tag => [tag.label, tag.sourceLabel]),
  ...item.scenes.flatMap(tag => [tag.label, tag.sourceLabel]),
  item.source.label,
].join('\n'));

export function filterPromptCases(
  cases: PromptCase[],
  query: PromptQuery,
  favorites: ReadonlySet<string>,
): PromptCase[] {
  const tokens = query.search.trim().split(/\s+/).filter(Boolean).map(normalize);
  const styles = new Set(query.styles);
  const scenes = new Set(query.scenes);
  const result = cases.filter(item => {
    if (query.categoryId && item.category.id !== query.categoryId) return false;
    if (query.view === 'featured' && !item.featured) return false;
    if (query.view === 'favorites' && !favorites.has(item.id)) return false;
    if (styles.size && !item.styles.some(tag => styles.has(tag.sourceLabel))) return false;
    if (scenes.size && !item.scenes.some(tag => scenes.has(tag.sourceLabel))) return false;
    if (tokens.length) {
      const searchable = caseTokens(item);
      if (!tokens.every(token => searchable.includes(token))) return false;
    }
    return true;
  });
  return result.sort((left, right) => {
    if (query.sort === 'id-asc') return left.sourceCaseId - right.sourceCaseId;
    if (query.sort === 'id-desc') return right.sourceCaseId - left.sourceCaseId;
    if (left.featured !== right.featured) return left.featured ? -1 : 1;
    return right.sourceCaseId - left.sourceCaseId;
  });
}

export function toggleSelection(values: string[], value: string): string[] {
  return values.includes(value)
    ? values.filter(item => item !== value)
    : [...values, value];
}

export function promptUsageLabel(item: PromptCase): string {
  if (item.usage.inputKind === 'reference-image') return '需要参考图';
  if (item.usage.inputKind === 'image-edit') return '需要图片编辑';
  if (item.usage.hasVariables) return '含可替换变量';
  if (item.usage.inputKind === 'unknown') return '用法未知';
  return '纯文生图';
}

export function sortFavoriteIds(ids: string[], cases: PromptCase[]): string[] {
  const order = new Map(cases.map(item => [item.id, item.sourceCaseId]));
  return [...new Set(ids)].sort((left, right) =>
    (order.get(left) ?? Number.MAX_SAFE_INTEGER) - (order.get(right) ?? Number.MAX_SAFE_INTEGER));
}

export function missingFavoriteIds(ids: string[], catalog: PromptCatalog): string[] {
  const known = new Set(catalog.cases.map(item => item.id));
  return ids.filter(id => !known.has(id));
}
