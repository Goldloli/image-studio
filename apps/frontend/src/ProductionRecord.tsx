import { useEffect, useRef, useState } from 'react';
import { ChevronDown, LoaderCircle, RefreshCw } from 'lucide-react';
import { api, native, stateLabel, type TaskCenterDetail } from './studio-api';
import { formatTime, modelLabel } from './library-state';
import { dimensionText, errorCategoryLabel, manifestText } from './production-record';

const activeStates = ['Pending', 'Submitted', 'Preparing', 'Running', 'Committing', 'CancelRequested'];

/**
 * Readable production record for one stable Job identity.
 *
 * The record is opened explicitly by the session card or the works preview, so
 * it only queries the host while mounted and never mounts a global task list or
 * its polling. Retry is local to the record; the caller's surrounding context
 * keeps its state.
 */
export function ProductionRecord({ jobId, fallback, onError, showSummary = true }: {
  jobId: string | null;
  fallback?: string;
  onError?: (message: string) => void;
  showSummary?: boolean;
}) {
  const [detail, setDetail] = useState<TaskCenterDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [attempt, setAttempt] = useState(0);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const sequence = useRef(0);
  const onErrorRef = useRef(onError);
  onErrorRef.current = onError;

  useEffect(() => {
    setDetail(null);
    setError('');
    setAdvancedOpen(false);
    if (!jobId || !native) {
      setLoading(false);
      return;
    }
    const current = ++sequence.current;
    setLoading(true);
    api<TaskCenterDetail>('task_center_detail', { jobId })
      .then(value => { if (current === sequence.current) setDetail(value); })
      .catch(cause => {
        if (current !== sequence.current) return;
        const message = String(cause);
        setError(message);
        onErrorRef.current?.(message);
      })
      .finally(() => { if (current === sequence.current) setLoading(false); });
    return () => { sequence.current += 1; };
  }, [jobId, attempt]);

  if (!jobId) return <p className="record-note">{fallback ?? '请求尚未创建任务。'}</p>;
  if (!native) return <p className="record-note">{fallback ?? '制作记录仅在桌面应用中读取。'}</p>;
  if (loading && !detail) return <p className="record-note" role="status"><LoaderCircle size={12} className="spin"/>正在读取制作记录…</p>;
  if (error && !detail) return <div className="record-error" role="alert"><span>制作记录读取失败：{error}</span><button type="button" onClick={() => setAttempt(value => value + 1)}><RefreshCw size={11}/>重试</button></div>;
  if (!detail) return <p className="record-note">{fallback ?? '该任务尚无完整制作记录。'}</p>;

  const task = detail.task;
  const active = activeStates.includes(task.state);
  return <div className="production-record" aria-label="制作记录">
    <p className="record-status" role="status">
      <span className={`record-state state-${task.state}`}>{stateLabel(task.state)}</span>
      {active && <LoaderCircle size={11} className="spin"/>}
      <strong>{detail.nextAction}</strong>
    </p>
    {showSummary && <dl className="record-summary">
      <div><dt>会话</dt><dd>{task.conversationTitle}</dd></div>
      <div><dt>型号</dt><dd>{modelLabel(task.modelId)}</dd></div>
      <div><dt>尺寸</dt><dd>{dimensionText(detail.requestedWidth, detail.requestedHeight, task.width, task.height)}</dd></div>
      <div><dt>种子</dt><dd>{task.seed === null ? '随机' : task.seed}</dd></div>
      <div><dt>创建</dt><dd>{formatTime(task.createdAt)}</dd></div>
      <div><dt>完成</dt><dd>{formatTime(task.completedAt)}</dd></div>
    </dl>}
    {task.error && <div className="record-error-card">
      <strong>{detail.errorCategory ? `错误 · ${errorCategoryLabel(detail.errorCategory)}` : '错误'}</strong>
      <p>{task.error}</p>
      {task.assetError && <p>资源异常：{task.assetError}</p>}
    </div>}
    {detail.attempts.length > 0 && <section className="record-attempts">
      <h4>Attempt 记录</h4>
      {detail.attempts.map(item => <div key={item.attemptId} className="record-attempt">
        <span className={`record-attempt-dot state-${item.state}`}/>
        <div>
          <strong>{stateLabel(item.state)} · {formatTime(item.startedAt)}</strong>
          {item.phase && <small>阶段 {item.phase}</small>}
          {item.error && <small className="error-text">{item.error}</small>}
        </div>
      </div>)}
    </section>}
    <button type="button" className="record-advanced-toggle" aria-expanded={advancedOpen} onClick={() => setAdvancedOpen(value => !value)}>
      完整制作记录<ChevronDown size={13} className={advancedOpen ? 'rotated' : ''}/>
    </button>
    {advancedOpen && <div className="record-advanced">
      <div><span>Job</span><code>{task.id}</code></div>
      {task.attemptId && <div><span>当前 Attempt</span><code>{task.attemptId}</code></div>}
      <div><span>输出哈希</span><code>{detail.advanced.outputHash ?? '尚无正式输出'}</code></div>
      <h4>RunManifest</h4>
      <pre>{manifestText(detail.advanced.manifest)}</pre>
      <h4>运行日志（最多 64 KiB）</h4>
      <pre>{detail.advanced.logs || '当前 Attempt 没有可读取日志。'}</pre>
    </div>}
  </div>;
}
