# Whole-World Reachability Golden Coverage

These tests compile the same closed program with the supported default MIR
pipeline, the empty `none` profile, and the default profile with semantic
reachability pruning disabled. They keep observable startup, entry, reverse
shutdown, ownership, destruction, panic, and runtime-trace behavior identical
while exercising direct, recursive, virtual, interface, and function-value
calls plus optional, shared, array, literal, and unreachable definitions.
