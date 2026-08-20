# Generic interface goldens

These cases carry closed generic interface applications through the public
compiler, verified final MIR, x86-64 object metadata, and the runtime. They
cover ordinary and generic implementing classes, inherited overrides,
multiple exact applications sharing one method, bound-selected calls, shared
owners and cleanup, structural bracket calls, exact dynamic tests and casts,
and checked dynamic failure. The conformance matrix adds all five primitive
applications, exact-class results, bare-interface aliases, shared
exact/interface/`Obj` arguments, optionals, arrays, nested and multi-parameter
applications, markers, and function-valued requirements. Compile-failure
cases pin complete diagnostics for
raw and malformed applications, recursive specialization, bounds, ambiguity,
claim kinds, duplicate applications, and exact conformance. The module privacy
case pins its stable diagnostic prefix because the provider path is
intentionally canonicalized to an environment-specific absolute path.
