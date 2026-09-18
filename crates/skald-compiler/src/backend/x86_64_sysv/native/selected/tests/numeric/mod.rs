use super::oracle::{run, Value};
use super::*;
use crate::backend::failure::FailureMessage;
use crate::backend::x86_64_sysv::native::selected::numeric::Numeric;
use skald_binary64::{Binary64, IntegerConversion};

fn evaluate(source: &str, cases: &[(Vec<Value>, Result<Value, FailureMessage>)]) {
    let mut tested = 0;
    for_sources(source, |context, lower| {
        let body = select(context, lower).unwrap();
        let mut arity = 0;
        body.visit(|fact| {
            if let SelectedFact::Entry { inputs, .. } = fact {
                arity = inputs.len();
            }
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap();
        if arity == 0 {
            return;
        }
        tested += 1;
        for (inputs, expected) in cases {
            assert_eq!(run(&body, inputs), *expected, "{source}, {inputs:?}");
        }
    });
    assert_eq!(tested, 1, "fixture must exercise its numeric callable");
}

mod editing;
mod references;
mod resources;
