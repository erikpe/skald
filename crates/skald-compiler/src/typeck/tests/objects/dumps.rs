use super::*;

#[test]
fn object_hir_dump_is_exact_and_identity_based() {
    let output = check_text(concat!(
        "class Box { value: i64; init(value: i64) { self.value = value; } fn get() -> i64 { return self.value; } }\n",
        "fn main() -> i64 { var value: Box = Box(1); return value.get(); }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    assert_eq!(
        dump_hir(&output.hir.unwrap()),
        concat!(
            "HirProgram @0..172\n",
            "  Entry f0\n",
            "  Classes\n",
            "    Class c0 \"Box\" @0..105\n",
            "      Fields\n",
            "        Field c0:field0 \"value\" : i64 @12..23\n",
            "      Initializer c0:init0 @24..64\n",
            "        Parameter c0:init0:p0 \"value\" value : i64 @29..39\n",
            "      CopyConstructor\n",
            "        Synthesized c0\n",
            "          Primitive c0:field0\n",
            "      CopyAssignment\n",
            "        Synthesized c0\n",
            "          Primitive c0:field0\n",
            "      Methods\n",
            "        Method c0:method0 \"get\" readonly -> i64 @65..103\n",
            "  ClassDefinitions\n",
            "    ClassDefinition c0 @0..105\n",
            "      MemberDefinition c0:init0 @24..64\n",
            "        Locals\n",
            "        Block @41..64\n",
            "          FieldAssignment @43..62\n",
            "            FieldPlace c0:field0 @43..62\n",
            "              ObjectPlace c0:init0:self : class c0 mutable @43..47\n",
            "            Binding c0:init0:p0 : i64 @56..61\n",
            "      MemberDefinition c0:method0 @65..103\n",
            "        Locals\n",
            "        Block @81..103\n",
            "          Return @83..101\n",
            "            FieldRead c0:field0 : i64 @90..100\n",
            "              ObjectPlace c0:method0:self : class c0 readonly @90..94\n",
            "  Declarations\n",
            "    Declaration f0 \"main\" internal @106..171\n",
            "      Parameters\n",
            "      ReturnType i64\n",
            "  Definitions\n",
            "    Definition f0 @106..171\n",
            "      Locals\n",
            "        Local f0:l0 \"value\" : class c0 @125..149\n",
            "      Block @123..171\n",
            "        LocalDeclaration f0:l0 @125..149\n",
            "          ObjectInitialization @142..148\n",
            "            ObjectPlace f0:l0 : class c0 mutable @129..134\n",
            "            Construct c0 via c0:init0 @142..148\n",
            "              ValueArgument @146..147\n",
            "                Integer 1 : i64 @146..147\n",
            "            ElidedCopy\n",
            "              Operation Synthesized c0\n",
            "        Return @150..169\n",
            "          MethodCall c0:method0 : i64 @157..168\n",
            "            ObjectPlace f0:l0 : class c0 mutable @157..162\n",
        )
    );
}
