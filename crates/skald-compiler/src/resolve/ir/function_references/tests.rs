use crate::{
    identity::{CallableId, FunctionId, FunctionTypeId},
    source::{SourceDatabase, Span},
};

use super::ResolvedAddressTakenCallableTable;

#[test]
fn address_taken_targets_are_unique_and_sorted_by_exact_identity() {
    let span = span();
    let first = CallableId::Function(FunctionId::new(0));
    let second = CallableId::Function(FunctionId::new(1));
    let mut table = ResolvedAddressTakenCallableTable::default();

    table.record(second, FunctionTypeId::new(1), span);
    table.record(first, FunctionTypeId::new(0), span);
    table.record(second, FunctionTypeId::new(1), span);

    assert_eq!(
        table.iter().map(|entry| entry.target).collect::<Vec<_>>(),
        vec![first, second]
    );
}

fn span() -> Span {
    let mut sources = SourceDatabase::new();
    Span::empty(sources.add("function-references.ska", ""), 0)
}
