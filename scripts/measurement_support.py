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
) -> subprocess.CompletedProcess[bytes]:
    command = [str(argument) for argument in arguments]
    process = subprocess.Popen(
        command,
        cwd=cwd,
        stdout=subprocess.PIPE if capture else subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        start_new_session=True,
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
) -> tuple[subprocess.CompletedProcess[bytes], float]:
    started = time.perf_counter_ns()
    completed = run_checked(
        arguments,
        operation=operation,
        cwd=cwd,
        capture=capture,
        timeout_seconds=timeout_seconds,
        expected_statuses=expected_statuses,
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


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def source_inventory(paths: Iterable[Path]) -> dict[str, object]:
    files = sorted({path.resolve() for path in paths})
    return {
        "files": [str(path.relative_to(REPOSITORY)) for path in files],
        "bytes": sum(path.stat().st_size for path in files),
    }


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
