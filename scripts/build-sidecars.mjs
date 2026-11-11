import { spawnSync } from 'node:child_process';
import { mkdirSync, copyFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';

const target = process.argv[2] || process.env.TAURI_ENV_TARGET_TRIPLE ||
  spawnSync('rustc', ['-vV'], { encoding: 'utf8' }).stdout?.match(/host: (.+)/)?.[1];
if (!target) throw Error('Cannot determine Rust target; install Rust or pass a target triple.');

const buildEnv = { ...process.env };
const windows = target.includes('windows');
if (windows) {
  // whisper-rs-sys 0.15.0 bundles bindings generated against glibc. Their FILE
  // layout assertions fail on MSVC; removing assertions would also hide ABI bugs.
  // Generate bindings against the actual Windows SDK using installed LLVM.
  delete buildEnv.WHISPER_DONT_GENERATE_BINDINGS;
  const candidates = [buildEnv.LIBCLANG_PATH, join(buildEnv.ProgramFiles || 'C:\\Program Files', 'LLVM', 'bin')].filter(Boolean);
  const clangPath = candidates.find(path => existsSync(join(path, 'libclang.dll')) || (path.toLowerCase().endsWith('libclang.dll') && existsSync(path)));
  if (!clangPath) throw Error('Windows Whisper builds require LLVM libclang.dll. Install LLVM and set LIBCLANG_PATH to its bin directory.');
  buildEnv.LIBCLANG_PATH = clangPath;
  console.log('Windows: generating native Whisper bindings with LLVM libclang.');
}

function run(command, args) {
  const result = spawnSync(command, args, { stdio: 'inherit', env: buildEnv });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status || 1);
}

run('cargo', ['fetch', '--locked']);
if (windows) {
  // The upstream build script does not track changes to the skip-bindings flag.
  // Invalidate just this package so a restored cache cannot reuse glibc bindings.
  run('cargo', ['clean', '-p', 'whisper-rs-sys', '--release', '--target', target]);
}
run('cargo', ['build', '--locked', '--release', '-p', 'pluely-transcribe', '--target', target]);
mkdirSync('src-tauri/binaries', { recursive: true });
const exe = windows ? '.exe' : '';
copyFileSync(`target/${target}/release/pluely-transcribe${exe}`, `src-tauri/binaries/pluely-transcribe-${target}${exe}`);
if (target.includes('apple')) {
  run('swiftc', ['-parse-as-library', '-O', '-target', target.startsWith('aarch64') ? 'arm64-apple-macosx14.2' : 'x86_64-apple-macosx14.2', '-framework', 'ScreenCaptureKit', '-framework', 'AVFoundation', 'native/macos/AudioCapture.swift', '-o', `src-tauri/binaries/pluely-capture-${target}`]);
}
