use crate::{mir::verify_preliminary_mir, test_support::lower_generic_source_to_preliminary_mir};

use super::*;
use crate::passes::static_lifecycle::{plan_static_lifetimes, verify_planned_mir};

const SOURCE: &str = "
class State {
    static explicit: i64 = 41;
    static zero: i64;
    static unused: i64 = 7;
    init() {}
}
fn main() -> i64 {
    State.zero = 1;
    return State.explicit + State.zero;
}
";

#[test]
fn verified_inspection_exposes_typed_facts_and_complete_stable_dump() {
    let verified = planned(SOURCE);
    let inspection = StaticActivationInspection::new(&verified);
    let statistics = inspection.planned().activation_statistics();

    assert_eq!(
        inspection.label(),
        StaticActivationInspectionLabel::VerifiedPlanning
    );
    assert_eq!(inspection.label().to_string(), "verified-static-activation");
    assert_eq!(statistics.declared_fields(), 3);
    assert_eq!(statistics.active_fields(), 2);
    assert_eq!(statistics.inactive_fields(), 1);
    assert_eq!(statistics.active_explicit_fields(), 1);
    assert_eq!(statistics.active_zero_default_fields(), 1);
    assert_eq!(statistics.inactive_explicit_fields(), 1);

    let dump = inspection.activation_dump();
    assert_eq!(
        dump,
        r#"StaticActivationAnalysis
  Summary declared=3 active=2 inactive=1 execution=2 edges=4 accesses=3 dependencies=0 initializers=1 destructions=0
  ConservativeTargets
    Initializer 1
  ActiveFields
    Field c0:static0 (State.explicit)
      Root callable f0 @112..192
      Via callable f0 -> c0:static0 (State.explicit) StaticAccess { access: Read, phase: Ordinary } @162..176
      Target c0:static0 (State.explicit) -> callable c0:static0:initializer Initializer @40..44
    Field c0:static1 (State.zero)
      Root callable f0 @112..192
      Via callable f0 -> c0:static1 (State.zero) StaticAccess { access: Read, phase: Ordinary } @179..189
  InactiveFields
    Field c0:static2 (State.unused)
  ReachableExecution
    Node callable f0
      Root callable f0 @112..192
      Target callable f0 -> c0:static0 (State.explicit) StaticAccess { access: Read, phase: Ordinary } @162..176
      Target callable f0 -> c0:static1 (State.zero) StaticAccess { access: Read, phase: Ordinary } @179..189
      Target callable f0 -> c0:static1 (State.zero) StaticAccess { access: Write, phase: Ordinary } @135..150
    Node callable c0:static0:initializer
      Root callable f0 @112..192
      Via callable f0 -> c0:static0 (State.explicit) StaticAccess { access: Read, phase: Ordinary } @162..176
      Via c0:static0 (State.explicit) -> callable c0:static0:initializer Initializer @40..44
  ActivationOrder
    Field c0:static0 (State.explicit)
    Field c0:static1 (State.zero)
  ShutdownOrder
    Field c0:static1 (State.zero)
    Field c0:static0 (State.explicit)
"#
    );
    assert_eq!(
        dump,
        StaticActivationInspection::new(&verified).activation_dump()
    );
}

#[test]
fn inspection_does_not_change_verified_planning_product() {
    let verified = planned(SOURCE);
    let before = verified.clone();
    let inspection = StaticActivationInspection::new(&verified);

    let _ = inspection.label();
    let _ = inspection.planned().activation_statistics();
    let _ = inspection.activation_dump();

    assert_eq!(verified, before);
}

fn planned(source: &str) -> VerifiedPlannedMirProgram {
    let preliminary = lower_generic_source_to_preliminary_mir(source);
    verify_preliminary_mir(&preliminary).unwrap();
    verify_planned_mir(plan_static_lifetimes(preliminary).unwrap()).unwrap()
}
