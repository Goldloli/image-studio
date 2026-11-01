import { describe, expect, it } from 'vitest';
import {
  claimOneShotNonce,
  composerActionFor,
  createSubmissionGate,
  isPrimaryActivation,
  mergeTurns,
  progressForTurn,
  shouldSendOnEnter,
  supportedDraftParams,
} from './conversation-state';
import type { ExecutionView, TurnView } from './studio-api';
import { DEFAULT_MODEL_ID } from './workspace-state';
import { Z_IMAGE_WINDOWS_CAPABILITY, Z_IMAGE_WINDOWS_MODEL_ID } from './model-recipes';

const turn: TurnView = {
  id: 'turn-1', conversationId: 'conversation-1', sequence: 1, submissionKey: 'send-1', prompt: '湖',
  modelId: DEFAULT_MODEL_ID, capabilityId: 'image.text-to-image.z-image-turbo.v1', sourceAssetId: null,
  aspect: '1:1', count: 1, seed: null, requestedSeed: '', status: 'Job',
  state: 'Running', jobId: 'job-1', attemptId: 'attempt-1', width: 1024, height: 1024,
  assetId: null, assetAvailable: false, assetError: null, error: null, progressStage: 'sampling',
  progressStep: 4, progressTotal: 9, receivedAt: 1,
};
const execution: ExecutionView = {
  busy: true, jobId: 'job-1', attemptId: 'attempt-1', stage: 'sampling', summary: '',
  step: 4, total: 9, logs: '', cleanupError: null,
};

describe('conversation progress', () => {
  it('maps real stages without pretending completion', () => {
    expect(progressForTurn(turn, execution)).toEqual({ percent: 48, label: '正在采样 4/9' });
    expect(progressForTurn({ ...turn, progressStep: 9 }, { ...execution, step: 9 })).toEqual({ percent: 85, label: '采样 9/9 · 正在处理' });
    expect(progressForTurn({ ...turn, state: 'Succeeded', progressStage: 'writing' }, null, false)).toEqual({ percent: 99, label: '正在显示图片' });
    expect(progressForTurn({ ...turn, state: 'Succeeded', progressStage: 'writing' }, null, true).percent).toBe(100);
    expect(progressForTurn({ ...turn, state: 'Failed' }, null).percent).toBe(48);
    expect(progressForTurn(turn, { ...execution, stage: 'inference', step: null, total: null })).toEqual({ percent: null, label: '正在执行本地推理' });
  });
});

describe('conversation turn identity', () => {
  it('drops optimistic duplicates only by submission identity', () => {
    const local = { ...turn, id: 'local-1', submissionKey: 'send-1', state: 'Pending', jobId: null };
    const other = { ...turn, id: 'local-2', submissionKey: 'send-2', state: 'Pending', jobId: null };
    expect(mergeTurns([turn], [local, other]).map(value => value.id)).toEqual(['turn-1', 'local-2']);
  });
});

describe('composer keyboard and parameters', () => {
  const base = { key: 'Enter', shiftKey: false, ctrlKey: false, metaKey: false, altKey: false, repeat: false, isComposing: false, keyCode: 13, compositionEndedAt: 0, now: 1000 };
  it('sends only on a plain confirmed Enter', () => {
    expect(shouldSendOnEnter(base)).toBe(true);
    expect(shouldSendOnEnter({ ...base, shiftKey: true })).toBe(false);
    expect(shouldSendOnEnter({ ...base, isComposing: true })).toBe(false);
    expect(shouldSendOnEnter({ ...base, keyCode: 229 })).toBe(false);
    expect(shouldSendOnEnter({ ...base, compositionEndedAt: 950 })).toBe(false);
    expect(shouldSendOnEnter({ ...base, compositionEndedAt: 700 })).toBe(true);
  });
  it('validates only the current fixed recipe', () => {
    expect(supportedDraftParams({ modelId: DEFAULT_MODEL_ID, aspect: '3:4', count: 1, seed: '4294967295' })).toBe(true);
    expect(supportedDraftParams({ modelId: DEFAULT_MODEL_ID, aspect: '16:9', count: 1, seed: '' })).toBe(false);
    expect(supportedDraftParams({ modelId: DEFAULT_MODEL_ID, aspect: '1:1', count: 2, seed: '' })).toBe(false);
    expect(supportedDraftParams({ modelId: Z_IMAGE_WINDOWS_MODEL_ID, capabilityId: Z_IMAGE_WINDOWS_CAPABILITY, aspect: '1:1', count: 1, seed: '' })).toBe(true);
    expect(supportedDraftParams({ modelId: Z_IMAGE_WINDOWS_MODEL_ID, capabilityId: Z_IMAGE_WINDOWS_CAPABILITY, aspect: '4:3', count: 1, seed: '' })).toBe(false);
    expect(supportedDraftParams({ modelId: 'flux2-klein-4b-distilled', capabilityId: 'image.text-to-image.flux2-klein-4b.v1', sourceAssetId: null, aspect: '1:1', count: 1, seed: '' })).toBe(true);
    expect(supportedDraftParams({ modelId: 'flux2-klein-4b-distilled', capabilityId: 'image.edit.flux2-klein-4b.v1', sourceAssetId: 'source-1', aspect: '1:1', count: 1, seed: '7' })).toBe(true);
    expect(supportedDraftParams({ modelId: 'flux2-klein-4b-distilled', capabilityId: 'image.edit.flux2-klein-4b.v1', sourceAssetId: null, aspect: '1:1', count: 1, seed: '' })).toBe(false);
    expect(supportedDraftParams({ modelId: 'flux2-klein-4b-distilled', capabilityId: 'image.text-to-image.flux2-klein-4b.v1', sourceAssetId: null, aspect: '4:3', count: 1, seed: '' })).toBe(false);
    expect(supportedDraftParams({ modelId: 'birefnet-general-epoch-244', capabilityId: 'image.remove-background.birefnet-general-epoch-244.v1', sourceAssetId: 'source-1', aspect: '1:1', count: 1, seed: '' })).toBe(true);
    expect(supportedDraftParams({ modelId: 'birefnet-general-epoch-244', capabilityId: 'image.remove-background.birefnet-general-epoch-244.v1', sourceAssetId: null, aspect: '1:1', count: 1, seed: '' })).toBe(false);
    expect(supportedDraftParams({ modelId: 'birefnet-general-epoch-244', capabilityId: 'image.remove-background.birefnet-general-epoch-244.v1', sourceAssetId: 'source-1', aspect: '1:1', count: 1, seed: '7' })).toBe(false);
    expect(supportedDraftParams({ modelId: 'realesrgan-x4plus', capabilityId: 'image.upscale.realesrgan-x4plus.v1', sourceAssetId: 'source-1', aspect: '1:1', count: 1, seed: '' })).toBe(true);
    expect(supportedDraftParams({ modelId: 'realesrgan-x4plus', capabilityId: 'image.upscale.realesrgan-x4plus.v1', sourceAssetId: null, aspect: '1:1', count: 1, seed: '' })).toBe(false);
    expect(supportedDraftParams({ modelId: 'realesrgan-x4plus', capabilityId: 'image.upscale.realesrgan-x4plus.v1', sourceAssetId: 'source-1', aspect: '4:3', count: 1, seed: '' })).toBe(false);
    expect(supportedDraftParams({ modelId: 'qwen-image-2.1-mflux-v1', capabilityId: 'image.text-to-image.qwen-image-2.1.v1', sourceAssetId: null, aspect: '1:1', count: 1, seed: '42' })).toBe(true);
    expect(supportedDraftParams({ modelId: 'qwen-image-2.1-mflux-v1', capabilityId: 'image.text-to-image.qwen-image-2.1.v1', sourceAssetId: null, aspect: '4:3', count: 1, seed: '' })).toBe(false);
    expect(supportedDraftParams({ modelId: 'qwen-image-2.1-turbo-mflux-v1', capabilityId: 'image.text-to-image.qwen-image-2.1-turbo.v1', sourceAssetId: null, aspect: '1:1', count: 1, seed: '' })).toBe(true);
    expect(supportedDraftParams({ modelId: 'moss-image-sft-mflux-v1', capabilityId: 'image.text-to-image.moss-image-sft.v1', sourceAssetId: null, aspect: '1:1', count: 1, seed: '' })).toBe(true);
    expect(supportedDraftParams({ modelId: 'moss-image-sft-mflux-v1', capabilityId: 'image.text-to-image.moss-image-sft.v1', sourceAssetId: 'source-1', aspect: '1:1', count: 1, seed: '' })).toBe(false);
  });
});

describe('composer submission boundary', () => {
  it('serializes repeated send attempts until the first submission settles', () => {
    const gate = createSubmissionGate();
    expect(gate.isActive()).toBe(false);
    expect(gate.tryStart()).toBe(true);
    expect(gate.isActive()).toBe(true);
    expect(gate.tryStart()).toBe(false);
    gate.finish();
    expect(gate.isActive()).toBe(false);
    expect(gate.tryStart()).toBe(true);
  });

  it('keeps the composer in submitting state even after the optimistic turn appears', () => {
    expect(composerActionFor({
      submittingDraftId: 1,
      currentActiveTurn: { ...turn, id: 'local-send-1', state: 'Pending', jobId: null },
      canSend: true,
    })).toEqual({ kind: 'submitting', disabled: true });
    expect(composerActionFor({
      submittingDraftId: null,
      currentActiveTurn: turn,
      canSend: true,
    })).toEqual({ kind: 'stop', disabled: false });
    expect(composerActionFor({
      submittingDraftId: null,
      currentActiveTurn: null,
      canSend: true,
    })).toEqual({ kind: 'send', disabled: false });
    expect(composerActionFor({
      submittingDraftId: null,
      currentActiveTurn: null,
      canSend: false,
    })).toEqual({ kind: 'send', disabled: true });
  });

  it('ignores the follow-up click of a multi-click gesture', () => {
    expect(isPrimaryActivation(0)).toBe(true);
    expect(isPrimaryActivation(1)).toBe(true);
    expect(isPrimaryActivation(2)).toBe(false);
    expect(isPrimaryActivation(3)).toBe(false);
  });
});

describe('one-shot navigation intents', () => {
  it('handles an asset nonce once and requires a new nonce for an explicit reopen', () => {
    const first = claimOneShotNonce(100, null);
    expect(first).toEqual({ claimed: true, processed: 100 });
    expect(claimOneShotNonce(100, first.processed)).toEqual({ claimed: false, processed: 100 });
    expect(claimOneShotNonce(101, first.processed)).toEqual({ claimed: true, processed: 101 });
    expect(claimOneShotNonce(null, 101)).toEqual({ claimed: false, processed: 101 });
  });
});
