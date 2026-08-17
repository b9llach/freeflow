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

const FILES = [
  "sherpa-onnx-c-api.dll",
  "sherpa-onnx-cxx-api.dll",
  "onnxruntime.dll",
  "onnxruntime_providers_shared.dll",
  "cargs.dll",
];

const root = path.resolve(__dirname, "..");
const srcDir = path.join(root, "src-tauri", "target", "release");
const dstDir = path.join(root, "src-tauri", "runtime-dlls");

if (!fs.existsSync(srcDir)) {
  console.error(
    `[copy-runtime-dlls] cargo release output not found at ${srcDir}. ` +
      `Did the cargo build step run yet?`
  );
  process.exit(1);
}

fs.mkdirSync(dstDir, { recursive: true });

let missing = 0;
for (const name of FILES) {
  const src = path.join(srcDir, name);
  const dst = path.join(dstDir, name);
  if (!fs.existsSync(src)) {
    console.warn(`[copy-runtime-dlls] MISSING: ${src}`);
    missing++;
    continue;
  }
  fs.copyFileSync(src, dst);
  const size = fs.statSync(dst).size;
  console.log(
    `[copy-runtime-dlls] ${name.padEnd(36)} ${(size / 1024 / 1024).toFixed(2)} MB`
  );
}

if (missing > 0) {
  console.error(
    `[copy-runtime-dlls] ${missing} runtime DLL(s) missing. The installed ` +
      `app will fail to start on any machine without them on PATH.`
  );
  process.exit(1);
}
