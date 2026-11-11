import { normalizeAspectForCapability, textRecipeForModel, Z_IMAGE_CAPABILITY_FALLBACK, Z_IMAGE_MODEL_ID } from './model-recipes';

export type CapabilityDraft = {
  prompt: string;
  aspect: string;
  seed: string;
  sourceAssetId: string | null;
  promptSourceId: string | null;
};
export type DraftParams = {
  modelId: string;
  aspect: string;
  count: number;
  seed: string;
  capabilityId?: string;
  sourceAssetId?: string | null;
  capabilityDrafts?: Record<string, CapabilityDraft>;
};
export type Draft = {
  id: number; name: string; prompt: string; collapsed: boolean; params: DraftParams;
  promptSourceId?: string | null;
  past: string[]; future: string[];
};
export type WorkspaceState = { drafts: Draft[]; activeId: number; sample: number | null; nextId: number };
export const DEFAULT_MODEL_ID = Z_IMAGE_MODEL_ID;
const defaultCapabilityId = (modelId: string) => textRecipeForModel(modelId)?.capabilityId ?? Z_IMAGE_CAPABILITY_FALLBACK;
export const createDraft = (id: number, name = '新会话'): Draft => ({id, name, prompt: '', collapsed: false, promptSourceId:null, params: {modelId:DEFAULT_MODEL_ID, aspect:'1:1', count:1, seed:'', capabilityId:defaultCapabilityId(DEFAULT_MODEL_ID), sourceAssetId:null}, past:[], future:[]});
export const initialWorkspace = (): WorkspaceState => ({drafts:[createDraft(1)], activeId:1, sample:null, nextId:2});
export const validSeed = (seed: string): boolean => seed === '' || (/^\d+$/.test(seed) && Number(seed) <= 4294967295);
export const clampZoom = (zoom: number): number => Math.min(4, Math.max(.25, Math.round(zoom * 4) / 4));

/** Repairs a persisted draft: capability switches must never keep an aspect the host rejects. */
export const normalizeDraftParams = (params: DraftParams): DraftParams => {
  const capability = params.capabilityId ?? defaultCapabilityId(params.modelId);
  const normalized: DraftParams = { ...params, aspect: normalizeAspectForCapability(capability, params.aspect) };
  if (params.capabilityDrafts) {
    const slots: NonNullable<DraftParams['capabilityDrafts']> = {};
    for (const [slotCapability, slot] of Object.entries(params.capabilityDrafts)) {
      slots[slotCapability] = { ...slot, aspect: normalizeAspectForCapability(slotCapability, slot.aspect) };
    }
    normalized.capabilityDrafts = slots;
  }
  return normalized;
};
export const confirmedDraft = (draft:Draft, composing:ReadonlyMap<number,string>):Draft => composing.has(draft.id)?{...draft,prompt:composing.get(draft.id)!}:draft;
export type WorkspaceAction =
  | {type:'new'} | {type:'select'; id:number} | {type:'sample'; index:number | null}
  | {type:'restore'; drafts:Draft[]; activeId:number} | {type:'append'; draft:Draft}
  | {type:'consume'; draftId:number; name:string; params:DraftParams}
  | {type:'applyPrompt'; id:number; value:string; sourceId:string; expected:string}
  | {type:'rename'; draftId:number; name:string}
  | {type:'remove'; draftId:number; successorId:number}
  | {type:'edit'; value:string; record?:boolean; before?:string; id?:number}
  | {type:'undo'} | {type:'redo'} | {type:'collapse'}
  | {type:'params'; patch:Partial<DraftParams>; id?:number};
export function workspaceReducer(state: WorkspaceState, action: WorkspaceAction): WorkspaceState {
  if(action.type==='restore') return action.drafts.length ? {drafts:action.drafts,activeId:action.drafts.some(d=>d.id===action.activeId)?action.activeId:action.drafts[0].id,sample:null,nextId:Math.max(...action.drafts.map(d=>d.id))+1}:state;
  if(action.type==='append') return {...state,drafts:[...state.drafts,action.draft],activeId:action.draft.id,sample:null,nextId:Math.max(state.nextId,action.draft.id+1)};
  if(action.type==='consume') return {...state,drafts:state.drafts.map(draft=>draft.id===action.draftId?{...draft,name:action.name,prompt:'',promptSourceId:null,params:action.params,past:[],future:[]}:draft)};
  if(action.type==='rename') return {...state,drafts:state.drafts.map(draft=>draft.id===action.draftId?{...draft,name:action.name}:draft)};
  if(action.type==='remove') return {...state,drafts:state.drafts.filter(draft=>draft.id!==action.draftId),activeId:action.successorId};
  if(action.type==='new') return {...state,drafts:[...state.drafts,createDraft(state.nextId)],activeId:state.nextId,nextId:state.nextId+1,sample:null};
  if(action.type==='select') return state.drafts.some(d=>d.id===action.id)?{...state,activeId:action.id,sample:null}:state;
  if(action.type==='sample') return {...state,sample:action.index};
  return {...state,drafts:state.drafts.map(d=>{
    if(d.id!==(action.type==='edit'||action.type==='params'||action.type==='applyPrompt'?(action.id??state.activeId):state.activeId)) return d;
    if(action.type==='applyPrompt') {
      if(d.prompt!==action.expected) return d;
      const before=d.prompt;
      return {...d,prompt:action.value,promptSourceId:action.sourceId,past:before===action.value?d.past:[...d.past,before].slice(-100),future:[]};
    }
    if(action.type==='edit') {
      const before=action.before??d.prompt;
      if(action.value===before && action.value===d.prompt) return d;
      return {...d,prompt:action.value,promptSourceId:action.value===d.prompt?d.promptSourceId:null,past:action.record===false||before===action.value?d.past:[...d.past,before].slice(-100),future:action.record===false?d.future:[]};
    }
    if(action.type==='undo') {
      if(!d.past.length) return d;
      return {...d,prompt:d.past[d.past.length-1],promptSourceId:null,past:d.past.slice(0,-1),future:[d.prompt,...d.future]};
    }
    if(action.type==='redo') {
      if(!d.future.length) return d;
      return {...d,prompt:d.future[0],promptSourceId:null,past:[...d.past,d.prompt],future:d.future.slice(1)};
    }
    if(action.type==='collapse') return {...d,collapsed:!d.collapsed};
    if(action.type==='params') {
      let params={...d.params,...action.patch};
      if(!/^[A-Za-z0-9._:-]{1,128}$/.test(params.modelId)||!['1:1','4:3','3:4'].includes(params.aspect)||params.count!==1) return d;
      const current=d.params.capabilityId??defaultCapabilityId(d.params.modelId);
      const next=params.capabilityId??defaultCapabilityId(params.modelId);
      if(current===next) {
        // Same capability: keep the chosen aspect unless the capability rejects it.
        return {...d,params:{...params,aspect:normalizeAspectForCapability(next,params.aspect)}};
      }
      // Switching capability: the stored slot and the incoming parameters must both
      // satisfy their own capability, otherwise the host rejects every later save.
      // An explicit source removal must clear the outgoing capability too, otherwise the
      // removed work stays referenced by an invisible slot and blocks its deletion.
      const outgoingSource=action.patch.sourceAssetId===null?null:d.params.sourceAssetId??null;
      const slots={...d.params.capabilityDrafts,[current]:{prompt:d.prompt,aspect:normalizeAspectForCapability(current,d.params.aspect),seed:d.params.seed,sourceAssetId:outgoingSource,promptSourceId:d.promptSourceId??null}};
      for(const [slotCapability,slot] of Object.entries(slots)) slots[slotCapability]={...slot,aspect:normalizeAspectForCapability(slotCapability,slot.aspect)};
      const saved=slots[next];
      const tool=next.includes('.remove-background.')||next.includes('.upscale.');
      params={...d.params,seed:tool?'':d.params.seed,sourceAssetId:null,...(saved?{aspect:saved.aspect,seed:saved.seed,sourceAssetId:saved.sourceAssetId}:{}),...action.patch,capabilityDrafts:slots};
      if(tool) params.aspect='1:1';
      params={...params,aspect:normalizeAspectForCapability(next,params.aspect)};
      return {...d,params,prompt:saved?.prompt??(tool?'':d.prompt),promptSourceId:saved?.promptSourceId??null,past:[],future:[]};
    }
    return d;
  })};
}
