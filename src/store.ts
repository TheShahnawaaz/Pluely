import { create } from 'zustand';
import type { AppSnapshot,Capabilities,ProviderStatus,StorageStatus,Screen } from './types';
import { initialSnapshot } from './types';
import { call,onSnapshot,onNativeEvent } from './bridge';
interface State {snapshot:AppSnapshot;capabilities:Capabilities|null;providerStatus:ProviderStatus;storageStatus:StorageStatus;screen:Screen;selectedSession:string|null;notice:string|null;busy:boolean;booted:boolean;
 boot:()=>Promise<()=>void>;run:(cmd:string,args?:Record<string,unknown>)=>Promise<unknown>;navigate:(s:Screen)=>void;selectSession:(id:string|null)=>void;notify:(text:string|null)=>void;}
export const useApp=create<State>((set,get)=>({snapshot:structuredClone(initialSnapshot),capabilities:null,providerStatus:{llm:false,groq:false,elevenlabs:false},storageStatus:{configured:false,locked:false,protection:'unconfigured'},screen:'home',selectedSession:null,notice:null,busy:false,booted:false,
 async boot(){try {const [snapshot,capabilities,providerStatus,storageStatus]=await Promise.all([call<AppSnapshot>('get_snapshot'),call<Capabilities>('get_capabilities'),call<ProviderStatus>('provider_status'),call<StorageStatus>('get_storage_status')]);set({snapshot,capabilities,providerStatus,storageStatus,booted:true});const disposers=await Promise.all([
 onSnapshot(next=>{if(next.revision>=get().snapshot.revision)set({snapshot:next})}),
 onNativeEvent<{sessionId:string;messageId:string;delta:string;revision:number}>('pluely:delta',delta=>{const current=get().snapshot;if(delta.revision<=current.revision)return;const target=current.sessions.find(s=>s.id===delta.sessionId);if(!target?.messages.some(m=>m.id===delta.messageId))return;set({snapshot:{...current,revision:delta.revision,sessions:current.sessions.map(s=>s.id!==delta.sessionId?s:{...s,messages:s.messages.map(m=>m.id===delta.messageId?{...m,text:m.text+delta.delta}:m)})}})}),
 onNativeEvent<string>('pluely:notice',notice=>set({notice})),
 onNativeEvent<{message:string}>('pluely:progress',progress=>set({notice:progress.message})),
 ]);return ()=>disposers.forEach(fn=>fn());}catch(e){set({notice:String(e),booted:true});return ()=>{}}},
 async run(cmd,args={}) {set({busy:true});try {const result=await call<unknown>(cmd,args);if(result&&typeof result==='object'&&'sessions' in result&&'revision' in result){const snapshot=result as AppSnapshot;if(snapshot.revision>=get().snapshot.revision)set({snapshot})}if(cmd==='configure_storage'||cmd==='unlock_history')set({storageStatus:await call<StorageStatus>('get_storage_status')});if(typeof result==='string')set({notice:result});if(cmd==='save_credential'||cmd==='delete_credential')set({providerStatus:await call<ProviderStatus>('provider_status')});return result}catch(e){set({notice:e instanceof Error?e.message:String(e)});return undefined}finally{set({busy:false})}},
 navigate:screen=>set({screen,selectedSession:null}),selectSession:id=>set({screen:'sessions',selectedSession:id}),notify:notice=>set({notice}),
}));
