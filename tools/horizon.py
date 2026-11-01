#!/usr/bin/env python3
"""Stage committed Horizon Rust sources and compile the independent native BBS port."""
import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time

try:
    import fcntl
except ImportError:
    fcntl = None

ROOT = Path(__file__).resolve().parents[1]
COMPAT = json.loads((ROOT / "compatibility.json").read_text(encoding="utf-8"))
PIN = COMPAT["horizon_revision"]
MIXINS = Path(os.environ.get("HORIZON_MIXINS", ROOT.parent / "horizon-mixins")).resolve()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def stage(horizon, stages):
    if not (horizon / ".git").exists():
        raise ValueError("--horizon must be a Git checkout (including a worktree)")
    head = subprocess.check_output(["git", "-C", str(horizon), "rev-parse", "HEAD"], text=True).strip()
    if head != PIN:
        raise ValueError(f"Horizon must be pinned at {PIN}, got {head}; use a separate checkout")
    inputs = {"revision": PIN.encode(), "export-schema": b"2"}
    signature = digest(b"".join(k.encode() + b"\0" + value for k, value in sorted(inputs.items())))
    stages.mkdir(parents=True, exist_ok=True)
    out = stages / signature[:20]
    if out.exists():
        marker = json.loads((out / ".horizon-bbs.json").read_text(encoding="utf-8"))
        if marker["signature"] != signature:
            raise ValueError(f"stage marker mismatch: {out}")
        for path, expected in marker["files"].items():
            if digest((out / path).read_bytes()) != expected:
                raise ValueError(f"staged source was edited: {path}; remove this generated stage to rebuild it")
        return out, False
    temporary = Path(tempfile.mkdtemp(prefix=".bbs-stage-", dir=stages))
    try:
        process = subprocess.Popen(["git", "-C", str(horizon), "archive", "--format=tar", PIN], stdout=subprocess.PIPE)
        try:
            with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
                for member in archive:
                    archive.extract(member, temporary, filter="data")
        finally:
            process.stdout.close()
            code = process.wait()
        if code:
            raise ValueError(f"git archive exited {code}")
        for path, expected in COMPAT["source_sha256"].items():
            if digest((temporary / path).read_bytes()) != expected:
                raise ValueError(f"pinned source hash mismatch: {path}")
        for file in temporary.rglob("*"):
            if file.is_symlink():
                content = file.read_bytes()
                file.unlink()
                file.write_bytes(content)
        (temporary / ".horizon-revision").write_text(PIN + "\n", encoding="utf-8")
        files = {p.relative_to(temporary).as_posix(): digest(p.read_bytes())
                 for p in sorted(temporary.rglob("*")) if p.is_file()}
        (temporary / ".horizon-bbs.json").write_text(json.dumps(
            {"signature": signature, "horizon_revision": PIN, "files": files}, sort_keys=True, indent=2) + "\n", encoding="utf-8")
        os.rename(temporary, out)
    except BaseException:
        shutil.rmtree(temporary, ignore_errors=True)
        raise
    return out, True


def mixin_stage(horizon, stages):
    import bundle
    if (ROOT / "mixins/bbs/runtime.rs").read_text(encoding="utf-8") != bundle.payload():
        raise ValueError("stale mod payload: run python3 tools/bundle.py")
    revision = subprocess.check_output(["git", "-C", str(MIXINS), "rev-parse", "HEAD"], text=True).strip()
    if revision != COMPAT["horizon_mixins_revision"]:
        raise ValueError(f"horizon-mixins must be pinned at {COMPAT['horizon_mixins_revision']}, got {revision}")
    original, _ = stage(horizon, stages / "original")
    subprocess.run(["cargo", "build", "--locked", "--offline", "--target-dir", str(MIXINS / "target"), "-p", "horizon-mixins-cli"], cwd=MIXINS, check=True, stdout=sys.stderr)
    exe_name = "horizon-mixins.exe" if os.name == "nt" else "horizon-mixins"
    cli = MIXINS / "target/debug" / exe_name
    output = stages / "modded"
    plan = stages / "plan.json"
    base = ["--workspace", str(original), "--mods", str(ROOT / "mixins")]
    subprocess.run([str(cli), "plan", *base, "--out", str(plan)], check=True, capture_output=True, text=True)
    applied = subprocess.run([str(cli), "apply", *base, "--out", str(output)], check=True, capture_output=True, text=True)
    report = json.loads(applied.stdout)
    return output, report["stage_changed"]


@contextlib.contextmanager
def lock(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a") as handle:
        if fcntl is not None:
            fcntl.flock(handle, fcntl.LOCK_EX)
        yield


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["stage", "check", "build", "test"])
    parser.add_argument("--horizon", type=Path, default=ROOT.parent / "horizon")
    parser.add_argument("--target-dir", type=Path, default=ROOT / "target/horizon-cargo")
    args = parser.parse_args()
    horizon = args.horizon.resolve()
    target = args.target_dir.resolve()
    if target.is_relative_to(horizon):
        parser.error("Cargo target must be outside the original Horizon checkout")
    started = time.monotonic()
    with lock(ROOT / "target/stage.lock"):
        output, changed = mixin_stage(horizon, ROOT / "target/mixins")
        if target.is_relative_to(output) or output.is_relative_to(target):
            parser.error("Cargo target and generated source stage must be separate")
        report = {"stage": str(output), "stage_changed": changed,
                  "horizon_revision": PIN, "staging_seconds": time.monotonic() - started}
        if args.action != "stage":
            command = ["cargo", args.action, "--locked", "--offline", "-p", "horizon-client",
                       "--bin", "horizon-client", "--no-default-features", "--target-dir", str(target)]
            if args.action == "build":
                command.extend(["--profile", "fast"])
            if args.action == "test":
                command.extend(["game::bbs", "--", "--nocapture"])
            with lock(target.parent / (target.name + ".bbs.lock")):
                run_start = time.monotonic()
                result = subprocess.run(command, cwd=output, capture_output=True, text=True)
            report.update(cargo_seconds=time.monotonic() - run_start, cargo_command=command,
                          cargo_exit=result.returncode)
            log = ROOT / "target" / f"horizon-{args.action}.log"
            log.write_text(result.stdout + result.stderr, encoding="utf-8")
            report["cargo_log"] = str(log)
            if result.returncode:
                print(result.stderr[-12000:], file=sys.stderr)
                print(json.dumps(report), file=sys.stderr)
                return result.returncode
            if args.action == "build":
                report["binary"] = str(target / "fast/horizon-client")
        print(json.dumps(report, indent=2))
        return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        if isinstance(error, subprocess.CalledProcessError) and error.stderr:
            print(error.stderr, file=sys.stderr)
        print(f"BBS: {error}", file=sys.stderr)
        raise SystemExit(1)
