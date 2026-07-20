use super::*;

#[test]
fn ast_dump_is_deterministic() {
    let (_, output) = parse_text("fn main() -> i64 { return add(1, -2); }");

    assert_eq!(
        dump_ast(&output.ast),
        concat!(
            "CompilationUnit @0..39\n",
            "  Function @0..39\n",
            "    Name \"main\" @3..7\n",
            "    Parameters\n",
            "    ReturnType\n",
            "      Type I64 @13..16\n",
            "    Block @17..39\n",
            "      Return @19..37\n",
            "        Call @26..36\n",
            "          Callee\n",
            "            Identifier \"add\" @26..29\n",
            "          Arguments\n",
            "            Integer \"1\" @30..31\n",
            "            Unary Negate @33..35\n",
            "              Integer \"2\" @34..35\n",
        )
    );
}
