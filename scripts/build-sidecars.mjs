import {spawnSync} from 'node:child_process';
import {mkdirSync,copyFileSync} from 'node:fs';
const args=process.argv.slice(2);const target=args[0]||process.env.TAURI_ENV_TARGET_TRIPLE||spawnSync('rustc',['-vV'],{encoding:'utf8'}).stdout.match(/host: (.+)/)?.[1];
if(!target)throw Error('Cannot determine Rust target');
const run=(cmd,args)=>{const r=spawnSync(cmd,args,{stdio:'inherit'});if(r.status!==0)process.exit(r.status||1)};
run('cargo',['build','--release','-p','pluely-transcribe','--target',target]);mkdirSync('src-tauri/binaries',{recursive:true});
const exe=target.includes('windows')?'.exe':'';copyFileSync(`target/${target}/release/pluely-transcribe${exe}`,`src-tauri/binaries/pluely-transcribe-${target}${exe}`);
if(target.includes('apple')){run('swiftc',['-parse-as-library','-O','-target',target.startsWith('aarch64')?'arm64-apple-macosx14.2':'x86_64-apple-macosx14.2','-framework','ScreenCaptureKit','-framework','AVFoundation','native/macos/AudioCapture.swift','-o',`src-tauri/binaries/pluely-capture-${target}`])}
