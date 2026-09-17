"""Explicit binary provenance and controlled shared measurement inputs."""

import os
import platform
import shutil
from pathlib import Path

from measurement_support import MeasurementFailure, run_checked, sha256_bytes


def tool_identity(command: str, timeout: float) -> dict:
    path = shutil.which(command)
    if path is None:
        raise MeasurementFailure(f"measurement tool is unavailable: {command}")
    output = run_checked([path, "--version"], timeout_seconds=timeout,
                         operation=f"{command} toolchain identity")
    return {"path": str(Path(path).resolve()), "sha256": sha256_bytes(Path(path).read_bytes()),
            "version": output.stdout.decode(errors="replace").strip()}


def host_identity(notes: str, timeout: float) -> dict:
    cpuinfo = Path("/proc/cpuinfo").read_text()
    model = next((line.split(":", 1)[1].strip() for line in cpuinfo.splitlines()
                  if line.startswith("model name")), "unknown")
    governors = sorted({p.read_text().strip() for p in
        Path("/sys/devices/system/cpu").glob("cpu*/cpufreq/scaling_governor")})
    cc = os.environ.get("CC", "cc")
    tools = {"cc": tool_identity(cc, timeout), "size": tool_identity("size", timeout),
             "time": tool_identity("/usr/bin/time", timeout)}
    for name in ("as", "ld"):
        selected = run_checked([cc, f"-print-prog-name={name}"], timeout_seconds=timeout,
                               operation=f"native {name} selection").stdout.decode().strip()
        tools[name] = tool_identity(selected, timeout)
    return {"system": list(platform.uname()), "cpu_model": model,
            "cpu_affinity": sorted(os.sched_getaffinity(0)), "governors": governors,
            "python": platform.python_version(), "notes": notes, "tools": tools}


def compiler_identity(path: Path, revision: str, profile: str, toolchain: str,
                      dirty: bool, timeout: float, *, build_flags: str = "") -> dict:
    if not path.is_file():
        raise MeasurementFailure(f"compiler binary does not exist: {path}")
    commit = run_checked(["git", "rev-parse", "--verify", f"{revision}^{{commit}}"],
                         timeout_seconds=timeout, operation="attested compiler revision")
    return {"revision": commit.stdout.decode().strip(), "dirty": dirty,
            "profile": profile, "build_toolchain": toolchain, "build_flags": build_flags,
            "executable_sha256": sha256_bytes(path.read_bytes())}
