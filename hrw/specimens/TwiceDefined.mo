model TwiceDefined "Two equations define one variable, leaving another undetermined — DELIBERATELY BROKEN"
  // purpose: DO NOT FIX. Diagnostic specimen for the STRUCTURAL ANALYSIS failure
  // path — a system that is square by *count* and singular by *structure*.
  //
  // Breaks at: STRUCTURAL ANALYSIS flags it singular; the compile then fails at SOLVE
  // LOWERING. Two equations and two unknowns, so the MLS §4.9 balance check passes and
  // DAE construction succeeds — this is NOT `UnbalancedShaft`, which fails the count.
  // Here both equations mention only `a` (and after Simplify folds `a = 1` into the
  // second, the Structural tab shows it as `0 = 1 - time`, mentioning nothing), so
  // maximum matching can pair at most one of them with `a` and `b` is reachable from
  // nothing. Structural rank 1 < 2.
  //
  // This is the specimen that shows why counting is not enough, which is the
  // whole reason matching exists as a phase.
  Real a;
  Real b;
equation
  a = 1.0;
  a = time;
end TwiceDefined;
