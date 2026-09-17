"""Review-gate classification over two independent paired foundation captures."""

import argparse
import json
import math
from pathlib import Path

from measurement_support import numeric_summary
from .manifest import POLICY


def classify_samples(baseline: list[float], candidate: list[float], limit: float,
                     minimum: int, *, timing: bool) -> dict:
    if (not isinstance(baseline, list) or not isinstance(candidate, list)
            or min(len(baseline), len(candidate)) < minimum or any(
            isinstance(v, bool) or not isinstance(v, (int, float)) or not math.isfinite(v) or v <= 0
            for v in [*baseline, *candidate])):
        return {"outcome": "inconclusive", "reason": "missing/invalid raw samples"}
    before, after = numeric_summary(baseline), numeric_summary(candidate)
    delta = after["median"] - before["median"]
    budget = before["median"] * limit
    noise = POLICY["mad_multiplier"] * max(before["median_absolute_deviation"],
                                           after["median_absolute_deviation"]) if timing else 0
    if delta > budget:
        outcome = "regression" if delta > noise else "inconclusive"
    else:
        outcome = "within-review-limits" if delta + noise <= budget else "inconclusive"
    return {"outcome": outcome, "baseline": before, "candidate": after,
            "increase_fraction": delta / before["median"], "twice_larger_mad": noise}


def repeated_outcome(first: dict, second: dict) -> str:
    if first["outcome"] == second["outcome"] == "regression":
        return "review-required"
    if first["outcome"] == second["outcome"] == "within-review-limits":
        return "within-review-limits"
    return "inconclusive"


def frame_changes(baseline: dict, candidate: dict) -> dict:
    if (not isinstance(baseline, dict) or not isinstance(candidate, dict)
            or baseline.get("status") != "supported" or candidate.get("status") != "supported"):
        return {"outcome": "inconclusive", "reason": "unsupported/missing frame metrics"}
    before, after = baseline.get("functions", {}), candidate.get("functions", {})
    if not isinstance(before, dict) or not isinstance(after, dict) or not before or not after:
        return {"outcome": "inconclusive", "reason": "empty frame inventory"}
    for function in [*before.values(), *after.values()]:
        if not isinstance(function, dict) or function.get("status") != "supported" or any(
            type(function.get(field)) is not int or function[field] < 0
            for field in ("frame_bytes", "max_stack_bytes", "static_frame_accesses")
        ):
            return {"outcome": "inconclusive", "reason": "incomplete per-callable frame metrics"}
    changes = {}
    for name in sorted(before.keys() | after.keys()):
        old, new = before.get(name), after.get(name)
        if old != new:
            changes[name] = {"baseline": old, "candidate": new}
    return {"outcome": "recorded", "changes": changes}


def compile_samples(entry: dict, field: str) -> list:
    samples = entry.get("compile_samples")
    return ([sample.get(field) if isinstance(sample, dict) else None for sample in samples]
            if isinstance(samples, list) else [])


def compare_runs(first: dict, second: dict) -> dict:
    result = {"schema": 1, "kind": "foundation-comparison", "outcome": "incompatible",
              "issues": [], "workloads": []}
    for run in (first, second):
        if (run.get("schema") != 1 or run.get("kind") != "foundation"
                or run.get("policy") != POLICY or "failure" in run):
            result["issues"].append("unsupported/failed capture or changed policy")
    if result["issues"]:
        return result
    for run in (first, second):
        compatibility = run.get("compatibility")
        if not isinstance(compatibility, dict) or any(field not in compatibility for field in (
                "manifest_sha256", "inputs", "runtime", "host", "harness", "configuration", "workloads")):
            result["issues"].append("capture compatibility/provenance fields are missing")
    if result["issues"]:
        return result
    if not first.get("run_id") or first["run_id"] == second.get("run_id"):
        result["issues"].append("two independent paired captures are required")
    if first.get("compatibility") != second.get("compatibility"):
        result["issues"].append("manifest, inputs, runtime, host/toolchain or modes differ")
    for run in (first, second):
        if set(run.get("variants", {})) != {"baseline", "candidate"}:
            result["issues"].append("capture does not contain both compiler builds")
    if result["issues"]:
        return result
    for role in ("baseline", "candidate"):
        if first["variants"][role].get("compiler") != second["variants"][role].get("compiler"):
            result["issues"].append(f"{role} binary/provenance differs across captures")
    configuration = first["compatibility"].get("configuration", {})
    if not isinstance(configuration, dict) or any(configuration.get(field) != value for field, value in (
            ("target", "x86_64-sysv"), ("mir_profile", "default"),
            ("compiler_profile", "golden"), ("mir_exclusions", []))):
        result["issues"].append("unsupported/missing target, profile or exclusions")
    elif any(type(configuration.get(field)) is not int or configuration[field] < 1 for field in (
            "compile_repeats", "native_repeats", "compile_warmups", "native_warmups")):
        result["issues"].append("repetition/warmup provenance is missing")
    identities = [first["variants"][role].get("compiler", {}) for role in ("baseline", "candidate")]
    if any(not all(identity.get(field) for field in (
            "revision", "executable_sha256", "profile", "build_toolchain")) for identity in identities):
        result["issues"].append("compiler provenance is incomplete")
    if any(type(identity.get("dirty")) is not bool or "build_flags" not in identity for identity in identities):
        result["issues"].append("compiler dirty-state/build-flag attestation is missing")
    if any(identities[0].get(field) != identities[1].get(field)
           for field in ("profile", "build_toolchain", "build_flags")):
        result["issues"].append("compiler build profiles/toolchains differ")
    specs = first["compatibility"].get("workloads", [])
    ids = [w["id"] for w in specs]
    if not ids or len(set(ids)) != len(ids):
        result["issues"].append("empty/duplicate workload inventory")
    indexes = []
    for run in (first, second):
        index = {}
        for role in ("baseline", "candidate"):
            entries = run["variants"][role].get("workloads", [])
            if not isinstance(entries, list) or any(not isinstance(e, dict) or "id" not in e for e in entries):
                result["issues"].append("workload evidence is missing or malformed")
                continue
            index[role] = {entry["id"]: entry for entry in entries}
            if len(index[role]) != len(entries) or set(index[role]) != set(ids):
                result["issues"].append("missing/duplicate workload evidence")
        indexes.append(index)
    if result["issues"]:
        return result
    result["outcome"] = "within-review-limits"
    if any(run.get("qualification") != "full" for run in (first, second)):
        result["issues"].append("smoke/subset captures cannot qualify foundation adoption")
        result["outcome"] = "inconclusive"
    invalid = []
    for spec in specs:
        identity = spec["id"]
        row = {"id": identity, "metrics": {}, "frames": [], "assembly_bytes": []}
        entries = [(index["baseline"][identity], index["candidate"][identity]) for index in indexes]
        for before, after in entries:
            if not before.get("reporting_equivalent") or not after.get("reporting_equivalent"):
                invalid.append(f"{identity}: reporting changed artifacts")
            if spec["native"] and any(e.get("native_semantics") != spec["expected"]
                                      for e in (before, after)):
                invalid.append(f"{identity}: native status/stdout/stderr differs from manifest")
            row["frames"].append(frame_changes(before.get("frames", {}), after.get("frames", {})))
            row["assembly_bytes"].append({"baseline": before.get("assembly_bytes"),
                                          "candidate": after.get("assembly_bytes")})
        for role in (0, 1):
            if (not entries[0][role].get("assembly_sha256") or
                    entries[0][role]["assembly_sha256"] != entries[1][role].get("assembly_sha256")):
                invalid.append(f"{identity}: compiler configuration emitted different assembly across captures")
        for metric, field, limit, timing in (
            ("compile_wall_ms", "wall_ms", POLICY["timing_increase"], True),
            ("compile_peak_rss_kib", "peak_rss_kib", POLICY["rss_text_increase"], False),
        ):
            pairs = [classify_samples(compile_samples(before, field),
                compile_samples(after, field), limit,
                POLICY["compile_repeats"], timing=timing) for before, after in entries]
            row["metrics"][metric] = {"outcome": repeated_outcome(*pairs), "pairs": pairs}
        if spec["native"]:
            pairs = [classify_samples(before.get("native_samples_ms", []),
                after.get("native_samples_ms", []), POLICY["timing_increase"],
                POLICY["native_repeats"], timing=True) for before, after in entries]
            row["metrics"]["native_wall_ms"] = {"outcome": repeated_outcome(*pairs), "pairs": pairs}
            sizes = []
            for before, after in entries:
                old, new = before.get("native_text") or {}, after.get("native_text") or {}
                sizes.append(classify_samples([old.get("bytes")], [new.get("bytes")],
                    POLICY["rss_text_increase"], 1, timing=False) if
                    old.get("status") == new.get("status") == "supported" else
                    {"outcome": "inconclusive", "reason": "unsupported/missing native text size"})
            row["metrics"]["native_text_bytes"] = {"outcome": repeated_outcome(*sizes), "pairs": sizes}
        outcomes = [metric["outcome"] for metric in row["metrics"].values()]
        if any(f["outcome"] == "inconclusive" for f in row["frames"]):
            outcomes.append("inconclusive")
        row["outcome"] = ("review-required" if "review-required" in outcomes else
                          "inconclusive" if "inconclusive" in outcomes else "within-review-limits")
        if row["outcome"] == "review-required":
            result["outcome"] = "review-required"
        elif row["outcome"] == "inconclusive" and result["outcome"] != "review-required":
            result["outcome"] = "inconclusive"
        result["workloads"].append(row)
    if invalid:
        result["outcome"] = "invalid"
        result["issues"].extend(invalid)
    return result


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("first", type=Path)
    parser.add_argument("second", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    result = compare_runs(json.loads(args.first.read_text()), json.loads(args.second.read_text()))
    result["captures"] = [str(args.first.resolve()), str(args.second.resolve())]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(f"foundation comparison: {result['outcome']}; {args.output}")
    return 0 if result["outcome"] == "within-review-limits" else 1
