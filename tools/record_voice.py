"""Records the game's voice callouts with Piper.

Usage: python record_voice.py <voice.onnx> <output dir>

The file names match the `CLIPS` table in src/voice.rs.
"""

import os
import sys
import tempfile
import wave
from pathlib import Path

import piper
from piper import PiperVoice, SynthesisConfig

LINES = {
    "mission_start": "Mission start. Good hunting.",
    "power_up": "Weapons upgraded.",
    "max_power": "Maximum firepower.",
    "shield": "Shield online.",
    "repair": "Hull repaired.",
    "bomb_ready": "Bomb ready.",
    "bomb_away": "Bomb away!",
    "hull_critical": "Warning. Hull integrity critical.",
    "boss_warning": "Warning. Large hostile approaching.",
    "core_exposed": "Core exposed. Fire at will.",
    "mission_complete": "Target destroyed. Mission complete.",
    "mission_failed": "Mission failed.",
    "rescued": "Survivor rescued.",
}


def main() -> None:
    model, out = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
    out.mkdir(parents=True, exist_ok=True)
    # espeak-ng (inside Piper) can't handle a long path to its data folder
    # and silently falls back to a path that doesn't exist, so hand it a
    # short relative one through a symlink.
    data = Path(piper.__file__).parent / "espeak-ng-data"
    with tempfile.TemporaryDirectory() as tmp:
        os.chdir(tmp)
        os.symlink(data, "e")
        voice = PiperVoice.load(model, espeak_data_dir="e")
        config = SynthesisConfig(length_scale=0.9)
        for name, text in LINES.items():
            with wave.open(str(out / f"{name}.wav"), "wb") as wav:
                voice.synthesize_wav(text, wav, syn_config=config)
            print(f"{name}: {text}")


if __name__ == "__main__":
    main()
