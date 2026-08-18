# CMake toolchain file used for every cmake-driven sys crate cargo builds
# (whisper-rs-sys is the important one — it compiles whisper.cpp / ggml).
#
# Two goals:
#
# 1. Cap the SIMD instruction set at AVX2.
#
#    whisper.cpp / ggml default to GGML_NATIVE=ON, which means "compile for
#    the build host's CPU." On a dev machine with AVX-512 support that bakes
#    AVX-512 instructions into the resulting binary — which then throws
#    EXCEPTION_ILLEGAL_INSTRUCTION the moment it runs on any user machine
#    without AVX-512. We force NATIVE off and pin /arch:AVX2 so the shipped
#    binary works on the AVX2 baseline (basically any Intel Haswell / AMD
#    Excavator or newer, i.e. anything sold since ~2013).
#
# 2. Static MSVC C++ runtime.
#
#    Belts and suspenders alongside the +crt-static rustflag: cmake targets
#    also link the static MultiThreaded CRT so whisper.cpp's static libs
#    are ABI-compatible with the rest of the exe.
#
# Referenced from .cargo/config.toml via
#   CMAKE_TOOLCHAIN_FILE = { value = "toolchain-windows.cmake", relative = true }

if(CMAKE_HOST_WIN32)
    # Static release CRT (matches Rust's +crt-static). Cache + FORCE so
    # whisper.cpp's own CMakeLists can't overwrite it.
    set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded" CACHE STRING "" FORCE)

    # Cap MSVC codegen at AVX2. /arch:AVX2 implies AVX + FMA + F16C + BMI1/2
    # for MSVC and disables AVX-512. INIT variants apply BEFORE the project's
    # own CFLAGS/CXXFLAGS so we don't need to fight per-project overrides.
    if(NOT CMAKE_C_FLAGS_INIT MATCHES "/arch:")
        set(CMAKE_C_FLAGS_INIT "${CMAKE_C_FLAGS_INIT} /arch:AVX2")
    endif()
    if(NOT CMAKE_CXX_FLAGS_INIT MATCHES "/arch:")
        set(CMAKE_CXX_FLAGS_INIT "${CMAKE_CXX_FLAGS_INIT} /arch:AVX2")
    endif()
endif()

# whisper.cpp / ggml specific knobs. Naming has drifted between versions
# (WHISPER_* → GGML_*), so we set both so at least one wins.
#
# We use `set(... CACHE ...)` WITHOUT `FORCE` on the initial declaration.
# whisper.cpp's outer CMakeLists uses `cmake_minimum_required(VERSION 3.5)`
# which sets CMP0077 policy to OLD → its `option()` calls FORCE-overwrite
# any pre-existing cache. To beat that we ALSO override via `-D` on the
# cmake command line via CMAKE_PROJECT_INCLUDE_BEFORE below, which runs
# after `project()` and can safely re-cache with FORCE.
set(GGML_NATIVE       OFF CACHE BOOL "" FORCE)
set(GGML_AVX512       OFF CACHE BOOL "" FORCE)
set(GGML_AVX512_VBMI  OFF CACHE BOOL "" FORCE)
set(GGML_AVX512_VNNI  OFF CACHE BOOL "" FORCE)
set(GGML_AVX512_BF16  OFF CACHE BOOL "" FORCE)
set(WHISPER_NATIVE    OFF CACHE BOOL "" FORCE)
set(WHISPER_AVX512    OFF CACHE BOOL "" FORCE)

# AVX2 baseline stays on — the shipped binary requires it, and we've
# scoped it via /arch:AVX2 above.
set(GGML_AVX          ON  CACHE BOOL "" FORCE)
set(GGML_AVX2         ON  CACHE BOOL "" FORCE)
set(GGML_FMA          ON  CACHE BOOL "" FORCE)
set(GGML_F16C         ON  CACHE BOOL "" FORCE)

# Point CMAKE_PROJECT_INCLUDE_BEFORE at a helper that re-forces the
# variables AFTER project() runs (which is when whisper.cpp's option()
# calls fire and would otherwise re-default them under CMP0077-OLD).
set(CMAKE_PROJECT_INCLUDE_BEFORE
    "${CMAKE_CURRENT_LIST_DIR}/toolchain-windows-project-include.cmake")

message(STATUS "===== FREEFLOW TOOLCHAIN LOADED =====")
message(STATUS "  GGML_NATIVE forced -> ${GGML_NATIVE}")
message(STATUS "  GGML_AVX forced    -> ${GGML_AVX}")
message(STATUS "  GGML_AVX512 forced -> ${GGML_AVX512}")
message(STATUS "  CMAKE_PROJECT_INCLUDE_BEFORE -> ${CMAKE_PROJECT_INCLUDE_BEFORE}")
