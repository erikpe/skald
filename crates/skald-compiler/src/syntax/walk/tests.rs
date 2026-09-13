use std::collections::BTreeSet;

use crate::{source::Span, test_support::parse_source};

use super::*;
use crate::syntax::ast::{
    ArrayConstructionArguments, BracketProjectionBounds, CallArguments, ClassMember, Expression,
    ForInSource, OptionalBoxInitializer, Statement, TopLevelDeclaration,
};

const ALL_SHAPES_SOURCE: &str = concat!(
    "import support;\n",
    "extern fn external(value: i64) -> i64;\n",
    "intrinsic fn intrinsic(value: i64) -> i64;\n",
    "interface Face<T> where T: Bound { fn apply(value: T) -> T; }\n",
    "class Base { init(value: i64) {} }\n",
    "class Shapes<T> extends Base implements Face<T> where T: Bound {\n",
    "  field: i64;\n",
    "  static stored: i64 = seed;\n",
    "  init(value: i64) { super(value); }\n",
    "  copy(ref other: Shapes<T>) { return; }\n",
    "  assign(ref other: Shapes<T>) { return; }\n",
    "  destroy { return; }\n",
    "  fn inspect(value: i64, maybe: i64?, values: i64[]) -> i64 {\n",
    "    var local: i64 = value;\n",
    "    none;\n",
    "    some(value);\n",
    "    value;\n",
    "    Shapes<T>();\n",
    "    Shapes<T>.stored();\n",
    "    1; 'a'; \"text\"; true; self;\n",
    "    -value; value + seed; true && false;\n",
    "    value is Shapes<T>; maybe is some; maybe!;\n",
    "    (i64) value; (Shapes<T>) value;\n",
    "    new Shapes<T>(value, seed);\n",
    "    new Shapes<T>(copy other);\n",
    "    new i64?(); new i64?(none);\n",
    "    i64[](); i64[](value); i64[](copy values);\n",
    "    i64[](value; index => index); i64[]{value, seed};\n",
    "    invoke(value, seed); Shapes<T>(copy other);\n",
    "    (value); value.member; values[value]; values[value:seed];\n",
    "    if (true) { value; } elif (false) { seed; } else { other; }\n",
    "    while (true) { break; continue; }\n",
    "    for (item: i64 in values) { item; }\n",
    "    for (item in value .. seed) { item; }\n",
    "    { value; }\n",
    "    receiver.field = value;\n",
    "    receiver = value;\n",
    "    return value;\n",
    "  }\n",
    "}\n",
    "fn empty() -> unit { return; }\n",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EventKind {
    Enter,
    Leave,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Event {
    kind: EventKind,
    node: &'static str,
    span: Span,
}

#[derive(Default)]
struct Recorder {
    events: Vec<Event>,
}

impl<'ast> SyntaxVisitor<'ast> for Recorder {
    fn enter(&mut self, node: SyntaxNode<'ast>) -> WalkControl {
        self.events.push(Event {
            kind: EventKind::Enter,
            node: node_label(node),
            span: node.span(),
        });
        WalkControl::Continue
    }

    fn leave(&mut self, node: SyntaxNode<'ast>) {
        self.events.push(Event {
            kind: EventKind::Leave,
            node: node_label(node),
            span: node.span(),
        });
    }
}

struct PruningRecorder<'source> {
    source: &'source str,
    pruned_slice: &'source str,
    events: Vec<Event>,
}

#[derive(Default)]
struct DirectChildRecorder {
    depth: usize,
    children: Vec<Span>,
}

impl<'ast> SyntaxVisitor<'ast> for DirectChildRecorder {
    fn enter(&mut self, node: SyntaxNode<'ast>) -> WalkControl {
        if self.depth == 1 {
            self.children.push(node.span());
        }
        self.depth += 1;
        WalkControl::Continue
    }

    fn leave(&mut self, _node: SyntaxNode<'ast>) {
        self.depth -= 1;
    }
}

impl<'ast> SyntaxVisitor<'ast> for PruningRecorder<'_> {
    fn enter(&mut self, node: SyntaxNode<'ast>) -> WalkControl {
        self.events.push(Event {
            kind: EventKind::Enter,
            node: node_label(node),
            span: node.span(),
        });
        if source_slice(self.source, node.span()) == self.pruned_slice {
            WalkControl::Prune
        } else {
            WalkControl::Continue
        }
    }

    fn leave(&mut self, node: SyntaxNode<'ast>) {
        self.events.push(Event {
            kind: EventKind::Leave,
            node: node_label(node),
            span: node.span(),
        });
    }
}

#[test]
fn emits_balanced_events_in_exact_source_order_with_original_spans() {
    let source = concat!(
        "import support;\n",
        "fn add(left: i64, right: i64) -> i64 {\n",
        "  var total: i64 = left + right;\n",
        "  return total;\n",
        "}\n",
    );
    let (_, output) = parse_source(source);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let mut recorder = Recorder::default();
    walk(&output.ast, &mut recorder);

    let entered = recorder
        .events
        .iter()
        .filter(|event| event.kind == EventKind::Enter)
        .map(|event| format!("{} @ {}", event.node, source_slice(source, event.span)))
        .collect::<Vec<_>>();
    assert_eq!(
        entered,
        vec![
            format!("unit @ {source}"),
            "import @ import support;".into(),
            concat!(
                "declaration:function @ fn add(left: i64, right: i64) -> i64 {\n",
                "  var total: i64 = left + right;\n",
                "  return total;\n",
                "}"
            )
            .into(),
            "type @ i64".into(),
            "type @ i64".into(),
            "type @ i64".into(),
            concat!(
                "block @ {\n",
                "  var total: i64 = left + right;\n",
                "  return total;\n",
                "}"
            )
            .into(),
            "statement:local @ var total: i64 = left + right;".into(),
            "type @ i64".into(),
            "expression:binary @ left + right".into(),
            "expression:identifier @ left".into(),
            "expression:identifier @ right".into(),
            "statement:return @ return total;".into(),
            "expression:identifier @ total".into(),
        ]
    );
    assert_balanced(&recorder.events);
}

#[test]
fn parsed_all_shapes_fixture_reaches_every_structural_family() {
    let (_, output) = parse_source(ALL_SHAPES_SOURCE);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let mut recorder = Recorder::default();
    walk(&output.ast, &mut recorder);
    assert_balanced(&recorder.events);

    let observed = recorder
        .events
        .iter()
        .filter(|event| event.kind == EventKind::Enter)
        .map(|event| event.node)
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        "block",
        "class-member:copy-assignment",
        "class-member:copy-constructor",
        "class-member:destructor",
        "class-member:field",
        "class-member:initializer",
        "class-member:method",
        "class-member:static-field",
        "declaration:class",
        "declaration:external-function",
        "declaration:function",
        "declaration:interface",
        "declaration:intrinsic-function",
        "expression:absent",
        "expression:allocation-copy",
        "expression:allocation-ordinary",
        "expression:array-copy",
        "expression:array-elements",
        "expression:array-empty",
        "expression:array-indexed",
        "expression:array-length",
        "expression:binary",
        "expression:boolean",
        "expression:bracket-index",
        "expression:bracket-slice",
        "expression:byte-literal",
        "expression:call-copy",
        "expression:call-ordinary",
        "expression:generic-static-selection",
        "expression:generic-type-application",
        "expression:grouped",
        "expression:identifier",
        "expression:logical",
        "expression:member-access",
        "expression:numeric-literal",
        "expression:object-cast",
        "expression:optional-box-absent",
        "expression:optional-box-value",
        "expression:presence-test",
        "expression:present",
        "expression:primitive-cast",
        "expression:self",
        "expression:string-literal",
        "expression:type-test",
        "expression:unary",
        "expression:unwrap",
        "import",
        "named-type",
        "statement:base-initialization",
        "statement:block",
        "statement:break",
        "statement:conditional",
        "statement:continue",
        "statement:expression",
        "statement:field-assignment",
        "statement:for-iterable",
        "statement:for-range",
        "statement:local",
        "statement:object-assignment",
        "statement:return",
        "statement:while",
        "type",
        "unit",
    ]);

    assert_eq!(observed, expected);
}

#[test]
fn composite_children_follow_their_declared_source_order() {
    let source = concat!(
        "class Example<T> extends Base implements First, Second where T: Bound {\n",
        "  static stored: Value = make(seed);\n",
        "  fn run(left: Left, right: Right) -> Result {\n",
        "    var local: Local = first + second;\n",
        "    if (if_condition) { first; } elif (elif_condition) { second; } else { third; }\n",
        "    for (item: Item in lower .. upper) {}\n",
        "    return;\n",
        "  }\n",
        "}\n",
    );
    let (_, output) = parse_source(source);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let TopLevelDeclaration::Class(class) = &output.ast.declarations[0] else {
        panic!("expected class declaration");
    };
    assert_eq!(
        direct_child_slices(class_declaration(&output.ast), source),
        [
            "Base",
            "First",
            "Second",
            "Bound",
            "static stored: Value = make(seed);",
            concat!(
                "fn run(left: Left, right: Right) -> Result {\n",
                "    var local: Local = first + second;\n",
                "    if (if_condition) { first; } elif (elif_condition) { second; } else { third; }\n",
                "    for (item: Item in lower .. upper) {}\n",
                "    return;\n",
                "  }"
            ),
        ]
    );

    assert_eq!(
        direct_child_slices(&class.members[0], source),
        ["Value", "make(seed)"]
    );
    let ClassMember::Method(method) = &class.members[1] else {
        panic!("expected method member");
    };
    assert_eq!(
        direct_child_slices(&class.members[1], source),
        [
            "Left",
            "Right",
            "Result",
            concat!(
                "{\n",
                "    var local: Local = first + second;\n",
                "    if (if_condition) { first; } elif (elif_condition) { second; } else { third; }\n",
                "    for (item: Item in lower .. upper) {}\n",
                "    return;\n",
                "  }"
            ),
        ]
    );
    assert_eq!(
        direct_child_slices(&method.body.statements[0], source),
        ["Local", "first + second"]
    );
    assert_eq!(
        direct_child_slices(&method.body.statements[1], source),
        [
            "if_condition",
            "{ first; }",
            "elif_condition",
            "{ second; }",
            "{ third; }",
        ]
    );
    assert_eq!(
        direct_child_slices(&method.body.statements[2], source),
        ["Item", "lower", "upper", "{}"]
    );

    let Statement::Local(local) = &method.body.statements[0] else {
        unreachable!()
    };
    assert_eq!(
        direct_child_slices(&local.initializer, source),
        ["first", "second"]
    );
    let ClassMember::StaticField(field) = &class.members[0] else {
        unreachable!()
    };
    assert_eq!(
        direct_child_slices(&field.initializer.as_ref().unwrap().expression, source),
        ["make", "seed"]
    );
}

#[test]
fn complete_type_occurrences_are_opaque_leaf_events() {
    let source = concat!(
        "class Derived extends Base<Inner<Value>> {}\n",
        "fn accept(value: Outer<Inner<Value>>) -> unit {}\n",
    );
    let (_, output) = parse_source(source);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let TopLevelDeclaration::Class(class) = &output.ast.declarations[0] else {
        panic!("expected class declaration");
    };
    let TopLevelDeclaration::Function(function) = &output.ast.declarations[1] else {
        panic!("expected function declaration");
    };

    let mut named_type = Recorder::default();
    walk(class.direct_base.as_ref().unwrap(), &mut named_type);
    assert_eq!(named_type.events.len(), 2);
    assert_eq!(named_type.events[0].node, "named-type");
    assert_balanced(&named_type.events);

    let mut type_syntax = Recorder::default();
    walk(&function.parameters[0].type_syntax, &mut type_syntax);
    assert_eq!(type_syntax.events.len(), 2);
    assert_eq!(type_syntax.events[0].node, "type");
    assert_balanced(&type_syntax.events);
}

#[test]
fn pruning_is_local_balanced_and_resumes_at_later_siblings() {
    let source = concat!(
        "class Holder {\n",
        "  static hidden: i64 = inside;\n",
        "  fn visible() -> unit {}\n",
        "}\n",
        "fn choose() -> unit {\n",
        "  if (condition) { hidden; } elif (later) { visible; }\n",
        "  left.member + right.member;\n",
        "}\n",
        "fn after() -> unit {}\n",
    );
    let (_, output) = parse_source(source);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    assert_pruning_case(
        &output.ast,
        source,
        concat!(
            "class Holder {\n",
            "  static hidden: i64 = inside;\n",
            "  fn visible() -> unit {}\n",
            "}"
        ),
        "fn after() -> unit {}",
    );
    assert_pruning_case(
        &output.ast,
        source,
        "static hidden: i64 = inside;",
        "fn visible() -> unit {}",
    );
    assert_pruning_case(&output.ast, source, "{ hidden; }", "later");
    assert_pruning_case(&output.ast, source, "left.member", "right.member");
}

#[test]
fn absent_optional_children_emit_no_placeholder_nodes() {
    let source = concat!(
        "class Holder { static value: i64; }\n",
        "fn empty(values: i64[]) -> unit {\n",
        "  new i64?();\n",
        "  values[:];\n",
        "  for (value in values) {}\n",
        "  return;\n",
        "}\n",
    );
    let (_, output) = parse_source(source);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let mut recorder = Recorder::default();
    walk(&output.ast, &mut recorder);
    let edges = direct_edges(&recorder.events);

    assert_eq!(
        children_of(&edges, source, "new i64?()"),
        ["i64?"],
        "an absent optional-box initializer contributes only its target type"
    );
    assert_eq!(
        children_of(&edges, source, "values[:]"),
        ["values"],
        "a fully open slice contributes only its receiver"
    );
    assert_eq!(
        children_of(&edges, source, "for (value in values) {}"),
        ["values", "{}"],
        "an unannotated loop contributes no type placeholder"
    );
    assert!(children_of(&edges, source, "return;").is_empty());
}

fn assert_pruning_case(
    ast: &CompilationUnit,
    source: &str,
    pruned_slice: &str,
    retained_slice: &str,
) {
    let mut recorder = PruningRecorder {
        source,
        pruned_slice,
        events: Vec::new(),
    };
    walk(ast, &mut recorder);
    assert_balanced(&recorder.events);

    let pruned_event = recorder
        .events
        .iter()
        .position(|event| {
            event.kind == EventKind::Enter && source_slice(source, event.span) == pruned_slice
        })
        .unwrap_or_else(|| panic!("missing pruned node {pruned_slice:?}"));
    let entered_node = &recorder.events[pruned_event];
    let left_node = recorder
        .events
        .get(pruned_event + 1)
        .expect("a pruned node must still receive a leave event");
    assert_eq!(left_node.kind, EventKind::Leave);
    assert_eq!(
        (left_node.node, left_node.span),
        (entered_node.node, entered_node.span)
    );
    let retained_event = recorder.events.iter().position(|event| {
        event.kind == EventKind::Enter && source_slice(source, event.span) == retained_slice
    });
    assert!(
        retained_event.is_some_and(|retained_event| retained_event > pruned_event),
        "traversal should resume at later sibling {retained_slice:?}"
    );
}

fn assert_balanced(events: &[Event]) {
    let mut entered = Vec::new();
    for event in events {
        match event.kind {
            EventKind::Enter => entered.push((event.node, event.span)),
            EventKind::Leave => assert_eq!(entered.pop(), Some((event.node, event.span))),
        }
    }
    assert!(entered.is_empty(), "every entered node must be left");
}

fn direct_edges(events: &[Event]) -> Vec<(Span, Span)> {
    let mut ancestors = Vec::new();
    let mut edges = Vec::new();
    for event in events {
        match event.kind {
            EventKind::Enter => {
                if let Some(parent) = ancestors.last() {
                    edges.push((*parent, event.span));
                }
                ancestors.push(event.span);
            }
            EventKind::Leave => {
                assert_eq!(ancestors.pop(), Some(event.span));
            }
        }
    }
    edges
}

fn children_of<'source>(
    edges: &[(Span, Span)],
    source: &'source str,
    parent_slice: &str,
) -> Vec<&'source str> {
    edges
        .iter()
        .filter(|(parent, _)| source_slice(source, *parent) == parent_slice)
        .map(|(_, child)| source_slice(source, *child))
        .collect()
}

fn direct_child_slices<'ast>(
    root: impl Into<SyntaxNode<'ast>>,
    source: &'ast str,
) -> Vec<&'ast str> {
    let mut recorder = DirectChildRecorder::default();
    walk(root, &mut recorder);
    recorder
        .children
        .into_iter()
        .map(|span| source_slice(source, span))
        .collect()
}

fn class_declaration(ast: &CompilationUnit) -> &TopLevelDeclaration {
    let declaration = &ast.declarations[0];
    assert!(matches!(declaration, TopLevelDeclaration::Class(_)));
    declaration
}

fn source_slice(source: &str, span: Span) -> &str {
    &source[span.range().start()..span.range().end()]
}

fn node_label(node: SyntaxNode<'_>) -> &'static str {
    match node {
        SyntaxNode::CompilationUnit(_) => "unit",
        SyntaxNode::Import(_) => "import",
        SyntaxNode::Declaration(declaration) => match declaration {
            TopLevelDeclaration::Function(_) => "declaration:function",
            TopLevelDeclaration::ExternalFunction(_) => "declaration:external-function",
            TopLevelDeclaration::IntrinsicFunction(_) => "declaration:intrinsic-function",
            TopLevelDeclaration::Class(_) => "declaration:class",
            TopLevelDeclaration::Interface(_) => "declaration:interface",
        },
        SyntaxNode::ClassMember(member) => match member {
            ClassMember::Field(_) => "class-member:field",
            ClassMember::StaticField(_) => "class-member:static-field",
            ClassMember::Initializer(_) => "class-member:initializer",
            ClassMember::CopyConstructor(_) => "class-member:copy-constructor",
            ClassMember::CopyAssignment(_) => "class-member:copy-assignment",
            ClassMember::Destructor(_) => "class-member:destructor",
            ClassMember::Method(_) => "class-member:method",
        },
        SyntaxNode::Block(_) => "block",
        SyntaxNode::Statement(statement) => match statement {
            Statement::BaseInitialization(_) => "statement:base-initialization",
            Statement::Local(_) => "statement:local",
            Statement::Return(_) => "statement:return",
            Statement::Break(_) => "statement:break",
            Statement::Continue(_) => "statement:continue",
            Statement::Expression(_) => "statement:expression",
            Statement::Conditional(_) => "statement:conditional",
            Statement::While(_) => "statement:while",
            Statement::ForIn(statement) => match statement.source {
                ForInSource::Iterable(_) => "statement:for-iterable",
                ForInSource::Range(_) => "statement:for-range",
            },
            Statement::Block(_) => "statement:block",
            Statement::FieldAssignment(_) => "statement:field-assignment",
            Statement::ObjectAssignment(_) => "statement:object-assignment",
        },
        SyntaxNode::Expression(expression) => expression_label(expression),
        SyntaxNode::Type(_) => "type",
        SyntaxNode::NamedType(_) => "named-type",
    }
}

fn expression_label(expression: &Expression) -> &'static str {
    match expression {
        Expression::Absent(_) => "expression:absent",
        Expression::Present(_) => "expression:present",
        Expression::Identifier(_) => "expression:identifier",
        Expression::GenericTypeApplication(_) => "expression:generic-type-application",
        Expression::GenericStaticSelection(_) => "expression:generic-static-selection",
        Expression::NumericLiteral(_) => "expression:numeric-literal",
        Expression::ByteLiteral(_) => "expression:byte-literal",
        Expression::StringLiteral(_) => "expression:string-literal",
        Expression::Boolean(_) => "expression:boolean",
        Expression::Unary(_) => "expression:unary",
        Expression::Binary(_) => "expression:binary",
        Expression::Logical(_) => "expression:logical",
        Expression::TypeTest(_) => "expression:type-test",
        Expression::PresenceTest(_) => "expression:presence-test",
        Expression::Unwrap(_) => "expression:unwrap",
        Expression::PrimitiveCast(_) => "expression:primitive-cast",
        Expression::ObjectCast(_) => "expression:object-cast",
        Expression::Allocation(expression) => match expression.arguments {
            CallArguments::Ordinary(_) => "expression:allocation-ordinary",
            CallArguments::Copy { .. } => "expression:allocation-copy",
        },
        Expression::OptionalBoxAllocation(expression) => match expression.initializer {
            OptionalBoxInitializer::Absent { .. } => "expression:optional-box-absent",
            OptionalBoxInitializer::Value { .. } => "expression:optional-box-value",
        },
        Expression::ArrayConstruction(expression) => match expression.arguments {
            ArrayConstructionArguments::Empty { .. } => "expression:array-empty",
            ArrayConstructionArguments::Length { .. } => "expression:array-length",
            ArrayConstructionArguments::Copy { .. } => "expression:array-copy",
            ArrayConstructionArguments::Indexed(_) => "expression:array-indexed",
            ArrayConstructionArguments::Elements(_) => "expression:array-elements",
        },
        Expression::Call(expression) => match expression.arguments {
            CallArguments::Ordinary(_) => "expression:call-ordinary",
            CallArguments::Copy { .. } => "expression:call-copy",
        },
        Expression::Grouped(_) => "expression:grouped",
        Expression::SelfValue(_) => "expression:self",
        Expression::MemberAccess(_) => "expression:member-access",
        Expression::BracketProjection(expression) => match expression.bounds {
            BracketProjectionBounds::Index(_) => "expression:bracket-index",
            BracketProjectionBounds::Slice { .. } => "expression:bracket-slice",
        },
    }
}
