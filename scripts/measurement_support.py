"""Shared process, identity, and summary helpers for repository measurements."""

from __future__ import annotations

import hashlib
import os
import signal
import statistics
import subprocess
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence


REPOSITORY = Path(__file__).resolve().parents[1]
DEFAULT_TIMEOUT_SECONDS = 30.0
GNU_TIME = Path("/usr/bin/time")
DEFAULT_RUNTIME_ARCHIVE = REPOSITORY / "build/runtime/libskald_runtime.a"


class MeasurementFailure(RuntimeError):
    """A measured child failed, timed out, or produced unusable metadata."""


@dataclass(frozen=True)
class ProcessUsage:
    completed: subprocess.CompletedProcess[bytes]
    wall_ms: float
    peak_rss_kib: int


def resolve_repository_path(value: str | Path) -> Path:
    path = Path(value)
    return path.resolve() if path.is_absolute() else (REPOSITORY / path).resolve()


def unique_run_directory(parent: Path, label: str = "run") -> Path:
    parent.mkdir(parents=True, exist_ok=True)
    return Path(tempfile.mkdtemp(prefix=f"{label}-", dir=parent))


def run_checked(
    arguments: Sequence[str | Path],
    *,
    operation: str = "command",
    cwd: Path = REPOSITORY,
    capture: bool = True,
    timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
    expected_statuses: Iterable[int] = (0,),
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[bytes]:
    command = [str(argument) for argument in arguments]
    process = subprocess.Popen(
        command,
        cwd=cwd,
        stdout=subprocess.PIPE if capture else subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        start_new_session=True,
        env=env,
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout_seconds)
    except subprocess.TimeoutExpired as error:
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
        raise MeasurementFailure(
            f"{operation} exceeded {timeout_seconds:g} seconds: {command!r}"
        ) from error
    completed = subprocess.CompletedProcess(command, process.returncode, stdout, stderr)
    if completed.returncode not in set(expected_statuses):
        rendered = completed.stderr.decode("utf-8", errors="backslashreplace")
        raise MeasurementFailure(
            f"{operation} failed with status {completed.returncode}: {command!r}\n{rendered}"
        )
    return completed


def timed_process(
    arguments: Sequence[str | Path],
    *,
    operation: str = "command",
    cwd: Path = REPOSITORY,
    capture: bool = True,
    timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
    expected_statuses: Iterable[int] = (0,),
    env: dict[str, str] | None = None,
) -> tuple[subprocess.CompletedProcess[bytes], float]:
    started = time.perf_counter_ns()
    completed = run_checked(
        arguments,
        operation=operation,
        cwd=cwd,
        capture=capture,
        timeout_seconds=timeout_seconds,
        expected_statuses=expected_statuses,
        env=env,
    )
    elapsed_ms = (time.perf_counter_ns() - started) / 1_000_000.0
    return completed, elapsed_ms


def measured_process(
    arguments: Sequence[str | Path],
    *,
    operation: str,
    run_directory: Path,
    cwd: Path = REPOSITORY,
    timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
    env: dict[str, str] | None = None,
) -> ProcessUsage:
    if not GNU_TIME.is_file():
        raise MeasurementFailure(f"GNU time is required at {GNU_TIME}")
    descriptor, usage_name = tempfile.mkstemp(prefix="usage-", suffix=".txt", dir=run_directory)
    os.close(descriptor)
    usage_path = Path(usage_name)
    try:
        completed, wall_ms = timed_process(
            [GNU_TIME, "--format=%M", f"--output={usage_path}", *arguments],
            operation=operation,
            cwd=cwd,
            timeout_seconds=timeout_seconds,
            env=env,
        )
        try:
            peak_rss_kib = int(usage_path.read_text(encoding="ascii").strip())
        except (OSError, ValueError) as error:
            raise MeasurementFailure(f"could not read peak RSS for {operation}: {error}") from error
        return ProcessUsage(completed, wall_ms, peak_rss_kib)
    finally:
        usage_path.unlink(missing_ok=True)


def timing_summary(samples_ms: Sequence[float]) -> dict[str, float | int]:
    if not samples_ms:
        raise ValueError("timing summary requires at least one sample")
    median = statistics.median(samples_ms)
    deviations = [abs(sample - median) for sample in samples_ms]
    return {
        "samples": len(samples_ms),
        "median_ms": median,
        "median_absolute_deviation_ms": statistics.median(deviations),
        "min_ms": min(samples_ms),
        "max_ms": max(samples_ms),
    }


def alternating_order(values: Sequence[str], iteration: int) -> tuple[str, ...]:
    ordered = tuple(values)
    return ordered if iteration % 2 == 0 else tuple(reversed(ordered))


def numeric_summary(samples: Sequence[float]) -> dict[str, float | int]:
    """Unit-neutral summaries for RSS, sizes, and comparison metrics."""
    summary = timing_summary(samples)
    return {"samples": summary["samples"], "median": summary["median_ms"],
            "median_absolute_deviation": summary["median_absolute_deviation_ms"],
            "min": summary["min_ms"], "max": summary["max_ms"]}


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def source_inventory(paths: Iterable[Path]) -> dict[str, object]:
    files = sorted({path.resolve() for path in paths})
    return {
        "files": [str(path.relative_to(REPOSITORY)) for path in files],
        "bytes": sum(path.stat().st_size for path in files),
    }


def runtime_artifact_identity(archive: str | Path | None = None) -> dict[str, object]:
    selected = archive or os.environ.get("SKALD_RUNTIME_ARCHIVE", DEFAULT_RUNTIME_ARCHIVE)
    archive_path = resolve_repository_path(selected)
    configuration_path = archive_path.parent / "build-config.txt"
    if not archive_path.is_file():
        raise MeasurementFailure(f"runtime archive does not exist: {archive_path}")
    try:
        lines = configuration_path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise MeasurementFailure(
            f"could not read runtime build configuration {configuration_path}: {error}"
        ) from error

    configuration: dict[str, str] = {}
    for line in lines:
        if "=" not in line:
            raise MeasurementFailure(
                f"invalid runtime build configuration line in {configuration_path}: {line!r}"
            )
        name, value = line.split("=", 1)
        if name in configuration:
            raise MeasurementFailure(
                f"duplicate runtime build configuration field {name!r} in {configuration_path}"
            )
        configuration[name] = value

    expected = {"format", "cc", "ar", "cflags"}
    if set(configuration) != expected or configuration["format"] != "1":
        raise MeasurementFailure(
            f"unsupported runtime build configuration in {configuration_path}"
        )
    return {
        "archive": display_path(archive_path),
        "archive_sha256": sha256_bytes(archive_path.read_bytes()),
        "configuration_file": display_path(configuration_path),
        "configuration": {
            "format": 1,
            "cc": configuration["cc"],
            "ar": configuration["ar"],
            "cflags": configuration["cflags"],
        },
    }


def display_path(path: Path) -> str:
    try:
        return str(path.relative_to(REPOSITORY))
    except ValueError:
        return str(path)


def repository_identity(timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS) -> dict[str, object]:
    revision = run_checked(
        ["git", "rev-parse", "HEAD"],
        operation="repository revision inspection",
        timeout_seconds=timeout_seconds,
    ).stdout.decode("ascii").strip()
    dirty = bool(
        run_checked(
            ["git", "status", "--porcelain"],
            operation="repository status inspection",
            timeout_seconds=timeout_seconds,
        ).stdout
    )
    return {"revision": revision, "dirty": dirty}


def deterministic_projection(report: dict[str, object]) -> object:
    """Return the stable portion of a report, excluding operational observations."""
    return report["deterministic"]
