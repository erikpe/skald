//! Compact identities and source fixtures for activation-analysis tests.

use crate::{
    identity::{ArrayTypeId, ClassId, FunctionId, StaticFieldId, StaticInitializerId},
    mir::{MirArrayLifecycleOperation, MirExecutionNode},
    passes::reachability::MirDependencyEdgeKind,
    source::{SourceDatabase, Span},
};

use super::{
    StaticActivationAnalysis, StaticActivationAnalysisParts, StaticActivationEdge,
    StaticActivationExecution, StaticActivationField, StaticActivationRoot,
    StaticActivationWitness,
};
use crate::mir::{StaticAccessKind, StaticEffectPhase};

pub(super) const DIRECT_ACCESS_SOURCE: &str = "
class State {
    static value: i64 = 1;
    init() {}
}
fn read() -> i64 { return State.value; }
fn write() -> unit { State.value = 2; }
fn borrow(ref value: i64) -> i64 { return value; }
fn borrow_mut(mut ref value: i64) -> unit { value = value + 1; }
fn main() -> i64 {
    write();
    borrow_mut(State.value);
    return read() + borrow(State.value);
}
";

pub(super) const STORED_FAMILY_SOURCE: &str = r#"
from std::str import Str;
class Item {
    value: i64;
    init(value: i64) { self.value = value; }
    copy(ref other: Item) { self.value = other.value; }
}
class State {
    static signed: i64 = 1;
    static unsigned: u64 = 2u;
    static byte: u8 = 3u8;
    static ratio: f64 = 4.0;
    static ready: bool = true;
    static maybe_number: i64? = 5;
    static item: Item = Item(6);
    static maybe_item: Item? = Item(7);
    static owner: shared Item = new Item(8);
    static maybe_owner: shared? Item = new Item(9);
    static values: i64[] = i64[]{10, 11};
    static text: Str = "ready";
    init() {}
}
fn main() -> i64 { return 0; }
"#;

pub(super) const DYNAMIC_AND_INDIRECT_SOURCE: &str = "
class State { static value: i64 = 42; init() {} }
interface Reader { fn read() -> i64; }
class Base implements Reader {
    init() {}
    virtual fn read() -> i64 { return State.value; }
}
class Child extends Base {
    init() { super(); }
    override fn read() -> i64 { return State.value; }
}
fn direct() -> i64 { return State.value; }
fn invoke(callback: fn() -> i64) -> i64 { return callback(); }
fn invoke_virtual(ref value: Base) -> i64 { return value.read(); }
fn invoke_interface(ref value: Reader) -> i64 { return value.read(); }
fn main() -> i64 { return invoke(direct); }
";

pub(super) const IMPLICIT_LIFECYCLE_SOURCE: &str = "
class State {
    static constructed: i64 = 1;
    static copied: i64 = 2;
    static destroyed: i64 = 3;
    init() {}
}
class Item {
    value: i64;
    init() { self.value = State.constructed; }
    copy(ref other: Item) { self.value = State.copied; }
    assign(ref other: Item) { self.value = State.copied; }
    destroy { var observed: i64 = State.destroyed; }
}
fn main() -> i64 {
    var left: Item = Item();
    var right: Item = left;
    left = right;
    var maybe: Item? = Item();
    var items: Item[] = Item[]{Item()};
    return 0;
}
";

pub(super) const INACTIVE_ONLY_DEPENDENCY_SOURCE: &str = "
class State {
    static target: i64 = 1;
    static dormant: i64 = unreachable_read();
    init() {}
}
fn unreachable_read() -> i64 { return State.target; }
fn main() -> i64 { return 0; }
";

pub(super) const SELF_DEPENDENCY_SOURCE: &str = "
class State {
    static value: i64 = State.value;
    init() {}
}
fn main() -> i64 { return 0; }
";

pub(super) const CYCLE_SOURCE: &str = "
fn read_left() -> i64 { return State.left; }
fn read_right() -> i64 { return State.right; }
class State {
    static left: i64 = read_right();
    static right: i64 = read_left();
    init() {}
}
fn main() -> i64 { return 0; }
";

pub(super) struct ActivationIdentityFixture {
    pub(super) entry: MirExecutionNode,
    pub(super) helper: MirExecutionNode,
    pub(super) initializer: MirExecutionNode,
    pub(super) destruction: MirExecutionNode,
    pub(super) active: StaticFieldId,
    pub(super) inactive: StaticFieldId,
    pub(super) spans: [Span; 4],
}

pub(super) fn activation_identity_fixture() -> ActivationIdentityFixture {
    let class = ClassId::new(2);
    let active = StaticFieldId::new(class, 0);
    let inactive = StaticFieldId::new(class, 1);
    ActivationIdentityFixture {
        entry: MirExecutionNode::callable(FunctionId::new(0).into()),
        helper: MirExecutionNode::callable(FunctionId::new(1).into()),
        initializer: MirExecutionNode::callable(StaticInitializerId::from(active).into()),
        destruction: MirExecutionNode::array(
            ArrayTypeId::new(0),
            MirArrayLifecycleOperation::Destruction,
        ),
        active,
        inactive,
        spans: fixture_spans(),
    }
}

pub(super) fn activation_analysis_fixture(reverse: bool) -> StaticActivationAnalysis {
    let fixture = activation_identity_fixture();
    let root = StaticActivationRoot::new(fixture.entry, fixture.spans[0]);
    let helper_edge = StaticActivationEdge::execution_dependency(
        fixture.entry,
        fixture.helper,
        MirDependencyEdgeKind::DirectCall,
        fixture.spans[1],
    );
    let access_edge = StaticActivationEdge::static_access(
        fixture.helper,
        fixture.active,
        StaticAccessKind::Read,
        StaticEffectPhase::Ordinary,
        fixture.spans[2],
    );
    let initializer_edge =
        StaticActivationEdge::initializer(fixture.active, fixture.initializer, fixture.spans[3]);
    let destruction_edge =
        StaticActivationEdge::destruction(fixture.active, fixture.destruction, fixture.spans[3]);
    let entry = StaticActivationExecution::new(
        fixture.entry,
        StaticActivationWitness::new(root, Vec::new()),
    );
    let helper = StaticActivationExecution::new(
        fixture.helper,
        StaticActivationWitness::new(root, vec![helper_edge]),
    );
    let initializer = StaticActivationExecution::new(
        fixture.initializer,
        StaticActivationWitness::new(root, vec![helper_edge, access_edge, initializer_edge]),
    );
    let destruction = StaticActivationExecution::new(
        fixture.destruction,
        StaticActivationWitness::new(root, vec![helper_edge, access_edge, destruction_edge]),
    );
    let active = StaticActivationField::new(
        fixture.active,
        StaticActivationWitness::new(root, vec![helper_edge, access_edge]),
    );
    let mut executions = vec![entry, helper, initializer, destruction];
    let mut edges = vec![helper_edge, access_edge, initializer_edge, destruction_edge];
    if reverse {
        executions.reverse();
        edges.reverse();
    }
    StaticActivationAnalysis::from_parts(StaticActivationAnalysisParts {
        active_fields: vec![active],
        inactive_fields: vec![fixture.inactive],
        reachable_execution: executions,
        edges,
    })
}

fn fixture_spans() -> [Span; 4] {
    let mut sources = SourceDatabase::new();
    let first = sources.add("activation-first.ska", "0123456789");
    let second = sources.add("activation-second.ska", "0123456789");
    [
        Span::empty(first, 1),
        Span::empty(first, 7),
        Span::empty(second, 0),
        Span::empty(second, 3),
    ]
}
