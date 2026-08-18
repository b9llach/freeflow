# Loaded via CMAKE_PROJECT_INCLUDE_BEFORE from toolchain-windows.cmake.
# This file runs AT THE TOP OF EVERY project() call, i.e. AFTER the
# toolchain has already set the cache but potentially BEFORE any
# option() call that would clobber it under CMP0077-OLD.
#
# The reason we can't rely on the toolchain's cache FORCE alone: whisper.cpp
# 1.7.x's outer CMakeLists sets `cmake_minimum_required(VERSION 3.5)`,
# which defaults CMP0077 to OLD, which makes `option()` unconditionally
# re-force its default value into the cache. Setting the variable here
# (again, with FORCE) — after project() but before the option() calls in
# the CMakeLists — outmaneuvers that.
#
# The `_INIT` compile-flag variants are also re-applied here so they beat
# cmake-rs's `-DCMAKE_C_FLAGS=...` overrides.

message(STATUS "===== FREEFLOW PROJECT-INCLUDE RUNNING =====")

# --- whisper.cpp / ggml SIMD dispatch ---------------------------------
set(GGML_NATIVE       OFF CACHE BOOL "" FORCE)
set(GGML_AVX          ON  CACHE BOOL "" FORCE)
set(GGML_AVX2         ON  CACHE BOOL "" FORCE)
set(GGML_FMA          ON  CACHE BOOL "" FORCE)
set(GGML_F16C         ON  CACHE BOOL "" FORCE)
set(GGML_AVX512       OFF CACHE BOOL "" FORCE)
set(GGML_AVX512_VBMI  OFF CACHE BOOL "" FORCE)
set(GGML_AVX512_VNNI  OFF CACHE BOOL "" FORCE)
set(GGML_AVX512_BF16  OFF CACHE BOOL "" FORCE)
set(GGML_AMX          OFF CACHE BOOL "" FORCE)
set(GGML_AMX_TILE     OFF CACHE BOOL "" FORCE)
set(GGML_AMX_INT8     OFF CACHE BOOL "" FORCE)
set(GGML_AMX_BF16     OFF CACHE BOOL "" FORCE)
set(WHISPER_NATIVE    OFF CACHE BOOL "" FORCE)
set(WHISPER_AVX512    OFF CACHE BOOL "" FORCE)

# --- MSVC runtime + arch cap (redundant with toolchain but sticky) ----
if(MSVC)
    set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded" CACHE STRING "" FORCE)
    # add_compile_options adds these AFTER CMAKE_C_FLAGS is fully set, so
    # neither cmake-rs's -DCMAKE_C_FLAGS= nor whisper.cpp's own ARCH_FLAGS
    # can strip them. /arch:AVX2 caps MSVC codegen — no AVX-512 through
    # any path.
    add_compile_options(/arch:AVX2)
endif()

message(STATUS "  GGML_NATIVE now -> ${GGML_NATIVE}")
message(STATUS "  GGML_AVX now    -> ${GGML_AVX}")
message(STATUS "  GGML_AVX2 now   -> ${GGML_AVX2}")
message(STATUS "  GGML_AVX512 now -> ${GGML_AVX512}")
