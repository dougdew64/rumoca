model IncompatibleConnect "Connects two structurally different connectors — DELIBERATELY BROKEN"
  // purpose: DO NOT FIX. Diagnostic specimen for the FLATTEN failure path — connecting
  // connectors whose member sets differ, which strict connection validation rejects
  // (MLS §9.3).
  //
  // SHOULD break at: FLATTEN. `PinA` has `v` and a flow `i`; `PinB` has only `v`, so
  // MLS 9.3 makes the connect a type error.
  //
  // ACTUALLY breaks at: STRUCTURAL ANALYSIS ("singular"). Rumoca accepts the connect and
  // the problem only surfaces later, as a misleading structural singularity.
  //
  // Adjudicated by System Modeler 2026-07-29, which REJECTS it:
  //   "Incompatible types. 'a ... 'b' has type 'PinB'."
  // So this specimen is correct and Rumoca has a bug. The validation exists —
  // `validate_type_compatibility` in rumoca-phase-flatten/src/connections/mod.rs — and
  // did not fire for this case. Logged as an upstream issue; see docs/ideas.md #45.
  //
  // Keep this specimen exactly as it is: when Rumoca is fixed, it should start failing at
  // flatten, and that transition is the test.
  //
  // The connectors are declared *inside* the model deliberately. An earlier version put
  // them at file scope, which made the file contain three top-level classes — and the
  // reachable-closure pipeline then returned **no result at all** rather than a flatten
  // failure, so the specimen exercised nothing. One top-level class keeps the failure
  // where the specimen intends it.
  connector PinA "Two members"
    Real v;
    flow Real i;
  end PinA;

  connector PinB "One member — deliberately not matching PinA"
    Real v;
  end PinB;

  PinA a;
  PinB b;
equation
  connect(a, b);
end IncompatibleConnect;
