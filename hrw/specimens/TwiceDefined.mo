model TwiceDefined "Two equations define one variable, leaving another undetermined — DELIBERATELY BROKEN"
  // purpose: DO NOT FIX. Diagnostic specimen for the STRUCTURAL ANALYSIS failure
  // path — a system that is square by *count* and singular by *structure*.
  //
  // Breaks at: STRUCTURAL ANALYSIS. Two equations and two unknowns, so the MLS
  // §4.9 balance check passes and DAE construction succeeds — this is NOT
  // `UnbalancedShaft`, which fails the count. Here both equations mention only
  // `a`, so maximum matching can pair at most one of them with `a` and `b` is
  // reachable from nothing. Structural rank 1 < 2.
  //
  // This is the specimen that shows why counting is not enough, which is the
  // whole reason matching exists as a phase.
  Real a;
  Real b;
equation
  a = 1.0;
  a = time;
end TwiceDefined;
