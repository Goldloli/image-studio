import { useEffect, useId, useRef } from 'react';
import type { ReactNode } from 'react';
import { LoaderCircle } from 'lucide-react';

/**
 * Destructive confirmation bound to a stable target identity. The cancel action
 * receives the initial focus, Escape cancels, and nothing can be confirmed while
 * the host request is in flight.
 */
export function ConfirmDialog({ title, description, confirmLabel = '确认删除', busy = false, error = '', onCancel, onConfirm }: {
  title: string;
  description: ReactNode;
  confirmLabel?: string;
  busy?: boolean;
  error?: string;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  const titleId = useId();
  const descriptionId = useId();

  useEffect(() => {
    cancelRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key !== 'Escape' || event.isComposing || busy) return;
      event.preventDefault();
      onCancel();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [busy, onCancel]);

  return <div className="modal-overlay" onPointerDown={event => {
    if (event.target === event.currentTarget && !busy) onCancel();
  }}>
    <section className="confirm-dialog" role="alertdialog" aria-modal="true" aria-labelledby={titleId} aria-describedby={descriptionId}>
      <h2 id={titleId}>{title}</h2>
      <div id={descriptionId} className="confirm-description">{description}</div>
      {error && <p className="confirm-error" role="alert">{error}</p>}
      <div className="confirm-actions">
        <button ref={cancelRef} className="secondary" disabled={busy} onClick={onCancel}>取消</button>
        <button className="danger" disabled={busy} onClick={onConfirm}>
          {busy ? <LoaderCircle size={14} className="spin"/> : null}
          {confirmLabel}
        </button>
      </div>
    </section>
  </div>;
}
