import { describe, expect, it } from 'vitest';
import { clampZoom, initialWorkspace, validSeed, workspaceReducer as reduce } from './workspace-state';

describe('workspace draft isolation',()=>{
  it('routes late IME events to their original draft after navigation',()=>{
    let state=reduce(initialWorkspace(),{type:'edit',id:1,value:'zhongwen',record:false});
    state=reduce(state,{type:'new'});
    state=reduce(state,{type:'edit',id:2,value:'另一个草稿'});
    state=reduce(state,{type:'edit',id:1,value:'中文',before:''});
    expect(state.activeId).toBe(2);
    expect(state.drafts[0].prompt).toBe('中文');
    expect(state.drafts[1].prompt).toBe('另一个草稿');
  });
  it('keeps the editing target while viewing an example',()=>{
    let state=reduce(initialWorkspace(),{type:'edit',value:'草稿 A'});
    state=reduce(state,{type:'new'});
    state=reduce(state,{type:'edit',value:'草稿 B'});
    state=reduce(state,{type:'select',id:1});
    state=reduce(state,{type:'sample',index:2});
    state=reduce(state,{type:'edit',value:'替换后的 A'});
    expect(state.activeId).toBe(1);
    expect(state.sample).toBe(2);
    expect(state.drafts[1].prompt).toBe('草稿 B');
    expect(state.drafts[0].prompt).toBe('替换后的 A');
  });
  it('reverts template replacement in one undo without using another draft history',()=>{
    let state=reduce(initialWorkspace(),{type:'edit',value:'原先的想法'});
    state=reduce(state,{type:'new'});
    state=reduce(state,{type:'edit',value:'另外一份草稿'});
    state=reduce(state,{type:'select',id:1});
    state=reduce(state,{type:'edit',value:'很长的示例提示词'});
    state=reduce(state,{type:'undo'});
    expect(state.drafts[0].prompt).toBe('原先的想法');
    expect(state.drafts[1].prompt).toBe('另外一份草稿');
    state=reduce(state,{type:'redo'});
    expect(state.drafts[0].prompt).toBe('很长的示例提示词');
  });
  it('composition commits a whole candidate rather than its transient preedit',()=>{
    let state=initialWorkspace();
    state=reduce(state,{type:'edit',value:'zhongwen',record:false});
    state=reduce(state,{type:'edit',value:'中文',before:''});
    state=reduce(state,{type:'edit',value:'中文'});
    state=reduce(state,{type:'undo'});
    expect(state.drafts[0].prompt).toBe('');
    expect(state.drafts[0].future).toEqual(['中文']);
  });
  it('keeps long confirmed text and parameters through new/select/collapse',()=>{
    const prompt='长文本\n'.repeat(1500);
    let state=reduce(initialWorkspace(),{type:'edit',value:prompt});
    state=reduce(state,{type:'params',patch:{aspect:'4:3',count:1,seed:'4294967295'}});
    state=reduce(state,{type:'collapse'});
    state=reduce(state,{type:'new'});
    expect(state.drafts[1].prompt).toBe('');
    state=reduce(state,{type:'select',id:1});
    expect(state.drafts[0].prompt).toBe(prompt);
    expect(state.drafts[0].collapsed).toBe(true);
    expect(state.drafts[0].params).toEqual({modelId:'z-image-turbo-mflux-v1',aspect:'4:3',count:1,seed:'4294967295'});
  });
  it('creates a new conversation with application defaults instead of copying the current draft',()=>{
    let state=reduce(initialWorkspace(),{type:'edit',value:'当前会话'});
    state=reduce(state,{type:'params',patch:{aspect:'3:4',count:1,seed:'73'}});
    state=reduce(state,{type:'new'});
    expect(state.drafts[1].prompt).toBe('');
    expect(state.drafts[1].params).toEqual({modelId:'z-image-turbo-mflux-v1',aspect:'1:1',count:1,seed:''});
    expect(state.drafts[0].params).toEqual({modelId:'z-image-turbo-mflux-v1',aspect:'3:4',count:1,seed:'73'});
  });
  it('clears only the acknowledged source conversation after send',()=>{
    let state=reduce(initialWorkspace(),{type:'edit',value:'发送中的 A'});
    state=reduce(state,{type:'append',draft:{...state.drafts[0],id:2,name:'会话 B',prompt:'B 保留'}});
    state=reduce(state,{type:'consume',draftId:1,name:'发送后的 A',params:{modelId:'z-image-turbo-mflux-v1',aspect:'1:1',count:1,seed:''}});
    expect(state.drafts[0].prompt).toBe('');
    expect(state.drafts[1].prompt).toBe('B 保留');
  });
  it('rejects invalid quantity and aspect rather than silently changing capability',()=>{
    const state=initialWorkspace();
    for(const patch of [{count:0},{count:2},{count:4},{count:1.5},{aspect:'2:3'},{aspect:'16:9'}]){
      expect(reduce(state,{type:'params',patch}).drafts[0].params).toEqual(state.drafts[0].params);
    }
    expect(reduce(state,{type:'select',id:99})).toBe(state);
  });
});

describe('canvas and input boundaries',()=>{
  it('clamps zoom to visible bounds with quarter steps',()=>{
    expect(clampZoom(-1)).toBe(.25);
    expect(clampZoom(9)).toBe(4);
    expect(clampZoom(1.6)).toBe(1.5);
  });
  it('validates optional integer seed over the full u32 range',()=>{
    for(const seed of ['', '0', '4294967295']) expect(validSeed(seed)).toBe(true);
    for(const seed of ['-1','4294967296','1.5','1e3',' ', 'seed']) expect(validSeed(seed)).toBe(false);
  });
});
