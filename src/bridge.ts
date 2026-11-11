import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AppSnapshot, Capabilities, ModelInfo, ProviderStatus } from './types';
import { initialSnapshot } from './types';
let preview=structuredClone(initialSnapshot);
const keys=new Set<string>();
export const native=isTauri();
export async function call<T>(command:string,args:Record<string,unknown>={}):Promise<T> {
 if(native) return invoke<T>(command,args);
 switch(command){
  case 'get_storage_status':return {configured:preview.settings.onboardingComplete,locked:false,protection:preview.settings.protection} as T;
  case 'get_snapshot': return structuredClone(preview) as T;
  case 'get_capabilities':return {native:false,platform:'Browser preview',microphone:false,systemAudio:false,screen:false,captureExclusion:false,localStt:false,keyStore:false,explanation:'UI preview. Recording, provider calls, secure credentials and file persistence run in the Windows/macOS desktop app.'} satisfies Capabilities as T;
  case 'provider_status': return {llm:keys.has('llm'),groq:keys.has('groq'),elevenlabs:keys.has('elevenlabs')} satisfies ProviderStatus as T;
  case 'save_settings': preview={...preview,settings:args.settings as AppSnapshot['settings'],revision:preview.revision+1}; return structuredClone(preview) as T;
  case 'save_mode': {const mode=args.mode as AppSnapshot['modes'][number];preview={...preview,modes:[...preview.modes.filter(m=>m.id!==mode.id),mode],revision:preview.revision+1};return structuredClone(preview) as T;}
  case 'select_mode':preview={...preview,selectedModeId:String(args.id),revision:preview.revision+1};return structuredClone(preview) as T;
  case 'delete_mode':preview={...preview,modes:preview.modes.filter(m=>m.id!==args.id),selectedModeId:preview.selectedModeId===args.id?'general':preview.selectedModeId,revision:preview.revision+1};return structuredClone(preview) as T;
  case 'update_session': {const session=args.session as AppSnapshot['sessions'][number];preview={...preview,sessions:preview.sessions.map(s=>s.id===session.id?session:s),revision:preview.revision+1};return structuredClone(preview) as T;}
  case 'delete_session':preview={...preview,sessions:preview.sessions.filter(s=>s.id!==args.id),revision:preview.revision+1};return structuredClone(preview) as T;
  case 'list_local_models':return [{id:'base',name:'Whisper Base',bytes:147951465,downloaded:false,description:'Multilingual · lightweight CPU option'},{id:'small',name:'Whisper Small',bytes:487601967,downloaded:false,description:'Multilingual · higher accuracy, more memory'}] satisfies ModelInfo[] as T;
  case 'load_example': {const now=new Date().toISOString();preview={...preview,sessions:[{id:'example',title:'Example · Product planning',startedAt:now,endedAt:now,status:'ready',elapsedMs:124000,modeId:'team',lastError:null,messages:[],notes:'## Summary\nThe team reviewed the first release of Pluely.\n\n## Decisions\n- Support macOS and Windows.\n- Let users choose their own AI and transcription providers.\n\n## Action items\n- Validate microphone and system audio capture on both platforms.\n\n*This is clearly labeled sample data for exploring the interface.*',transcript:[{id:'t1',role:'me',atMs:0,text:'Let’s keep the first release local-first, with no account required.',final:true},{id:'t2',role:'them',atMs:18000,text:'Agreed. We should offer both local transcription and an API key option.',final:true}]}],revision:preview.revision+1};return structuredClone(preview) as T;}
  default:throw new Error('This action requires the native macOS or Windows app. The browser preview does not record, send audio, or persist credentials.');
 }
}
export async function onSnapshot(handler:(s:AppSnapshot)=>void):Promise<()=>void> {
 if(!native) return ()=>{};
 return listen<AppSnapshot>('pluely:snapshot',event=>handler(event.payload));
}

export async function onNativeEvent<T>(name:string,handler:(payload:T)=>void):Promise<()=>void>{if(!native)return ()=>{};return listen<T>(name,e=>handler(e.payload));}
