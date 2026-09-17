from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from cleanup_measurements.metrics import assembly_metrics, parse_text_size


ASSEMBLY = """.type example, @function
example:
    push rbp
    mov rbp, rsp
    sub rsp, 32
    mov qword ptr [rbp - 8], rax
    lea rdi, [rbp - 16]
    sub rsp, 16
    mov qword ptr [rsp], rdi
    mov rax, qword ptr [rbp - 8]
    add rsp, 16
    leave
    ret
.size example, .-example
"""


class FoundationMetricTests(unittest.TestCase):
    def test_known_frame_and_explicit_memory_operands(self):
        self.assertEqual(assembly_metrics(ASSEMBLY), {"status": "supported", "functions": {
            "example": {"status": "supported", "frame_bytes": 40, "max_stack_bytes": 56,
                        "static_frame_accesses": 3}}})

    def test_wrapper_has_saved_frame_pointer_but_no_local_reservation(self):
        wrapper = ASSEMBLY.replace("    sub rsp, 32\n", "")
        self.assertEqual(assembly_metrics(wrapper)["functions"]["example"]["frame_bytes"], 8)

    def test_multiple_functions_and_local_symbols_are_measured_independently(self):
        assembly = ASSEMBLY + ASSEMBLY.replace("example", ".Lska.second").replace("sub rsp, 32", "sub rsp, 48")
        metrics = assembly_metrics(assembly)
        self.assertEqual(metrics["status"], "supported")
        self.assertEqual(metrics["functions"][".Lska.second"]["frame_bytes"], 56)
        self.assertEqual(len(metrics["functions"]), 2)

    def test_lifecycle_address_prefix_and_stack_neutral_leaves(self):
        prefixed = ASSEMBLY.replace("    push rbp", "    lea rdi, [rdi + 16]\n    push rbp")
        self.assertEqual(assembly_metrics(prefixed)["functions"]["example"]["frame_bytes"], 40)
        leaf = ".type leaf, @function\nleaf:\n    mov rax, [rsi + rcx*8 + 16]\n    mov [rdi], rax\n    ret\n.size leaf, .-leaf\n"
        self.assertEqual(assembly_metrics(leaf)["functions"]["leaf"],
                         {"status": "supported", "frame_bytes": 0, "max_stack_bytes": 0,
                          "static_frame_accesses": 0})

    def test_nested_rsp_scratch_and_failure_call_recipes(self):
        helper = """.type helper, @function
helper:
    test rax, rax
    je .Ldone
    sub rsp, 16
    mov [rsp], rax
    sub rsp, 16
    mov [rsp], rax
    mov rax, [rsp]
    add rsp, 16
    mov rax, [rsp]
    add rsp, 16
.Ldone:
    ret
.size helper, .-helper
"""
        self.assertEqual(assembly_metrics(helper)["functions"]["helper"],
                         {"status": "supported", "frame_bytes": 0, "max_stack_bytes": 32,
                          "static_frame_accesses": 4})
        inconsistent = helper.replace("je .Ldone", "je .Linner").replace(
            "mov [rsp], rax\n    sub rsp, 16", "mov [rsp], rax\n.Linner:\n    sub rsp, 16", 1)
        self.assertEqual(assembly_metrics(inconsistent)["status"], "unsupported")
        # A loop accumulating reservations cannot be measured as a fixed frame.
        growing = helper.replace("je .Ldone", "je .Ldone\n.Lloop:").replace("    add rsp, 16", "    jmp .Lloop", 1)
        self.assertEqual(assembly_metrics(growing)["status"], "unsupported")
        self.assertEqual(assembly_metrics(helper.replace("je .Ldone", "je missing"))["status"], "unsupported")
        self.assertEqual(assembly_metrics(helper.replace("    add rsp, 16", "", 1))["status"], "unsupported")
        self.assertEqual(assembly_metrics(helper.replace("ret", "call ska_rt_panic\n    ud2"))["status"], "supported")
        failure = """.type failure, @function
failure:
    test rax, rax
    je .Linvalid
    sub rsp, 8
    call ska_rt_panic
.Linvalid:
    ud2
.size failure, .-failure
"""
        self.assertEqual(assembly_metrics(failure)["functions"]["failure"]["max_stack_bytes"], 8)

    def test_unknown_shapes_are_not_guessed_zero(self):
        for old, new in (("push rbp", "push rbx"), ("sub rsp, 32", "sub rsp, r11"),
                         ("[rbp - 8]", "[rbp + rax * 8]"),
                         ("leave", "pop rbx\n    leave"),
                         ("sub rsp, 16", "sub rsp, r11")):
            with self.subTest(new=new):
                result = assembly_metrics(ASSEMBLY.replace(old, new))
                self.assertEqual(result["status"], "unsupported")
                self.assertNotIn("frame_bytes", result["functions"]["example"])
        self.assertEqual(assembly_metrics(".text\n")["status"], "unsupported")
        self.assertEqual(assembly_metrics(ASSEMBLY.split(".size")[0])["status"], "unsupported")

    def test_native_text_excludes_data_headers_and_total_file_size(self):
        self.assertEqual(parse_text_size("file:\n.text 112 4096\n.text.hot 8 4208\n.data 999 8192\n"),
                         {"status": "supported", "bytes": 120})
        for output in (".data 99 8192", ".text unknown 4096", ".text 40"):
            self.assertEqual(parse_text_size(output)["status"], "unsupported")

    def test_second_operand_stack_writes_and_loop_edges_are_unsupported(self):
        for instruction in ("xchg rax, rsp", "xadd rax, esp", "xchg rax, rbp",
                            "xchg qword ptr [rdi], rsp", "loop .Lbody", "loope .Lbody", "loopne .Lbody"):
            with self.subTest(instruction=instruction):
                assembly = f""".type probe, @function
probe:
.Lbody:
    {instruction}
    ret
.size probe, .-probe
"""
                result = assembly_metrics(assembly)
                self.assertEqual(result["status"], "unsupported")
                self.assertNotIn("max_stack_bytes", result["functions"]["probe"])
        # A memory operand addressed through rsp does not write rsp itself.
        memory_swap = ".type swap, @function\nswap:\n    xchg qword ptr [rsp], rax\n    ret\n.size swap, .-swap\n"
        self.assertEqual(assembly_metrics(memory_swap)["functions"]["swap"]["static_frame_accesses"], 1)


if __name__ == "__main__":
    unittest.main()
