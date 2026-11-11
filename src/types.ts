import settingsSeed from '../shared/default-settings.json';
import modesSeed from '../shared/default-modes.json';
export type Speaker = 'me' | 'them';
export type Screen = 'home' | 'sessions' | 'modes' | 'settings';
export type HistoryProtection = 'unconfigured' | 'device' | 'passphrase' | 'none';
export interface Settings {
  onboardingComplete: boolean;
  llm: {baseUrl: string; model: string; vision: boolean; temperature: number};
  stt: {provider: 'groq' | 'elevenlabs' | 'local'; model: string; language: string; localModel: string};
  theme: 'light' | 'dark' | 'system';
  captureExcluded: boolean;
  screenContext: boolean;
  noFocus: boolean;
  startAtLogin: boolean;
  protection: HistoryProtection;
  shortcuts: {visibility: string; assist: string; recording: string; clear: string};
}
export interface Transcript {id: string; role: Speaker; text: string; atMs: number; final: boolean}
export interface Message {id: string; role: 'user' | 'assistant'; text: string; createdAt: string; pending?: boolean}
export interface Session {
  id: string; title: string; startedAt: string; endedAt: string | null;
  status: 'recording' | 'paused' | 'processing' | 'ready' | 'error';
  transcript: Transcript[]; messages: Message[]; notes: string; modeId: string;
  elapsedMs: number; lastError: string | null;
}
export interface ContextFile {id: string; name: string; content: string; bytes: number}
export interface Mode {id: string; name: string; emoji: string; prompt: string; notesPrompt: string; files: ContextFile[]}
export interface AppSnapshot {settings: Settings; sessions: Session[]; modes: Mode[]; activeSessionId: string | null; selectedModeId: string; revision: number}
export interface Capabilities {native: boolean; platform: string; microphone: boolean; systemAudio: boolean; screen: boolean; captureExclusion: boolean; localStt: boolean; keyStore: boolean; explanation: string}
export interface StorageStatus {configured: boolean; locked: boolean; protection: HistoryProtection}
export interface ProviderStatus {llm: boolean; groq: boolean; elevenlabs: boolean}
export interface ModelInfo {id: string; name: string; bytes: number; downloaded: boolean; description: string}
export interface ProgressEvent {kind: 'model' | 'notes' | 'capture'; id: string; percent?: number; message: string}
export const defaultSettings = settingsSeed as Settings;
export const initialModes = modesSeed as Mode[];
export const initialSnapshot: AppSnapshot = {settings:defaultSettings,sessions:[],modes:initialModes,activeSessionId:null,selectedModeId:'general',revision:0};
