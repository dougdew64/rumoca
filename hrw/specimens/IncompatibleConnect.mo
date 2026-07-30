connector PinA "Two members"
  Real v;
  flow Real i;
end PinA;

connector PinB "One member — deliberately not matching PinA"
  Real v;
end PinB;

model IncompatibleConnect "Connects two structurally different connectors — DELIBERATELY BROKEN"
  // purpose: DO NOT FIX. Diagnostic specimen for the FLATTEN failure path —
  // connecting connectors whose member sets differ, which strict connection
  // validation rejects (MLS §9.3).
  //
  // Breaks at: FLATTEN. PinA has `v` and a flow `i`; PinB has only `v`. The two
  // cannot be connected.
  PinA a;
  PinB b;
equation
  connect(a, b);
end IncompatibleConnect;
