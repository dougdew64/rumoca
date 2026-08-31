model ScopedConnect "Two resistor segments in series, each wired internally"
  // purpose: Connection scope — `connect` statements declared at TWO hierarchy levels, so
  // potential sets (built in one global union-find) span scopes while flow sets (built per
  // scope) do not. `RcCircuit` cannot show this: all four of its connects sit at root.
  //
  // The shape being demonstrated: `seg1.n` is wired to `seg1.R.n` INSIDE Segment and to
  // `seg2.p` OUTSIDE it. Those two connects are declared at different scopes, which is the
  // whole point — `build_connection_sets` groups flow pairs per scope and potential pairs
  // globally, so the same physical junction is one potential set and more than one flow set.
  //
  // `Segment` is declared INSIDE this model deliberately, not at file scope. Putting a
  // second class at file scope makes the file contain two top-level classes, and the
  // reachable-closure pipeline then returns no result at all — the trap recorded on
  // IncompatibleConnect.mo, which cost that specimen its intended failure once already.
  //
  // Portable Modelica and MSL only, per the specimen rules: no Wolfram extensions.

  model Segment "A resistor behind two pins, with its own internal connects"
    Modelica.Electrical.Analog.Interfaces.Pin p;
    Modelica.Electrical.Analog.Interfaces.Pin n;
    Modelica.Electrical.Analog.Basic.Resistor R(R = 50);
  equation
    // Declared at scope `seg1` / `seg2` — NOT at root.
    connect(p, R.p);
    connect(R.n, n);
  end Segment;

  Modelica.Electrical.Analog.Sources.ConstantVoltage src(V = 5);
  Segment seg1;
  Segment seg2;
  Modelica.Electrical.Analog.Basic.Ground gnd;
equation
  // Declared at root scope.
  connect(src.p, seg1.p);
  connect(seg1.n, seg2.p);
  connect(seg2.n, src.n);
  connect(src.n, gnd.p);
  annotation(experiment(StartTime = 0, StopTime = 1, Interval = 0.001, Tolerance = 1e-6));
end ScopedConnect;
