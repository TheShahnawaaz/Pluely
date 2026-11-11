import type { Session, Transcript } from './types';
export function applyTranscript(session: Session, incoming: Transcript): Session {
  if (session.status !== 'recording' && session.status !== 'paused') return session;
  const previous=session.transcript.find(x=>x.id===incoming.id);
  if(previous?.final && !incoming.final) return session;
  const transcript=[...session.transcript.filter(x=>x.id!==incoming.id),incoming].sort((a,b)=>a.atMs-b.atMs||a.id.localeCompare(b.id));
  return {...session,transcript};
}
export function formatDuration(ms: number): string {const s=Math.max(0,Math.floor(ms/1000));return `${Math.floor(s/60).toString().padStart(2,'0')}:${(s%60).toString().padStart(2,'0')}`}
export function sessionMarkdown(s: Session): string {
  return `# ${s.title}\n\n${new Date(s.startedAt).toISOString()}\n\n${s.notes || 'No notes generated.'}\n\n## Transcript\n\n${s.transcript.filter(t=>t.final).map(t=>`[${formatDuration(t.atMs)}] ${t.role==='me'?'You':'Others'}: ${t.text}`).join('\n\n')}\n`;
}
export function searchSessions(sessions: Session[], query: string): Session[] {
 const terms=query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
 return sessions.filter(s=>{const text=[s.title,s.notes,...s.transcript.map(t=>t.text)].join('\n').toLocaleLowerCase();return terms.every(t=>text.includes(t))}).sort((a,b)=>b.startedAt.localeCompare(a.startedAt));
}
export function validateEndpoint(raw:string): string {
 const url=new URL(raw.trim());
 const local=['localhost','127.0.0.1','[::1]'].includes(url.hostname);
 if(url.username||url.password||url.search||url.hash) throw new Error('Use an endpoint URL without credentials, query parameters or fragments.');
 if(url.protocol!=='https:' && !(local&&url.protocol==='http:')) throw new Error('Remote endpoints must use HTTPS. HTTP is allowed for local models only.');
 return url.toString().replace(/\/$/,'');
}
