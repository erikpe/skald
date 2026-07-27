use super::*;

#[test]
fn hir_dump_is_deterministic_and_records_types_and_operations() {
    let output = check_text("fn main() -> i64 { return 1 + -2; }");
    let hir = output.hir.unwrap();

    assert_eq!(
        dump_hir(&hir),
        concat!(
            "HirProgram @0..35\n",
            "  SelectedModule m0\n",
            "  Modules\n",
            "    Module m0 main source 0 provider provider0 package package0\n",
            "  Entry f0\n",
            "  Declarations\n",
            "    Declaration f0 module m0 \"main\" internal @0..35\n",
            "      Parameters\n",
            "      ReturnType i64\n",
            "  Definitions\n",
            "    Definition f0 @0..35\n",
            "      Locals\n",
            "      Block @17..35\n",
            "        Return @19..33\n",
            "          Binary AddI64 : i64 @26..32\n",
            "            Integer 1 : i64 @26..27\n",
            "            Unary NegateI64 : i64 @30..32\n",
            "              Integer 2 : i64 @31..32\n",
        )
    );
}
