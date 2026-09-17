"""The maintained cleanup workload corpus, shared by ordinary and paired runs."""

from dataclasses import dataclass
from pathlib import Path

from measurement_support import REPOSITORY


@dataclass(frozen=True)
class Workload:
    identity: str
    dimensions: tuple[str, ...]
    compiler_arguments: tuple[str, ...]
    input_paths: tuple[Path, ...]
    runtime_trace: str
    native_group: str | None = None
    native_arguments: tuple[str, ...] = ()


def direct_workload(
    identity: str,
    dimensions: tuple[str, ...],
    source: str,
    *,
    compiler_arguments: tuple[str, ...] = (),
    runtime_trace: str = "enabled",
    native_group: str | None = None,
    native_arguments: tuple[str, ...] = (),
) -> Workload:
    source_path = REPOSITORY / source
    trace_arguments = ("--omit-runtime-trace",) if runtime_trace == "omitted" else ()
    return Workload(
        identity,
        dimensions,
        (source, *compiler_arguments, *trace_arguments),
        (source_path,),
        runtime_trace,
        native_group,
        native_arguments,
    )


def workloads() -> tuple[Workload, ...]:
    module_root = REPOSITORY / "tests/golden/vm_benchmark/cases/modules"
    compile_baselines = (
        direct_workload(
            "compile/small-source",
            ("small-source",),
            "samples/vertical/exit_42.ska",
            compiler_arguments=("--no-stdlib",),
            runtime_trace="omitted",
        ),
        Workload(
            "compile/many-modules-large-cfg",
            ("many-modules", "large-callable-cfg"),
            ("--entry", "app", "--module-root", str(module_root.relative_to(REPOSITORY))),
            tuple(sorted(module_root.glob("**/*.ska"))),
            "enabled",
        ),
        direct_workload(
            "compile/many-generic-applications",
            ("many-generic-applications",),
            "tests/golden/generic_interfaces/conformance_matrix.ska",
            compiler_arguments=("--no-stdlib",),
            runtime_trace="omitted",
        ),
        direct_workload(
            "compile/nested-ownership",
            ("nested-ownership",),
            "tests/golden/standard_vec/vec_generic_type_shapes.ska",
            runtime_trace="omitted",
        ),
    )
    native = [
        direct_workload(
            "native/generic-vector-growth",
            ("existing-native", "generic-applications", "ownership"),
            "tests/benchmarks/generic_vec/growth.ska",
            native_group="generic-vector",
        )
    ]
    for integer in ("u8", "u64", "i64"):
        for form in ("range", "while"):
            native.append(
                direct_workload(
                    f"native/range-{integer}-{form}",
                    ("existing-native", "loop"),
                    f"tests/benchmarks/range_loop/{integer}_{form}.ska",
                    runtime_trace="omitted",
                    native_group=f"range-{integer}",
                )
            )
    trace_sources = (
        ("call-recursion", "tests/benchmarks/panic_runtime_trace/call_recursion.ska"),
        ("tight-loop", "tests/benchmarks/panic_runtime_trace/tight_loop.ska"),
        ("allocation", "tests/benchmarks/panic_runtime_trace/allocation.ska"),
        ("representative-golden", "tests/golden/operators/primitive_operator_profile.ska"),
    )
    for name, source in trace_sources:
        for policy in ("enabled", "omitted"):
            native.append(
                direct_workload(
                    f"native/runtime-trace-{name}-{policy}",
                    ("existing-native", "runtime-trace"),
                    source,
                    runtime_trace=policy,
                    native_group=f"runtime-trace-{name}",
                )
            )
    return (*compile_baselines, *native)


