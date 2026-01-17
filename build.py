"""Master build script for FreeFlow - builds both Python and Electron."""

import subprocess
import sys
import shutil
from pathlib import Path
import argparse


def run_command(cmd, cwd=None, description=None):
    """Run a command and handle errors."""
    if description:
        print(f"\n{description}...")

    result = subprocess.run(cmd, cwd=cwd, shell=isinstance(cmd, str))

    if result.returncode != 0:
        print(f"Command failed with code {result.returncode}")
        sys.exit(1)

    return result


def build_python(project_root):
    """Build the Python API with PyInstaller."""
    print("\n" + "=" * 50)
    print("Building Python API")
    print("=" * 50)

    # Run the Python build script
    run_command(
        [sys.executable, "build_python.py"],
        cwd=project_root,
        description="Running PyInstaller"
    )

    # Verify output exists
    dist_dir = project_root / "dist" / "freeflow-api"
    if not dist_dir.exists():
        print("Error: Python build output not found!")
        sys.exit(1)

    print("Python build complete!")


def build_electron(project_root, platform=None):
    """Build the Electron app."""
    print("\n" + "=" * 50)
    print("Building Electron App")
    print("=" * 50)

    electron_dir = project_root / "electron"

    # Install dependencies if needed
    if not (electron_dir / "node_modules").exists():
        run_command(
            ["npm", "install"],
            cwd=electron_dir,
            description="Installing npm dependencies"
        )

    # Determine build command
    if platform:
        build_cmd = f"npm run build:{platform}"
    else:
        build_cmd = "npm run build"

    run_command(
        build_cmd,
        cwd=electron_dir,
        description="Running electron-builder"
    )

    print("Electron build complete!")


def main():
    parser = argparse.ArgumentParser(description="Build FreeFlow application")
    parser.add_argument(
        "--python-only",
        action="store_true",
        help="Only build the Python API"
    )
    parser.add_argument(
        "--electron-only",
        action="store_true",
        help="Only build the Electron app (requires Python build to exist)"
    )
    parser.add_argument(
        "--platform",
        choices=["win", "mac", "linux"],
        help="Target platform for Electron build"
    )
    args = parser.parse_args()

    project_root = Path(__file__).parent

    print("=" * 50)
    print("FreeFlow Build Script")
    print("=" * 50)

    if args.electron_only:
        # Check that Python build exists
        if not (project_root / "dist" / "freeflow-api").exists():
            print("Error: Python build not found. Run without --electron-only first.")
            sys.exit(1)
        build_electron(project_root, args.platform)
    elif args.python_only:
        build_python(project_root)
    else:
        # Build both
        build_python(project_root)
        build_electron(project_root, args.platform)

    print("\n" + "=" * 50)
    print("Build complete!")
    print("=" * 50)

    # Show output location
    electron_dist = project_root / "electron" / "dist"
    if electron_dist.exists():
        print(f"\nOutput: {electron_dist}")
        for item in electron_dist.iterdir():
            if item.is_file() and item.suffix in ['.exe', '.dmg', '.AppImage']:
                size_mb = item.stat().st_size / (1024 * 1024)
                print(f"  {item.name} ({size_mb:.1f} MB)")


if __name__ == "__main__":
    main()
