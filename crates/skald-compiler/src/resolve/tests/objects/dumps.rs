use super::*;

#[test]
fn resolved_direct_base_dump_is_exact_and_identity_based() {
    let output = resolve_text("class Derived extends Base {}\nclass Base {}");
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    assert_eq!(
        dump_resolved(&output.program),
        concat!(
            "ResolvedProgram @0..43\n",
            "  Entry <none>\n",
            "  ClassDeclarations\n",
            "    Class c0 \"Derived\" @0..29\n",
            "      DirectBase c1 @22..26\n",
            "      Fields\n",
            "      OrdinaryInitializer\n",
            "        <none>\n",
            "      CopyConstructor\n",
            "        Synthesized c0\n",
            "      CopyAssignment\n",
            "        Synthesized c0\n",
            "      Destructor\n",
            "        <none>\n",
            "      Methods\n",
            "    Class c1 \"Base\" @30..43\n",
            "      Fields\n",
            "      OrdinaryInitializer\n",
            "        <none>\n",
            "      CopyConstructor\n",
            "        Synthesized c1\n",
            "      CopyAssignment\n",
            "        Synthesized c1\n",
            "      Destructor\n",
            "        <none>\n",
            "      Methods\n",
            "  Declarations\n",
            "  Definitions\n",
            "  ClassDefinitions\n",
            "    ClassDefinition c0 @0..29\n",
            "    ClassDefinition c1 @30..43\n",
        )
    );
}

#[test]
fn resolved_destructor_dump_is_exact_and_identity_based() {
    let output = resolve_text(concat!(
        "class Empty { init() {} destroy { return; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    assert_eq!(
        dump_resolved(&output.program),
        concat!(
            "ResolvedProgram @0..77\n",
            "  Entry f0\n",
            "  ClassDeclarations\n",
            "    Class c0 \"Empty\" @0..45\n",
            "      Fields\n",
            "      OrdinaryInitializer\n",
            "        Initializer c0:init0 @14..23\n",
            "          Parameters\n",
            "      CopyConstructor\n",
            "        Synthesized c0\n",
            "      CopyAssignment\n",
            "        Synthesized c0\n",
            "      Destructor\n",
            "        Destructor c0:destroy0 @24..43\n",
            "      Methods\n",
            "  Declarations\n",
            "    Declaration f0 \"main\" internal @46..76\n",
            "      Parameters\n",
            "      ReturnType\n",
            "        Type I64 @59..62\n",
            "  Definitions\n",
            "    Definition f0 @46..76\n",
            "      Locals\n",
            "      Block @63..76\n",
            "        Return @65..74\n",
            "          Integer \"0\" @72..73\n",
            "  ClassDefinitions\n",
            "    ClassDefinition c0 @0..45\n",
            "      MemberDefinition c0:init0 @14..23\n",
            "        Locals\n",
            "        Block @21..23\n",
            "      MemberDefinition c0:destroy0 @24..43\n",
            "        Locals\n",
            "        Block @32..43\n",
            "          Return @34..41\n",
        )
    );
}

#[test]
fn resolved_object_dump_is_exact_and_identity_based() {
    let output = resolve_text(concat!(
        "class Box { value: i64; init(value: i64) { self.value = value; } fn get() -> i64 { return self.value; } }\n",
        "fn main() -> i64 { var box: Box = Box(1); return box.get(); }\n",
    ));
    assert!(!output.has_errors());

    assert_eq!(
        dump_resolved(&output.program),
        concat!(
            "ResolvedProgram @0..168\n",
            "  Entry f0\n",
            "  ClassDeclarations\n",
            "    Class c0 \"Box\" @0..105\n",
            "      Fields\n",
            "        Field c0:field0 \"value\" @12..23\n",
            "          Type I64 @19..22\n",
            "      OrdinaryInitializer\n",
            "        Initializer c0:init0 @24..64\n",
            "          Parameters\n",
            "            Parameter c0:init0:p0 \"value\" @29..39\n",
            "              Binding Value\n",
            "              Type I64 @36..39\n",
            "      CopyConstructor\n",
            "        Synthesized c0\n",
            "      CopyAssignment\n",
            "        Synthesized c0\n",
            "      Destructor\n",
            "        <none>\n",
            "      Methods\n",
            "        Method c0:method0 readonly \"get\" @65..103\n",
            "          Parameters\n",
            "          ReturnType\n",
            "            Type I64 @77..80\n",
            "  Declarations\n",
            "    Declaration f0 \"main\" internal @106..167\n",
            "      Parameters\n",
            "      ReturnType\n",
            "        Type I64 @119..122\n",
            "  Definitions\n",
            "    Definition f0 @106..167\n",
            "      Locals\n",
            "        Local f0:l0 \"box\" @125..147\n",
            "          Type Class c0 @134..137\n",
            "      Block @123..167\n",
            "        LocalDeclaration f0:l0 @125..147\n",
            "          Construct c0 with c0:init0 @140..146\n",
            "            Integer \"1\" @144..145\n",
            "        Return @148..165\n",
            "          MethodCall c0:method0 @155..164\n",
            "            Receiver f0:l0 class c0 @155..158\n",
            "            Arguments\n",
            "  ClassDefinitions\n",
            "    ClassDefinition c0 @0..105\n",
            "      MemberDefinition c0:init0 @24..64\n",
            "        Locals\n",
            "        Block @41..64\n",
            "          FieldAssignment c0:field0 @43..62\n",
            "            Receiver c0:init0:self class c0 @43..47\n",
            "            Equal @54..55\n",
            "            Value\n",
            "              Binding c0:init0:p0 @56..61\n",
            "      MemberDefinition c0:method0 @65..103\n",
            "        Locals\n",
            "        Block @81..103\n",
            "          Return @83..101\n",
            "            FieldAccess c0:field0 @90..100\n",
            "              Receiver c0:method0:self class c0 @90..94\n",
        )
    );
}
