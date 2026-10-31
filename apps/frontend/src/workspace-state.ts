export type DraftParams = {
  modelId: string;
  aspect: string;
  count: number;
  seed: string;
  capabilityId?: string;
  sourceAssetId?: string | null;
};
export type Draft = {
  id: number; name: string; prompt: string; collapsed: boolean; params: DraftParams;
  promptSourceId?: string | null;
  past: string[]; future: string[];
};
export type WorkspaceState = { drafts: Draft[]; activeId: number; sample: number | null; nextId: number };
export const DEFAULT_MODEL_ID = 'z-image-turbo-mflux-v1';
export const createDraft = (id: number, name = '新会话'): Draft => ({id, name, prompt: '', collapsed: false, promptSourceId:null, params: {modelId:DEFAULT_MODEL_ID, aspect:'1:1', count:1, seed:'', capabilityId:'image.text-to-image.z-image-turbo.v1', sourceAssetId:null}, past:[], future:[]});
export const initialWorkspace = (): WorkspaceState => ({drafts:[createDraft(1)], activeId:1, sample:null, nextId:2});
export const validSeed = (seed: string): boolean => seed === '' || (/^\d+$/.test(seed) && Number(seed) <= 4294967295);
export const clampZoom = (zoom: number): number => Math.min(4, Math.max(.25, Math.round(zoom * 4) / 4));
export const confirmedDraft = (draft:Draft, composing:ReadonlyMap<number,string>):Draft => composing.has(draft.id)?{...draft,prompt:composing.get(draft.id)!}:draft;
export type WorkspaceAction =
  | {type:'new'} | {type:'select'; id:number} | {type:'sample'; index:number | null}
  | {type:'restore'; drafts:Draft[]; activeId:number} | {type:'append'; draft:Draft}
  | {type:'consume'; draftId:number; name:string; params:DraftParams}
  | {type:'applyPrompt'; id:number; value:string; sourceId:string; expected:string}
  | {type:'rename'; draftId:number; name:string}
  | {type:'edit'; value:string; record?:boolean; before?:string; id?:number}
  | {type:'undo'} | {type:'redo'} | {type:'collapse'}
  | {type:'params'; patch:Partial<DraftParams>; id?:number};
export function workspaceReducer(state: WorkspaceState, action: WorkspaceAction): WorkspaceState {
  if(action.type==='restore') return action.drafts.length ? {drafts:action.drafts,activeId:action.drafts.some(d=>d.id===action.activeId)?action.activeId:action.drafts[0].id,sample:null,nextId:Math.max(...action.drafts.map(d=>d.id))+1}:state;
  if(action.type==='append') return {...state,drafts:[...state.drafts,action.draft],activeId:action.draft.id,sample:null,nextId:Math.max(state.nextId,action.draft.id+1)};
  if(action.type==='consume') return {...state,drafts:state.drafts.map(draft=>draft.id===action.draftId?{...draft,name:action.name,prompt:'',promptSourceId:null,params:action.params,past:[],future:[]}:draft)};
  if(action.type==='rename') return {...state,drafts:state.drafts.map(draft=>draft.id===action.draftId?{...draft,name:action.name}:draft)};
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
      const params={...d.params,...action.patch};
      if(!/^[A-Za-z0-9._:-]{1,128}$/.test(params.modelId)||!['1:1','4:3','3:4'].includes(params.aspect)||params.count!==1) return d;
      return {...d,params};
    }
    return d;
  })};
}
