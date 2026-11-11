/**
 * Pure formatting helpers for the inline production record.
 *
 * The record is shared by conversation result cards and the works preview, so
 * it must not depend on either page. Host facts arrive through the existing
 * single-Job query (`task_center_detail`); this module only turns them into
 * readable text and never invents data the host did not report.
 */

const categoryLabels: Record<string, string> = {
  validation: '输入或参数',
  memory: '内存',
  runtime: '运行环境',
  cancel: '取消',
  interrupt: '宿主中断',
  storage: '存储',
  other: '其他执行',
};

export function errorCategoryLabel(category: string | null | undefined): string {
  if (!category) return '';
  return categoryLabels[category] ?? category;
}

/** Show the requested size when it differs from the committed image size. */
export function dimensionText(
  requestedWidth: number,
  requestedHeight: number,
  width: number,
  height: number,
): string {
  const requested = `${requestedWidth}×${requestedHeight}`;
  return requestedWidth === width && requestedHeight === height
    ? `${requested} 请求`
    : `${requested} 请求 · ${width}×${height} 实际`;
}

export function manifestText(manifest: unknown): string {
  if (manifest === null || manifest === undefined) return '尚无完整 manifest';
  try {
    return JSON.stringify(manifest, null, 2) ?? '尚无完整 manifest';
  } catch {
    return '制作记录无法序列化';
  }
}
