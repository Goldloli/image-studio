import type { DraftParams } from './workspace-state';
import type { ExecutionView, TurnView } from './studio-api';

export type TurnProgress = { percent: number; label: string };
export type ComposerAction =
  | { kind: 'send'; disabled: boolean }
  | { kind: 'stop'; disabled: boolean }
  | { kind: 'submitting'; disabled: true };

export function createSubmissionGate() {
  let active = false;
  return {
    tryStart() {
      if (active) return false;
      active = true;
      return true;
    },
    finish() {
      active = false;
    },
    isActive() {
      return active;
    },
  };
}

export function composerActionFor(input: {
  submittingDraftId: number | null;
  currentActiveTurn: TurnView | null | undefined;
  canSend: boolean;
}): ComposerAction {
  if (input.submittingDraftId !== null) return { kind: 'submitting', disabled: true };
  if (input.currentActiveTurn) {
    return { kind: 'stop', disabled: input.currentActiveTurn.state === 'CancelRequested' };
  }
  return { kind: 'send', disabled: !input.canSend };
}

export function claimOneShotNonce(
  requested: number | null,
  processed: number | null,
): { claimed: boolean; processed: number | null } {
  if (requested === null || requested === processed) return { claimed: false, processed };
  return { claimed: true, processed: requested };
}

export function isPrimaryActivation(clickCount: number): boolean {
  return clickCount <= 1;
}

export function progressForTurn(
  turn: TurnView,
  execution: ExecutionView | null | undefined,
  imageReady = false,
): TurnProgress {
  const activeExecution = execution?.jobId === turn.jobId ? execution : null;
  const stage = activeExecution?.stage && activeExecution.stage !== 'idle'
    ? activeExecution.stage
    : turn.progressStage;
  const step = activeExecution?.jobId === turn.jobId ? activeExecution.step : turn.progressStep;
  const total = activeExecution?.jobId === turn.jobId ? activeExecution.total : turn.progressTotal;
  if (turn.state === 'Succeeded') {
    return imageReady ? { percent: 100, label: '图片已生成' } : { percent: 99, label: '正在显示图片' };
  }
  if (stage === 'loading') return { percent: 10, label: '正在加载模型' };
  if (stage === 'encoding') return { percent: 15, label: '正在编码提示词' };
  if (stage === 'sampling') {
    const resolvedTotal = total && total > 0 ? total : 9;
    const resolvedStep = Math.max(0, Math.min(step ?? 0, resolvedTotal));
    return {
      percent: Math.floor(20 + 65 * resolvedStep / resolvedTotal),
      label: resolvedStep >= resolvedTotal ? `采样 ${resolvedStep}/${resolvedTotal} · 正在处理` : `正在采样 ${resolvedStep}/${resolvedTotal}`,
    };
  }
  if (stage === 'decoding') return { percent: 90, label: '正在解码图片' };
  if (stage === 'writing') return { percent: 95, label: '正在验证并保存' };
  if (stage === 'committing') return { percent: 95, label: '正在提交结果' };
  if (turn.state === 'Pending') return { percent: 2, label: '正在接收提示词' };
  if (turn.state === 'Submitted') return { percent: 5, label: '已接收，等待预检' };
  if (turn.state === 'Preparing') return { percent: 10, label: '正在准备生成' };
  if (turn.state === 'Running') return { percent: 20, label: '正在生成' };
  if (turn.state === 'Committing') return { percent: 90, label: '正在保存结果' };
  if (turn.state === 'CancelRequested') return { percent: 8, label: '正在取消' };
  return { percent: 0, label: turn.state === 'Rejected' ? '发送未启动' : '执行已结束' };
}

export function mergeTurns(serverTurns: TurnView[], optimistic: TurnView[]): TurnView[] {
  const accepted = new Set(serverTurns.map(turn => turn.submissionKey));
  return [
    ...serverTurns,
    ...optimistic.filter(turn => !accepted.has(turn.submissionKey)),
  ];
}

export function shouldSendOnEnter(input: {
  key: string;
  shiftKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  repeat: boolean;
  isComposing: boolean;
  keyCode: number;
  compositionEndedAt: number;
  now: number;
}): boolean {
  if (input.key !== 'Enter' || input.shiftKey || input.ctrlKey || input.metaKey || input.altKey || input.repeat) return false;
  if (input.isComposing || input.keyCode === 229) return false;
  return !input.compositionEndedAt || input.now - input.compositionEndedAt > 200;
}

export function supportedDraftParams(params: DraftParams): boolean {
  const seedValid = /^(?:|[0-9]+)$/.test(params.seed)
    && (params.seed === '' || Number(params.seed) <= 4294967295);
  if (params.modelId === 'z-image-turbo-mflux-v1') {
    return (params.capabilityId ?? 'image.text-to-image.z-image-turbo.v1') === 'image.text-to-image.z-image-turbo.v1'
      && ['1:1', '4:3', '3:4'].includes(params.aspect)
      && params.count === 1 && !params.sourceAssetId && seedValid;
  }
  if (params.modelId === 'flux2-klein-4b-distilled') {
    const text = params.capabilityId === 'image.text-to-image.flux2-klein-4b.v1' && !params.sourceAssetId;
    const edit = params.capabilityId === 'image.edit.flux2-klein-4b.v1' && Boolean(params.sourceAssetId);
    return params.aspect === '1:1' && params.count === 1 && (text || edit) && seedValid;
  }
  return false;
}
