// Copies the native runtime DLLs that sherpa-rs's `download-binaries`
// feature drops in target/release/ (Windows only) into a stable staging
// directory that tauri.conf.json's `bundle.resources` map points at.
//
// This runs as `beforeBundleCommand`, so cargo has already produced the
// DLLs by the time we get here, and tauri's bundler will package the
// staging directory into the installer immediately after.
//
// No-op on non-Windows: sherpa-rs uses libsherpa-onnx.dylib / .so and the
// dynamic loader finds them via rpath / DYLD_LIBRARY_PATH conventions on
// those platforms.

const fs = require("node:fs");
const path = require("node:path");

if (process.platform !== "win32") {
  process.exit(0);
}

// Native DLLs that sherpa-rs's download-binaries feature drops in
// target/release/ (Parakeet backend + ONNX Runtime).
const SHERPA_FILES = [
  "sherpa-onnx-c-api.dll",
  "sherpa-onnx-cxx-api.dll",
  "onnxruntime.dll",
  "onnxruntime_providers_shared.dll",
  "cargs.dll",
];

// Visual C++ 2015-2022 Redistributable runtime DLLs. These are imports of
// freeflow2.exe (via whisper.cpp's C++ stdlib usage) and of onnxruntime.dll.
// Even with `+crt-static` on Rust, the C++ standard library (msvcp140) still
// links dynamically, and onnxruntime is a prebuilt DLL we don't control at
// all. Bundling these next to the exe removes the "install VC++ Redist
// first" prerequisite for end users.
const VC_RUNTIME_FILES = [
  "msvcp140.dll",
  "vcruntime140.dll",
  "vcruntime140_1.dll",
];

const root = path.resolve(__dirname, "..");
const cargoOut = path.join(root, "src-tauri", "target", "release");
const dstDir = path.join(root, "src-tauri", "runtime-dlls");

if (!fs.existsSync(cargoOut)) {
  console.error(
    `[copy-runtime-dlls] cargo release output not found at ${cargoOut}. ` +
      `Did the cargo build step run yet?`
  );
  process.exit(1);
}

fs.mkdirSync(dstDir, { recursive: true });

let missing = 0;

function copy(name, src) {
  const dst = path.join(dstDir, name);
  if (!fs.existsSync(src)) {
    console.warn(`[copy-runtime-dlls] MISSING: ${src}`);
    missing++;
    return;
  }
  fs.copyFileSync(src, dst);
  const size = fs.statSync(dst).size;
  console.log(
    `[copy-runtime-dlls] ${name.padEnd(36)} ${(size / 1024 / 1024).toFixed(2)} MB`
  );
}

// 1. Sherpa / ONNX runtime DLLs from cargo output.
for (const name of SHERPA_FILES) {
  copy(name, path.join(cargoOut, name));
}

// 2. VC++ Redistributable runtime DLLs. Prefer the VS-shipped redistributable
// tree (its versions match the compiler that built onnxruntime and our exe);
// fall back to System32 which every Windows dev box has.
function findVcRedistDir() {
  const candidates = [];
  const vsRoots = [
    process.env["ProgramFiles"] &&
      path.join(process.env["ProgramFiles"], "Microsoft Visual Studio"),
    process.env["ProgramFiles(x86)"] &&
      path.join(process.env["ProgramFiles(x86)"], "Microsoft Visual Studio"),
  ].filter(Boolean);
  for (const vsRoot of vsRoots) {
    if (!fs.existsSync(vsRoot)) continue;
    for (const year of fs.readdirSync(vsRoot).sort().reverse()) {
      // 2022, 2019, ...
      for (const edition of ["BuildTools", "Community", "Professional", "Enterprise", "Preview"]) {
        const redist = path.join(
          vsRoot, year, edition, "VC", "Redist", "MSVC"
        );
        if (fs.existsSync(redist)) {
          const versions = fs.readdirSync(redist).sort().reverse();
          for (const v of versions) {
            const crtDir = path.join(redist, v, "x64", "Microsoft.VC143.CRT");
            if (fs.existsSync(crtDir)) candidates.push(crtDir);
            const crtDir142 = path.join(redist, v, "x64", "Microsoft.VC142.CRT");
            if (fs.existsSync(crtDir142)) candidates.push(crtDir142);
          }
        }
      }
    }
  }
  return candidates[0];
}

const vcRedistDir = findVcRedistDir();
const system32 = path.join(process.env["WINDIR"] || "C:\\Windows", "System32");

for (const name of VC_RUNTIME_FILES) {
  let src = vcRedistDir && path.join(vcRedistDir, name);
  if (!src || !fs.existsSync(src)) {
    src = path.join(system32, name);
  }
  copy(name, src);
}

if (missing > 0) {
  console.error(
    `[copy-runtime-dlls] ${missing} runtime DLL(s) missing. The installed ` +
      `app will fail to start on any machine without them on PATH.`
  );
  process.exit(1);
}
