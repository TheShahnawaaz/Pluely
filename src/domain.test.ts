import { describe,it,expect } from 'vitest';
import { applyTranscript,formatDuration,searchSessions,sessionMarkdown,validateEndpoint } from './domain';
import type { Session } from './types';
const session:Session={id:'one',title:'Roadmap',startedAt:'2026-11-11T12:00:00Z',endedAt:null,status:'recording',transcript:[],messages:[],notes:'Ship the Windows beta on Friday.',modeId:'general',elapsedMs:65000,lastError:null};
describe('meeting integrity',()=>{
 it('retains finalized text against a stale partial',()=>{const a=applyTranscript(session,{id:'t',role:'me',text:'Agreed.',atMs:1,final:true});expect(applyTranscript(a,{id:'t',role:'me',text:'Ag',atMs:1,final:false}).transcript[0].text).toBe('Agreed.')});
 it('orders out-of-order transcription completions by source time',()=>{let a=applyTranscript(session,{id:'later',role:'them',text:'Later',atMs:3000,final:true});a=applyTranscript(a,{id:'first',role:'me',text:'First',atMs:1000,final:true});expect(a.transcript.map(t=>t.id)).toEqual(['first','later'])});
 it('does not append a transcript to a completed session',()=>expect(applyTranscript({...session,status:'ready'},{id:'t',role:'me',text:'late',atMs:1,final:true}).transcript).toHaveLength(0));
 it('searches actual notes and requires every search term',()=>{expect(searchSessions([session],'windows friday')).toHaveLength(1);expect(searchSessions([session],'windows monday')).toHaveLength(0)});
 it('exports only finalized transcript entries',()=>{const text=sessionMarkdown({...session,transcript:[{id:'a',role:'me',text:'Final',atMs:0,final:true},{id:'b',role:'them',text:'Partial secret',atMs:3,final:false}]});expect(text).toContain('You: Final');expect(text).not.toContain('Partial secret')});
 it('formats durations without negative time',()=>{expect(formatDuration(65000)).toBe('01:05');expect(formatDuration(-1)).toBe('00:00')});
});
describe('provider boundary',()=>{
 it.each(['http://example.com/v1','file:///private/file','https://user:token@example.com/v1','https://example.com/v1?api_key=secret'])('rejects unsafe endpoint %s',url=>expect(()=>validateEndpoint(url)).toThrow());
 it('permits explicit local-model endpoints',()=>expect(validateEndpoint('http://localhost:11434/v1/')).toBe('http://localhost:11434/v1'));
});
