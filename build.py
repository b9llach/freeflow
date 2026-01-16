"""Build script to create FreeFlow executable using PyInstaller."""

import subprocess
import sys


def build():
    """Build the executable."""
    print("Building FreeFlow executable...")
    print("This may take several minutes due to the large model dependencies.")
    print()

    cmd = [
        sys.executable,
        "-m",
        "PyInstaller",
        "--name=FreeFlow",
        "--onefile",  # Creates a folder with exe and dependencies (faster startup than onefile)
        "--windowed",  # No console window
        "--noconfirm",  # Overwrite output without asking
        "--clean",  # Clean cache before building
        # Icon (optional - uncomment if you have an icon)
        # "--icon=icon.ico",
        # Hidden imports that PyInstaller might miss 
        "--hidden-import=nemo",
        "--hidden-import=nemo.collections",
        "--hidden-import=nemo.collections.asr",
        "--hidden-import=nemo.collections.asr.models",
        "--hidden-import=sounddevice",
        "--hidden-import=scipy.io.wavfile",
        "--hidden-import=pynput.keyboard._win32",
        "--hidden-import=pynput.mouse._win32",
        "--hidden-import=pyperclip",
        # Collect all data for NeMo
        "--collect-all=nemo",
        "--collect-all=nemo_toolkit",
        "--collect-all=pytorch_lightning",
        "--collect-all=omegaconf",
        "--collect-all=hydra",
        # Entry point
        "main.py",
    ]

    try:
        subprocess.run(cmd, check=True)
        print()
        print("=" * 50)
        print("Build complete!")
        print("Executable location: dist/FreeFlow/FreeFlow.exe")
        print("=" * 50)
    except subprocess.CalledProcessError as e:
        print(f"Build failed with error: {e}")
        sys.exit(1)
    except FileNotFoundError:
        print("PyInstaller not found. Install it with:")
        print("  pip install pyinstaller")
        sys.exit(1)


if __name__ == "__main__":
    build()
