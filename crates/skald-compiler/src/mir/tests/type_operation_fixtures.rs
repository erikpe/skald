use super::*;

pub(super) fn type_operation_mir() -> MirProgram {
    lower_text(
        "class Base { init() {} }\n\
         class Derived extends Base { init() { super(); } }\n\
         class Other { init() {} }\n\
         fn take(ref value: Derived) -> unit {}\n\
         fn inspect(ref erased: Obj) -> bool {\n\
           var result: bool = erased is Derived;\n\
           take((Derived) erased);\n\
           return result;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    )
}
