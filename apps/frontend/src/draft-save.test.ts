import { describe, expect, it } from 'vitest';
import { DraftSaveQueue, type StoredDraft } from './studio-api';
import { confirmedDraft, initialWorkspace, workspaceReducer } from './workspace-state';

const draft: StoredDraft = { id: 1, name: '新会话', prompt: '', params: { modelId:'z-image-turbo-mflux-v1', aspect: '1:1', count: 1, seed: '' }, collapsed: false, version: 1 };
describe('durable draft acknowledgements', () => {
  it('flushes the last confirmed edit when a new IME composition is still open',async()=>{
    let state=workspaceReducer(initialWorkspace(),{type:'edit',value:'刚确认但未等自动保存的文字'});
    const composing=new Map([[1,state.drafts[0].prompt]]);
    state=workspaceReducer(state,{type:'edit',id:1,value:'刚确认但未等自动保存的文字pinyin',record:false});
    const saved:string[]=[];
    const queue=new DraftSaveQueue(async(value,version)=>{saved.push(value.prompt);return {...value,version:version+1};},()=>{});
    queue.initialize([draft]);queue.enqueue(confirmedDraft(state.drafts[0],composing));await queue.flush();
    expect(saved).toEqual(['刚确认但未等自动保存的文字']);
    expect(state.drafts[0].prompt).toContain('pinyin');
  });
  it('serializes and coalesces edits that arrive during a write', async () => {
    const calls: string[] = [], statuses: string[] = [];
    let release: (() => void) | undefined;
    const queue = new DraftSaveQueue(async (value, version) => {
      calls.push(value.prompt);
      if (calls.length === 1) await new Promise<void>(resolve => { release = resolve; });
      return { ...value, version: version + 1 };
    }, status => statuses.push(status));
    queue.initialize([draft]);
    queue.enqueue({ ...draft, prompt: '中文 A' });
    const first = queue.flush();
    queue.enqueue({ ...draft, prompt: '中文 B' });
    queue.enqueue({ ...draft, prompt: '中文 C' });
    expect(queue.flush()).toBe(first);
    expect(statuses).not.toContain('saved');
    release!(); await first;
    expect(calls).toEqual(['中文 A', '中文 C']);
    expect(statuses.at(-1)).toBe('saved');
    expect(queue.hasPending()).toBe(false);
  });
  it('retains a failed write and retries with the last acknowledged version', async () => {
    let fail = true;
    const versions: number[] = [];
    const queue = new DraftSaveQueue(async (value, version) => {
      versions.push(version);
      if (fail) throw new Error('磁盘不足');
      return { ...value, version: version + 1 };
    }, () => {});
    queue.initialize([draft]); queue.enqueue({ ...draft, prompt: '必须保留' });
    await expect(queue.flush()).rejects.toThrow('磁盘不足');
    expect(queue.hasPending()).toBe(true);
    fail = false; await queue.flush();
    expect(versions).toEqual([1, 1]);
    expect(queue.hasPending()).toBe(false);
  });
  it('keeps conversation versions isolated when an older acknowledgement arrives', async () => {
    const second: StoredDraft = { ...draft, id: 2, name: '会话 B', prompt: 'B 原文' };
    const queue = new DraftSaveQueue(async (value, version) => ({ ...value, version: version + 1 }), () => {});
    queue.initialize([draft, second]);
    queue.enqueue({ ...draft, prompt: 'A 已发送' });
    queue.enqueue({ ...second, prompt: 'B 新输入' });
    await queue.flush();
    expect(queue.version(1)).toBe(2);
    expect(queue.version(2)).toBe(2);
    queue.remember({ ...draft, prompt: '', version: 3 });
    expect(queue.version(1)).toBe(3);
    expect(queue.savedContent(2)).toContain('B 新输入');
  });
});
