#!/usr/bin/env python3
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
def payload():
    adapter = (ROOT / "native/horizon.rs").read_text(encoding="utf-8")
    return adapter.replace('include!("bbs_engine.rs");', (ROOT / "src/lib.rs").read_text(encoding="utf-8")).replace('include!("bbs_tests.rs");', (ROOT / "tests/bbs_engine.rs").read_text(encoding="utf-8"))
if __name__ == "__main__":
    import sys
    path = ROOT / "mixins/bbs/runtime.rs"
    if "--check" in sys.argv:
        if path.read_text(encoding="utf-8") != payload(): raise SystemExit("run python3 tools/bundle.py to refresh the mod payload")
    else:
        path.write_text(payload(), encoding="utf-8", newline="\n")
