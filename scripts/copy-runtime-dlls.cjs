// Stages the Visual C++ runtime DLLs into src-tauri/runtime-dlls/ so that
// tauri.conf.json's `bundle.resources` map can ship them next to
// freeflow2.exe.
//
// freeflow2.exe imports msvcp140.dll (whisper.cpp's C++ standard library
// usage). Bundling the runtime removes the "install the VC++ Redistributable
// first" prerequisite on clean Windows machines.
//
// Runs as `beforeBundleCommand`. No-op on non-Windows.

const fs = require("node:fs");
const path = require("node:path");

if (process.platform !== "win32") {
  process.exit(0);
}

const VC_RUNTIME_FILES = [
  "msvcp140.dll",
  "vcruntime140.dll",
  "vcruntime140_1.dll",
];

const root = path.resolve(__dirname, "..");
const dstDir = path.join(root, "src-tauri", "runtime-dlls");
fs.mkdirSync(dstDir, { recursive: true });

// Prefer the VS-shipped redistributable tree (versions match the compiler
// that built the exe); fall back to System32, which every Windows box has.
function findVcRedistDir() {
  const vsRoots = [
    process.env["ProgramFiles"] &&
      path.join(process.env["ProgramFiles"], "Microsoft Visual Studio"),
    process.env["ProgramFiles(x86)"] &&
      path.join(process.env["ProgramFiles(x86)"], "Microsoft Visual Studio"),
  ].filter(Boolean);

  for (const vsRoot of vsRoots) {
    if (!fs.existsSync(vsRoot)) continue;
    for (const year of fs.readdirSync(vsRoot).sort().reverse()) {
      for (const edition of ["BuildTools", "Community", "Professional", "Enterprise", "Preview"]) {
        const redist = path.join(vsRoot, year, edition, "VC", "Redist", "MSVC");
        if (!fs.existsSync(redist)) continue;
        for (const v of fs.readdirSync(redist).sort().reverse()) {
          for (const crt of ["Microsoft.VC143.CRT", "Microsoft.VC142.CRT"]) {
            const dir = path.join(redist, v, "x64", crt);
            if (fs.existsSync(dir)) return dir;
          }
        }
      }
    }
  }
  return undefined;
}

const vcRedistDir = findVcRedistDir();
const system32 = path.join(process.env["WINDIR"] || "C:\\Windows", "System32");

let missing = 0;
for (const name of VC_RUNTIME_FILES) {
  let src = vcRedistDir && path.join(vcRedistDir, name);
  if (!src || !fs.existsSync(src)) {
    src = path.join(system32, name);
  }
  if (!fs.existsSync(src)) {
    console.warn(`[copy-runtime-dlls] MISSING: ${name}`);
    missing++;
    continue;
  }
  const dst = path.join(dstDir, name);
  fs.copyFileSync(src, dst);
  const size = fs.statSync(dst).size;
  console.log(
    `[copy-runtime-dlls] ${name.padEnd(24)} ${(size / 1024 / 1024).toFixed(2)} MB`
  );
}

if (missing > 0) {
  console.error(
    `[copy-runtime-dlls] ${missing} runtime DLL(s) missing. The installed ` +
      `app will fail to start on machines without the VC++ Redistributable.`
  );
  process.exit(1);
}
