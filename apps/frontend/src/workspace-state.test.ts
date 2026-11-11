import { describe, expect, it } from 'vitest';
import { clampZoom, createDraft, initialWorkspace, normalizeDraftParams, validSeed, workspaceReducer as reduce, workspaceReducer } from './workspace-state';
import type { DraftParams } from './workspace-state';
import { aspectsForCapability, BIREFNET_CAPABILITY, BIREFNET_MODEL_ID, KLEIN_EDIT_CAPABILITY, KLEIN_MODEL_ID, KLEIN_TEXT_CAPABILITY, Z_IMAGE_MODEL_ID } from './model-recipes';

describe('workspace draft isolation',()=>{
  it('persists separate prompts, source images and parameters across capability switches and tool sends',()=>{
    const z='image.text-to-image.z-image-turbo.v1';
    const r='image.upscale.realesrgan-x4plus.v1';
    const e='image.edit.flux2-klein-4b.v1';
    let state=reduce(initialWorkspace(),{type:'edit',value:'尚未发送的中文想法'});
    state=reduce(state,{type:'params',patch:{aspect:'4:3',seed:'42'}});
    state=reduce(state,{type:'params',patch:{modelId:'realesrgan-x4plus',capabilityId:r}});
    expect(state.drafts[0].prompt).toBe('');
    expect(state.drafts[0].params.seed).toBe('');
    state=reduce(state,{type:'params',patch:{sourceAssetId:'source-upscale'}});
    state=reduce(state,{type:'params',patch:{modelId:'flux2-klein-4b-distilled',capabilityId:e,sourceAssetId:'source-edit'}});
    state=reduce(state,{type:'edit',value:'只把外套改为蓝色'});
    state=reduce(state,{type:'params',patch:{modelId:'realesrgan-x4plus',capabilityId:r}});
    expect(state.drafts[0].prompt).toBe('');
    expect(state.drafts[0].params.sourceAssetId).toBe('source-upscale');
    state=reduce(state,{type:'consume',draftId:1,name:'放大',params:state.drafts[0].params});
    // JSON round-trip simulates the persisted params restored after app restart.
    state=JSON.parse(JSON.stringify(state));
    state=reduce(state,{type:'params',patch:{modelId:'z-image-turbo-mflux-v1',capabilityId:z}});
    expect(state.drafts[0].prompt).toBe('尚未发送的中文想法');
    expect(state.drafts[0].params).toMatchObject({aspect:'4:3',seed:'42',sourceAssetId:null});
    state=reduce(state,{type:'params',patch:{modelId:'flux2-klein-4b-distilled',capabilityId:e}});
    expect(state.drafts[0].prompt).toBe('只把外套改为蓝色');
    expect(state.drafts[0].params.sourceAssetId).toBe('source-edit');
  });
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
  it('applies a library prompt as one editable step and clears provenance after undo',()=>{
    let state=reduce(initialWorkspace(),{type:'edit',value:'原来的草稿'});
    state=reduce(state,{type:'params',patch:{seed:'42'}});
    state=reduce(state,{type:'applyPrompt',id:1,value:'完整案例原文\n[VARIABLE]',sourceId:'awesome-gpt-image-2:case:544',expected:'原来的草稿'});
    expect(state.drafts[0].prompt).toBe('完整案例原文\n[VARIABLE]');
    expect(state.drafts[0].promptSourceId).toBe('awesome-gpt-image-2:case:544');
    expect(state.drafts[0].params.seed).toBe('42');
    state=reduce(state,{type:'undo'});
    expect(state.drafts[0].prompt).toBe('原来的草稿');
    expect(state.drafts[0].promptSourceId).toBeNull();
  });
  it('rejects a stale prompt replacement and clears provenance after manual edits',()=>{
    let state=reduce(initialWorkspace(),{type:'edit',value:'新内容'});
    state=reduce(state,{type:'applyPrompt',id:1,value:'案例原文',sourceId:'awesome-gpt-image-2:case:544',expected:'旧内容'});
    expect(state.drafts[0].prompt).toBe('新内容');
    state=reduce(state,{type:'applyPrompt',id:1,value:'案例原文',sourceId:'awesome-gpt-image-2:case:544',expected:'新内容'});
    state=reduce(state,{type:'edit',value:'用户改写'});
    expect(state.drafts[0].promptSourceId).toBeNull();
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
    expect(state.drafts[0].params).toEqual({modelId:'z-image-turbo-mflux-v1',aspect:'4:3',count:1,seed:'4294967295',capabilityId:'image.text-to-image.z-image-turbo.v1',sourceAssetId:null});
  });
  it('creates a new conversation with application defaults instead of copying the current draft',()=>{
    let state=reduce(initialWorkspace(),{type:'edit',value:'当前会话'});
    state=reduce(state,{type:'params',patch:{aspect:'3:4',count:1,seed:'73'}});
    state=reduce(state,{type:'new'});
    expect(state.drafts[1].prompt).toBe('');
    expect(state.drafts[1].params).toEqual({modelId:'z-image-turbo-mflux-v1',aspect:'1:1',count:1,seed:'',capabilityId:'image.text-to-image.z-image-turbo.v1',sourceAssetId:null});
    expect(state.drafts[0].params).toEqual({modelId:'z-image-turbo-mflux-v1',aspect:'3:4',count:1,seed:'73',capabilityId:'image.text-to-image.z-image-turbo.v1',sourceAssetId:null});
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
  it('converges an incompatible aspect when selecting a 1:1-only model',()=>{
    let state=reduce(initialWorkspace(),{type:'params',patch:{aspect:'4:3'}});
    state=reduce(state,{type:'params',patch:{modelId:'qwen-image-2.1-mflux-v1',capabilityId:'image.text-to-image.qwen-image-2.1.v1'}});
    expect(state.drafts[0].params.aspect).toBe('1:1');
    expect(state.drafts[0].prompt).toBe('');
    state=reduce(state,{type:'params',patch:{aspect:'1:1'}});
    expect(state.drafts[0].params.aspect).toBe('1:1');
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

describe('conversation deletion state',()=>{
  it('removes only the confirmed draft and selects the host successor',()=>{
    const state = workspaceReducer({...initialWorkspace(),drafts:[createDraft(1,'A'),createDraft(2,'B'),createDraft(3,'C')],activeId:2}, {type:'remove',draftId:2,successorId:3});
    expect(state.drafts.map(draft=>draft.id)).toEqual([1,3]);
    expect(state.activeId).toBe(3);
    expect(state.drafts.map(draft=>draft.name)).toEqual(['A','C']);
  });
  it('keeps other drafts untouched when the deleted target was not active',()=>{
    const before = {...initialWorkspace(),drafts:[createDraft(1,'A'),createDraft(2,'B')],activeId:1};
    const state = workspaceReducer(before, {type:'remove',draftId:2,successorId:1});
    expect(state.activeId).toBe(1);
    expect(state.drafts).toHaveLength(1);
    expect(state.drafts[0]).toBe(before.drafts[0]);
  });
});

describe('capability switches produce host-valid draft slots',()=>{
  const zImageCapability = 'image.text-to-image.z-image-turbo.v1';
  it('converges a 4:3 draft to 1:1 for a 1:1-only model and keeps every stored slot valid',()=>{
    let state = workspaceReducer(initialWorkspace(), {type:'params',patch:{modelId:Z_IMAGE_MODEL_ID,capabilityId:zImageCapability,aspect:'4:3',seed:'20261031'}});
    expect(state.drafts[0].params.aspect).toBe('4:3');
    state = workspaceReducer(state, {type:'params',patch:{modelId:KLEIN_MODEL_ID,capabilityId:KLEIN_TEXT_CAPABILITY}});
    expect(state.drafts[0].params.aspect).toBe('1:1');
    state = workspaceReducer(state, {type:'params',patch:{modelId:KLEIN_MODEL_ID,capabilityId:KLEIN_EDIT_CAPABILITY,sourceAssetId:'asset-00000000000000000000000000000000',aspect:'1:1'}});
    expect(state.drafts[0].params.aspect).toBe('1:1');
    state = workspaceReducer(state, {type:'params',patch:{modelId:Z_IMAGE_MODEL_ID,capabilityId:zImageCapability}});
    const slots = state.drafts[0].params.capabilityDrafts ?? {};
    for (const [capability, slot] of Object.entries(slots)) expect(aspectsForCapability(capability)).toContain(slot.aspect);
    expect(slots[KLEIN_EDIT_CAPABILITY]?.aspect).toBe('1:1');
    expect(state.drafts[0].params.aspect).toBe('4:3');
    expect(slots[zImageCapability]?.aspect).toBe('4:3');
  });
  it('keeps a supported same-capability aspect untouched',()=>{
    const state = workspaceReducer(initialWorkspace(), {type:'params',patch:{modelId:Z_IMAGE_MODEL_ID,capabilityId:zImageCapability,aspect:'3:4'}});
    expect(state.drafts[0].params.aspect).toBe('3:4');
  });
  it('repairs legacy invalid params and slots when a draft is loaded',()=>{
    const legacy: DraftParams = {
      modelId: KLEIN_MODEL_ID,
      capabilityId: KLEIN_TEXT_CAPABILITY,
      aspect: '4:3',
      count: 1,
      seed: '7',
      sourceAssetId: null,
      capabilityDrafts: {
        [KLEIN_TEXT_CAPABILITY]: {prompt:'',aspect:'4:3',seed:'7',sourceAssetId:null,promptSourceId:null},
        [zImageCapability]: {prompt:'',aspect:'3:4',seed:'',sourceAssetId:null,promptSourceId:null},
      },
    };
    const normalized = normalizeDraftParams(legacy);
    expect(normalized.aspect).toBe('1:1');
    expect(normalized.capabilityDrafts?.[KLEIN_TEXT_CAPABILITY]?.aspect).toBe('1:1');
    expect(normalized.capabilityDrafts?.[zImageCapability]?.aspect).toBe('3:4');
  });
  it('clears the outgoing slot source when the user explicitly removes it',()=>{
    let state = workspaceReducer(initialWorkspace(), {type:'params',patch:{modelId:KLEIN_MODEL_ID,capabilityId:KLEIN_EDIT_CAPABILITY,sourceAssetId:'asset-A',aspect:'1:1'}});
    expect(state.drafts[0].params.sourceAssetId).toBe('asset-A');
    state = workspaceReducer(state, {type:'params',patch:{sourceAssetId:null,capabilityId:KLEIN_TEXT_CAPABILITY}});
    expect(state.drafts[0].params.sourceAssetId).toBeNull();
    expect(state.drafts[0].params.capabilityDrafts?.[KLEIN_EDIT_CAPABILITY]?.sourceAssetId).toBeNull();
    expect(JSON.stringify(state.drafts[0].params)).not.toContain('asset-A');
  });
  it('keeps the outgoing slot source on a plain model switch',()=>{
    let state = workspaceReducer(initialWorkspace(), {type:'params',patch:{modelId:KLEIN_MODEL_ID,capabilityId:KLEIN_EDIT_CAPABILITY,sourceAssetId:'asset-A',aspect:'1:1'}});
    state = workspaceReducer(state, {type:'params',patch:{modelId:Z_IMAGE_MODEL_ID,capabilityId:zImageCapability}});
    expect(state.drafts[0].params.sourceAssetId).toBeNull();
    expect(state.drafts[0].params.capabilityDrafts?.[KLEIN_EDIT_CAPABILITY]?.sourceAssetId).toBe('asset-A');
  });
  it('keeps unrelated slots untouched when a new source is selected',()=>{
    let state = workspaceReducer(initialWorkspace(), {type:'params',patch:{modelId:KLEIN_MODEL_ID,capabilityId:KLEIN_EDIT_CAPABILITY,sourceAssetId:'asset-B',aspect:'1:1'}});
    state = workspaceReducer(state, {type:'params',patch:{modelId:BIREFNET_MODEL_ID,capabilityId:BIREFNET_CAPABILITY}});
    const slots = state.drafts[0].params.capabilityDrafts ?? {};
    expect(slots[KLEIN_EDIT_CAPABILITY]?.sourceAssetId).toBe('asset-B');
    expect(slots[zImageCapability]?.sourceAssetId ?? null).toBeNull();
    expect(state.drafts[0].params.sourceAssetId).toBeNull();
  });
});
