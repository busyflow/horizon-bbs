#!/usr/bin/env python3
"""Produce a deterministic, source-only launcher/registry native mod bundle (.hnmod)."""
import hashlib
import json
from pathlib import Path
import tomllib
import zipfile
import bundle

ROOT = Path(__file__).resolve().parents[1]


def package():
    if (ROOT / 'mixins/bbs/runtime.rs').read_text(encoding='utf-8') != bundle.payload():
        raise ValueError('stale runtime: run tools/bundle.py')
    compat = json.loads((ROOT / 'compatibility.json').read_text(encoding='utf-8'))
    project = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))['package']
    mixin = tomllib.loads((ROOT / 'mixins/bbs/horizon-mixin.toml').read_text(encoding='utf-8'))
    assert mixin['mod']['version'] == project['version']
    assert mixin['compatibility']['horizon_revision'] == compat['horizon_revision']
    manifest = dict(schema=1, id='bbs', version=project['version'], game='26.3',
                    horizon=compat['horizon_revision'], mixins=compat['horizon_mixins_revision'])
    files = {'horizon-native.json': (json.dumps(manifest, indent=2, sort_keys=True) + '\n').encode('utf-8')}
    for path in sorted((ROOT / 'mixins/bbs').iterdir()):
        if path.suffix in ('.rs', '.toml'):
            files[path.relative_to(ROOT).as_posix()] = path.read_bytes()
    out = ROOT / 'target' / f"bbs-{project['version']}.hnmod"
    out.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(out, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for name, content in sorted(files.items()):
            info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, content)
    print(json.dumps(dict(file=str(out), size=out.stat().st_size,
                         sha256=hashlib.sha256(out.read_bytes()).hexdigest()), indent=2))


if __name__ == '__main__':
    package()
