"""Build script for packaging FreeFlow Python API with PyInstaller."""

import subprocess
import sys
import shutil
from pathlib import Path


def check_torch_version():
    """Check if PyTorch is CPU-only or CUDA and warn about size."""
    try:
        import torch
        cuda_available = torch.cuda.is_available()
        torch_version = torch.__version__

        print(f"PyTorch version: {torch_version}")

        if '+cu' in torch_version or cuda_available:
            print()
            print("WARNING: CUDA-enabled PyTorch detected!")
            print("This will result in a much larger bundle (~2GB+).")
            print()
            print("For a smaller CPU-only build, run:")
            print("  pip uninstall torch torchaudio")
            print("  pip install torch torchaudio --index-url https://download.pytorch.org/whl/cpu")
            print()
            response = input("Continue with CUDA build? [y/N]: ").strip().lower()
            if response != 'y':
                print("Build cancelled.")
                sys.exit(0)
        else:
            print("CPU-only PyTorch detected - good for smaller bundle size.")

    except ImportError:
        print("WARNING: PyTorch not found. Build may fail.")


def main():
    project_root = Path(__file__).parent
    dist_dir = project_root / "dist"
    build_dir = project_root / "build"

    print("=" * 50)
    print("FreeFlow Python API Build Script")
    print("=" * 50)
    print()

    # Check PyTorch version
    check_torch_version()
    print()

    # Clean previous builds
    print("Cleaning previous builds...")
    if dist_dir.exists():
        shutil.rmtree(dist_dir)
    if build_dir.exists():
        shutil.rmtree(build_dir)

    # Run PyInstaller
    print("Running PyInstaller...")
    print("This may take several minutes on first run.")
    print()

    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "PyInstaller",
            "api.spec",
            "--clean",
            "--noconfirm",
        ],
        cwd=project_root,
    )

    if result.returncode != 0:
        print()
        print("Build failed!")
        sys.exit(1)

    print()
    print("=" * 50)
    print("Build complete!")
    print(f"Output: {dist_dir / 'freeflow-api'}")
    print("=" * 50)

    # Show size
    api_dir = dist_dir / "freeflow-api"
    if api_dir.exists():
        total_size = sum(f.stat().st_size for f in api_dir.rglob("*") if f.is_file())
        size_mb = total_size / (1024 * 1024)
        print(f"Total size: {size_mb:.1f} MB")


if __name__ == "__main__":
    main()
