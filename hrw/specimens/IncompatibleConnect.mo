model IncompatibleConnect "Connects two structurally different connectors — DELIBERATELY BROKEN"
  // purpose: DO NOT FIX. Diagnostic specimen for the FLATTEN failure path — connecting
  // connectors whose member sets differ, which strict connection validation rejects
  // (MLS §9.3).
  //
  // Breaks at: FLATTEN. `PinA` has `v` and a flow `i`; `PinB` has only `v`, so the two
  // cannot be connected.
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
