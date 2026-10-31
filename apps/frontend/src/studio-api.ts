import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';
import type { DraftParams } from './workspace-state';

export const native = isTauri();
export type StoredDraft = { id: number; name: string; prompt: string; collapsed: boolean; promptSourceId?: string | null; params: DraftParams; version: number };
export type ModelView = { id: string; name: string; phase: string; summary: string; ready: boolean; runtimeReady: boolean; modelReady: boolean; deviceVerified: boolean; receivedBytes: number; totalBytes: number; modelBytes: number; runtimeBytes: number; requiredBytes: number; error: string | null; busy: boolean };
export type JobView = { id: string; turnId: string | null; conversationId: string | null; attemptId: string; state: string; prompt: string; width: number; height: number; seed: number | null; error: string | null; assetId: string | null; manifest: unknown | null };
export type AssetView = { id: string; jobId: string; conversationId: string | null; turnId: string | null; prompt: string; width: number; height: number; seed: number | null; bytes: number; available: boolean; error: string | null };
export type StableCursor = { createdAt: number; id: string };
export type AssetLibraryItem = {
  id: string; jobId: string; attemptId: string; turnId: string | null; turnSequence: number | null;
  conversationId: string | null; conversationTitle: string; draftId: number | null; prompt: string;
  modelId: string; width: number; height: number; seed: number | null; byteCount: number;
  createdAt: number; available: boolean; availabilityError: string | null;
};
export type AssetLibraryFacets = {
  models: { id: string; count: number }[];
  sizes: { width: number; height: number; count: number }[];
  conversations: { id: string; title: string; count: number }[];
};
export type AssetLibraryPage = {
  items: AssetLibraryItem[]; total: number; hasMore: boolean; nextCursor: StableCursor | null; facets: AssetLibraryFacets;
};
export type AssetLibraryQuery = {
  modelId?: string | null; conversationId?: string | null; width?: number | null; height?: number | null;
  createdFrom?: number | null; createdTo?: number | null; cursor?: StableCursor | null; limit?: number;
};
export type TaskStatus = 'all' | 'active' | 'succeeded' | 'failed' | 'cancelled' | 'interrupted';
export type TaskCounts = { total: number; active: number; succeeded: number; failed: number; cancelled: number; interrupted: number };
export type TaskCenterItem = {
  id: string; turnId: string | null; turnSequence: number | null; conversationId: string | null; conversationTitle: string; draftId: number | null;
  attemptId: string; state: string; prompt: string; modelId: string; width: number; height: number; seed: number | null;
  createdAt: number; completedAt: number | null; error: string | null; assetId: string | null;
  assetAvailable: boolean; assetError: string | null;
};
export type TaskCenterPage = { items: TaskCenterItem[]; counts: TaskCounts; hasMore: boolean; nextCursor: StableCursor | null };
export type TaskCenterQuery = {
  status: TaskStatus; search?: string; conversationId?: string | null; modelId?: string | null;
  cursor?: StableCursor | null; limit?: number;
};
export type TaskAttemptView = { attemptId: string; state: string; startedAt: number; completedAt: number | null; phase: string | null; error: string | null };
export type TaskCenterDetail = {
  task: TaskCenterItem; requestedWidth: number; requestedHeight: number; requestedSeed: unknown;
  attempts: TaskAttemptView[]; errorCategory: string; nextAction: string;
  advanced: { manifest: unknown | null; outputHash: string | null; logs: string };
};
export type FeatureCatalogEntry = {
  featureId: string; title: string; category: string; summary: string; description: string;
  inputs: string[]; outputs: string[]; productStatus: 'implemented' | 'planned'; scope: string;
  action: 'open-composer' | null; modelId: string | null; dependencies: { kind: string; label: string }[];
  state: 'available' | 'planned' | 'busy' | 'blocked' | 'model-missing' | 'runtime-not-ready' | 'device-not-ready';
  stateLabel: string; blockingReason: string | null; canStart: boolean;
};
export type FeatureCatalog = {
  schemaVersion: number; reviewedDate: string; entries: FeatureCatalogEntry[];
  counts: { total: number; available: number; planned: number };
};
export type ExecutionView = { busy: boolean; jobId: string | null; attemptId: string | null; stage: string; summary: string; step: number | null; total: number | null; logs: string; cleanupError: string | null };
export type ConversationView = { id: string; draftId: number; title: string; titleSource: 'auto' | 'manual'; createdAt: number; activitySeq: number; running: boolean };
export type TurnView = {
  id: string; conversationId: string; sequence: number; submissionKey: string; prompt: string; modelId: string;
  capabilityId: string; sourceAssetId: string | null;
  aspect: string; count: number; seed: number | null; requestedSeed: string; status: string; state: string;
  jobId: string | null; attemptId: string | null; width: number; height: number; assetId: string | null;
  assetAvailable: boolean; assetError: string | null; error: string | null; progressStage: string | null; progressStep: number | null;
  progressTotal: number | null; receivedAt: number;
};
export type TurnDetail = { turnId: string; attempts: JobView[] };
export type TurnPage = { turns: TurnView[]; hasMore: boolean; nextCursor: number | null };
export type StudioSnapshot = {
  model: ModelView; models: ModelView[]; drafts: StoredDraft[]; activeId: number | null; activeConversationId: string | null;
  conversations: ConversationView[]; conversationsHasMore: boolean; conversationsNextCursor: number | null;
  turns: TurnView[]; turnsHasMore: boolean; turnsNextCursor: number | null;
  selectedAssetId: string | null; jobs: JobView[]; jobCounts:[number,number,number]; assets: AssetView[];
  execution: ExecutionView; error: string | null;
};
export type ImportedSource = {
  assetId: string; contentHash: string; width: number; height: number; byteCount: number;
  originalName: string; originalSha256: string; orientation: string; processingVersion: string;
  hasTransparentPixels: boolean;
};
export type SourceImportAck = ImportedSource | { cancelled: true };
export const api = <T>(command: string, args?: Record<string, unknown>) => invoke<T>(command, args);
export const assetUrl = (id: string) => convertFileSrc(id, 'studio-asset');
export const promptImageUrl = (sourceCaseId: number) => convertFileSrc(String(sourceCaseId), 'studio-prompt');
export const bytesLabel = (value: number) => value >= 1e9 ? `${(value / 1e9).toFixed(2)} GB` : value >= 1e6 ? `${(value / 1e6).toFixed(1)} MB` : `${Math.round(value / 1e3)} KB`;
export const stateLabel = (state: string): string => ({ Pending: '正在发送', Submitted: '已接收', Preparing: '准备中', Running: '生成中', Committing: '保存结果', CancelRequested: '正在取消', Succeeded: '已完成', Failed: '失败', Rejected: '未启动', Cancelled: '已取消', Interrupted: '已中断' }[state] ?? state);

export const draftContent = (draft: Omit<StoredDraft, 'version'>) => ({ id: draft.id, name: draft.name, prompt: draft.prompt, collapsed: draft.collapsed, promptSourceId: draft.promptSourceId ?? null, params: { ...draft.params } });
type SaveStatus = 'saved' | 'pending' | 'saving' | 'error';

/** Serial, coalescing CAS writes. Acknowledging an older write never marks newer edits saved. */
export class DraftSaveQueue {
  private versions = new Map<number, number>();
  private saved = new Map<number, string>();
  private pending = new Map<number, Omit<StoredDraft, 'version'>>();
  private flight: Promise<void> | null = null;
  constructor(private write: (draft: Omit<StoredDraft, 'version'>, version: number) => Promise<StoredDraft>, private status: (value: SaveStatus, error?: string) => void) {}
  initialize(drafts: StoredDraft[]) { for (const draft of drafts) this.remember(draft); }
  remember(draft: StoredDraft) { this.versions.set(draft.id, draft.version); this.saved.set(draft.id, JSON.stringify(draftContent(draft))); }
  version(id: number) { return this.versions.get(id) ?? 0; }
  savedContent(id: number) { return this.saved.get(id) ?? null; }
  enqueue(draft: Omit<StoredDraft, 'version'>) {
    const value = draftContent(draft);
    if (JSON.stringify(value) === this.saved.get(draft.id) && !this.pending.has(draft.id)) return;
    this.pending.set(draft.id, value);
    this.status('pending');
  }
  hasPending() { return this.pending.size > 0; }
  flush(): Promise<void> {
    if (this.flight) return this.flight;
    this.flight = this.drain().finally(() => { this.flight = null; });
    return this.flight;
  }
  private async drain() {
    try {
      while (this.pending.size) {
        const [id, value] = this.pending.entries().next().value!;
        const serialized = JSON.stringify(value);
        if (serialized === this.saved.get(id)) { this.pending.delete(id); continue; }
        this.status('saving');
        const record = await this.write(value, this.versions.get(id) ?? 0);
        this.remember(record);
        if (JSON.stringify(this.pending.get(id)) === serialized) this.pending.delete(id);
      }
      this.status('saved');
    } catch (error) { this.status('error', String(error)); throw error; }
  }
}
