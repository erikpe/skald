//! Compile-time coverage for the intentional repository-internal API paths.

use std::{ffi::OsString, path::Path};

use skald_compiler::{
    backend::{emit_assembly, target_by_name, Target},
    diagnostics::{render_diagnostics, Diagnostics},
    driver::{
        compile_request_to_assembly, compile_source_to_assembly, run_cli, ArtifactKind,
        ArtifactOptions, AssemblyArtifact, CompilationEnvironment, CompilationError,
        CompilationRequest, EntrySelector, StandardLibrarySelection, Toolchain,
    },
    external::{ExternalLink, ExternalLinkTable},
    hir::{
        dump_hir, HirBinaryOperation, HirComparisonOperand, HirComparisonPredicate,
        HirIntegerBitwiseOperation, HirIntegerType, HirInterfaceCallTarget,
        HirInterfaceConformance, HirInterfaceDeclaration, HirObjectSlice, HirObjectView,
        HirPrimitiveCast, HirPrimitiveCastKind, HirPrimitiveComparison, HirPrimitiveType,
        HirProgram, HirUnaryOperation, HirViewTarget, ObjectProjection, Type,
    },
    identity::{
        ArrayTypeId, CallableId, ExternalLinkId, InterfaceId, InterfaceRequirementId, ModuleId,
        PackageId, ProviderId,
    },
    lexer::{dump_tokens, lex, LexOutput},
    literal::{IntegerRadix, NumericLiteralKind},
    mir::{
        dump_mir, dump_preliminary_mir, lower_hir, lower_preliminary_hir, verify_mir,
        verify_preliminary_mir, MirArrayInstruction, MirArrayLifecycle, MirArrayType,
        MirArrayTypeTable, MirBaseCopy, MirBinaryOperation, MirCallReceiver, MirComparisonOperand,
        MirComparisonPredicate, MirDirectBase, MirIntegerBitwiseOperation, MirIntegerType,
        MirInterfaceCallTarget, MirInterfaceConformance, MirInterfaceDeclaration, MirObjectView,
        MirPlaceProjection, MirPrimitiveCast, MirPrimitiveCastKind, MirPrimitiveComparison,
        MirPrimitiveType, MirProgram, MirType, MirUnaryOperation, MirViewTarget,
    },
    module::{
        dump_module_graph, load_module_graph, normalize_provider_roots, CandidateResolution,
        LoadedModule, ModuleCandidate, ModuleGraph, ModuleGraphLoadFailure, ModuleImportEdge,
        ModulePath, ModulePathErrorKind, ModuleProvenance, ModuleSourceLocation,
        NormalizedProvider, ProgramModuleTable, ProgramModuleTableError,
        ProviderNormalizationError, ProviderRootConfiguration, ProviderSet,
    },
    passes::{
        run_mir_pipeline,
        static_lifecycle::{
            dump_planned_mir, dump_static_effects, plan_static_lifetimes,
            synthesize_static_lifecycle, verify_planned_mir, verify_synthesized_mir,
            PlannedMirProgram, StaticEffectAnalysis, StaticLifecyclePlan, StaticLifetimeDependency,
        },
    },
    resolve::{
        dump_resolved, resolve, resolve_module_graph, ResolveOutput, ResolvedClassHierarchy,
        ResolvedClassMember, ResolvedModuleBinding, ResolvedModuleBindingTable,
        ResolvedModuleBindings, ResolvedModuleDeclaration, ResolvedModuleDeclarationTable,
        ResolvedModuleDeclarations, ResolvedOrdinaryBinding, ResolvedOrdinaryBindingTable,
        ResolvedOrdinaryBindings, ResolvedPrimitiveCastExpr, ResolvedPrimitiveType,
        ResolvedProgram, ResolvedTopLevelId, ResolvedVisibility,
    },
    source::SourceDatabase,
    syntax::{dump_ast, parse, CompilationUnit, ParseOutput, PrimitiveCastExpr, PrimitiveType},
    typeck::{type_check, TypeCheckOutput},
};

#[test]
fn intentional_module_and_request_paths_compose() {
    let entry: ModulePath = "app::main".parse().unwrap();
    let request = CompilationRequest::new(
        EntrySelector::Module(entry.clone()),
        vec!["project/modules".into(), "deps/modules".into()],
        StandardLibrarySelection::Replacement("sdk/modules".into()),
        Target::X86_64SysV,
        ArtifactOptions::new(ArtifactKind::Assembly, Some("main.s".into())),
        CompilationEnvironment::new("project".into(), "install/std".into()),
    );

    assert_eq!(entry.to_string(), "app::main");
    assert_eq!(request.entry(), &EntrySelector::Module(entry));
    assert_eq!(request.module_roots().len(), 2);
    assert_eq!(request.artifact().output(), Some(Path::new("main.s")));
    assert_eq!(
        "not-valid".parse::<ModulePath>().unwrap_err().kind(),
        ModulePathErrorKind::InvalidComponent
    );

    let _module_identity: Option<ModuleId> = None;
    let _external_link_identity: Option<ExternalLinkId> = None;
    let _external_link: Option<ExternalLink> = None;
    let _external_links: Option<ExternalLinkTable> = None;
    let _provider_identity: Option<ProviderId> = None;
    let _package_identity: Option<PackageId> = None;
    let _provenance: Option<ModuleProvenance> = None;
    let _source_location: Option<ModuleSourceLocation> = None;
    let _provider_set: Option<ProviderSet> = None;
    let _provider: Option<NormalizedProvider> = None;
    let _candidate: Option<ModuleCandidate> = None;
    let _resolution: Option<CandidateResolution> = None;
    let _graph: Option<ModuleGraph> = None;
    let _loaded_module: Option<LoadedModule> = None;
    let _import_edge: Option<ModuleImportEdge> = None;
    let _normalizer: fn(
        &Path,
        &[ProviderRootConfiguration],
    ) -> Result<ProviderSet, Vec<ProviderNormalizationError>> = normalize_provider_roots;
    let _loader: fn(
        &EntrySelector,
        &Path,
        &ProviderSet,
    ) -> Result<ModuleGraph, ModuleGraphLoadFailure> = load_module_graph;
    let _graph_dumper: fn(&ModuleGraph) -> String = dump_module_graph;
    let _module_table: Option<ProgramModuleTable> = None;
    let _module_table_error: Option<ProgramModuleTableError> = None;
    let _graph_resolver: fn(&ModuleGraph) -> ResolveOutput = resolve_module_graph;
    let _module_declaration: Option<ResolvedModuleDeclaration> = None;
    let _module_declarations: Option<ResolvedModuleDeclarations> = None;
    let _module_declaration_table: Option<ResolvedModuleDeclarationTable> = None;
    let _module_binding: Option<ResolvedModuleBinding> = None;
    let _module_bindings: Option<ResolvedModuleBindings> = None;
    let _module_binding_table: Option<ResolvedModuleBindingTable> = None;
    let _ordinary_binding: Option<ResolvedOrdinaryBinding> = None;
    let _ordinary_bindings: Option<ResolvedOrdinaryBindings> = None;
    let _ordinary_binding_table: Option<ResolvedOrdinaryBindingTable> = None;
    let _top_level_id: Option<ResolvedTopLevelId> = None;
    assert!(ResolvedVisibility::Public.is_public());
    assert!(!ResolvedVisibility::Private.is_public());
}

#[test]
fn intentional_phase_and_dump_paths_compose() {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add(
        "api.ska",
        "class Empty { init() {} } fn main() -> i64 { return 0; }",
    );
    let source = sources.get(source_id).unwrap();

    let lexed: LexOutput = lex(source);
    let _tokens = dump_tokens(source, &lexed.tokens);
    let parsed: ParseOutput = parse(source, &lexed.tokens);
    let ast: &CompilationUnit = &parsed.ast;
    let _ast_dump = dump_ast(ast);
    let resolved: ResolveOutput = resolve(ast);
    let resolved_program: &ResolvedProgram = &resolved.program;
    assert_eq!(resolved_program.modules.selected().index(), 0);
    assert_eq!(
        resolved_program.classes.iter().next().unwrap().module,
        resolved_program.modules.selected()
    );
    let hierarchy: &ResolvedClassHierarchy = &resolved_program.hierarchy;
    let class = resolved_program.classes.iter().next().unwrap().id;
    let _base_chain = hierarchy.base_chain(class);
    let _member: Option<ResolvedClassMember> = hierarchy.member(class, "member");
    let _syntax_primitive_cast: Option<PrimitiveCastExpr> = None;
    let _syntax_primitive_type: Option<PrimitiveType> = None;
    let _resolved_primitive_cast: Option<ResolvedPrimitiveCastExpr> = None;
    let _resolved_primitive_type: Option<ResolvedPrimitiveType> = None;
    let _resolved_dump = dump_resolved(resolved_program);
    let checked: TypeCheckOutput = type_check(resolved_program);
    let hir: &HirProgram = checked.hir.as_ref().unwrap();
    assert_eq!(hir.modules, resolved_program.modules);
    let _hir_dump = dump_hir(hir);
    let _base_projection: Option<ObjectProjection> = None;
    let _object_slice: Option<HirObjectSlice> = None;
    let _object_view: Option<HirObjectView> = None;
    let _view_target: Option<HirViewTarget> = None;
    let _interface: Option<HirInterfaceDeclaration> = None;
    let _conformance: Option<HirInterfaceConformance> = None;
    let _interface_call: Option<HirInterfaceCallTarget> = None;
    let hir_comparison = HirPrimitiveComparison {
        predicate: HirComparisonPredicate::LessThan,
        operand: HirComparisonOperand::Integer(HirIntegerType::U64),
    };
    assert_eq!(hir_comparison.operand_type(), Type::U64);
    assert_eq!(hir_comparison.result_type(), Type::Bool);
    let hir_cast = HirPrimitiveCast::new(HirPrimitiveType::U64, HirPrimitiveType::U8);
    assert_eq!(hir_cast.kind(), HirPrimitiveCastKind::IntegerBits);
    assert_eq!(HirPrimitiveType::Bool.payload_type(), Type::Bool);
    assert_eq!(hir_cast.source_type(), Type::U64);
    assert_eq!(hir_cast.result_type(), Type::U8);
    let hir_bitwise = HirBinaryOperation::IntegerBitwise {
        operation: HirIntegerBitwiseOperation::Xor,
        operand: HirIntegerType::U8,
    };
    assert_eq!(hir_bitwise.operand_type(), Type::U8);
    assert_eq!(hir_bitwise.result_type(), Type::U8);
    assert_eq!(
        HirUnaryOperation::BitwiseComplement(HirIntegerType::U64).result_type(),
        Type::U64
    );
    let preliminary = lower_preliminary_hir(hir);
    verify_preliminary_mir(&preliminary).unwrap();
    let _preliminary_dump = dump_preliminary_mir(&preliminary);
    assert!(!preliminary.has_static_initializers());
    let planned: PlannedMirProgram = plan_static_lifetimes(preliminary).unwrap();
    verify_planned_mir(&planned).unwrap();
    let static_effects: &StaticEffectAnalysis = planned.effects();
    let _static_effect_dump = dump_static_effects(static_effects);
    let _planned_dump = dump_planned_mir(&planned);
    let _lifecycle: &StaticLifecyclePlan = planned.lifecycle();
    let _dependencies: &[StaticLifetimeDependency] = planned.dependencies();
    let synthesized = synthesize_static_lifecycle(planned).unwrap();
    verify_synthesized_mir(&synthesized).unwrap();
    assert!(synthesized.static_lifecycle.is_some());
    let mir: MirProgram = lower_hir(hir);
    assert_eq!(mir.modules, hir.modules);
    let _mir_base: Option<MirDirectBase> = None;
    let _mir_base_copy: Option<MirBaseCopy<skald_compiler::identity::CopyConstructorId>> = None;
    let _mir_projection: Option<MirPlaceProjection> = None;
    let _mir_view: Option<MirObjectView> = None;
    let _mir_view_target: Option<MirViewTarget> = None;
    let _mir_interface: Option<MirInterfaceDeclaration> = None;
    let _mir_conformance: Option<MirInterfaceConformance> = None;
    let _mir_interface_call: Option<MirInterfaceCallTarget> = None;
    let _mir_receiver: Option<MirCallReceiver> = None;
    let _mir_array_id: Option<ArrayTypeId> = None;
    let _mir_array_table: Option<MirArrayTypeTable> = None;
    let _mir_array_type: Option<MirArrayType> = None;
    let _mir_array_lifecycle: Option<MirArrayLifecycle> = None;
    let _mir_array_instruction: Option<MirArrayInstruction> = None;
    let mir_comparison = MirPrimitiveComparison {
        predicate: MirComparisonPredicate::LessThan,
        operand: MirComparisonOperand::Integer(MirIntegerType::U64),
    };
    assert_eq!(mir_comparison.operand_type(), MirType::U64);
    assert_eq!(mir_comparison.result_type(), MirType::Bool);
    let mir_cast = MirPrimitiveCast::new(MirPrimitiveType::U64, MirPrimitiveType::U8);
    assert_eq!(mir_cast.kind(), MirPrimitiveCastKind::IntegerBits);
    assert_eq!(MirPrimitiveType::Bool.payload_type(), MirType::Bool);
    assert_eq!(mir_cast.source_type(), MirType::U64);
    assert_eq!(mir_cast.result_type(), MirType::U8);
    let mir_bitwise = MirBinaryOperation::IntegerBitwise {
        operation: MirIntegerBitwiseOperation::Xor,
        operand: MirIntegerType::U8,
    };
    assert_eq!(mir_bitwise.operand_type(), MirType::U8);
    assert_eq!(mir_bitwise.result_type(), MirType::U8);
    assert_eq!(
        MirUnaryOperation::BitwiseComplement(MirIntegerType::U64).result_type(),
        MirType::U64
    );
    verify_mir(&mir).unwrap();
    let mir = run_mir_pipeline(mir).unwrap();
    let _mir_dump = dump_mir(&mir);
    let target = target_by_name("x86_64-sysv").unwrap();
    let _assembly = emit_assembly(target, &mir).unwrap();
    let diagnostics: &Diagnostics = &checked.diagnostics;
    let _diagnostics = render_diagnostics(&sources, diagnostics);
    let _identity_path: Option<CallableId> = None;
    let _interface_identity: Option<InterfaceId> = None;
    let _requirement_identity: Option<InterfaceRequirementId> = None;
    let _literal_path: Option<NumericLiteralKind> = None;
    let _integer_radix_path: Option<IntegerRadix> = None;
}

#[test]
fn intentional_driver_paths_compile() {
    let _cli_entry: fn(Vec<OsString>) -> i32 = run_cli::<Vec<OsString>>;
    let _request_pipeline: fn(&CompilationRequest) -> Result<AssemblyArtifact, CompilationError> =
        compile_request_to_assembly;
    let _toolchain = Toolchain::new("cc", "runtime.a");
    let artifact = compile_source_to_assembly(
        "api.ska",
        "fn main() -> i64 { return 0; }",
        Target::X86_64SysV,
    )
    .unwrap();

    assert!(!artifact.assembly.is_empty());

    let arrays = compile_source_to_assembly(
        "arrays-api.ska",
        concat!(
            "fn duplicate(values: bool[]) -> bool[] { return values; }\n",
            "fn main() -> i64 {\n",
            "  var values: bool[] = bool[](3u);\n",
            "  var empty: u8[] = u8[]();\n",
            "  values[-1] = true;\n",
            "  var copied: bool[] = duplicate(values);\n",
            "  var selected: bool = copied[2];\n",
            "  return 0;\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .unwrap();
    assert!(arrays.assembly.contains(".Lska_array_0_initialize_element"));
    assert!(arrays.assembly.contains(".Lska_array_0_copy_element"));
    assert!(arrays.assembly.contains("[r11 + r10*1 + 16]"));
    assert!(arrays.assembly.contains("call ska_rt_free"));
}
