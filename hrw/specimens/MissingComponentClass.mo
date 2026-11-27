model MissingComponentClass "Declares a component of a class that does not exist — DELIBERATELY BROKEN"
  // purpose: DO NOT FIX. Diagnostic specimen for the INSTANTIATE failure path —
  // a component whose *type* cannot be found, as opposed to a reference to an
  // undeclared *variable*, which is `UndefinedRef` and stops at RESOLVE.
  //
  // Breaks at: RESOLVE, which fails; every later tab reports not reached or no result.
  // The first draft of this comment claimed INSTANTIATE, on the theory that a missing
  // *class* is a different problem from a missing *variable*. It is not, to Rumoca:
  // both are name resolution, and both fail at Resolve. (2026-08-05 measured "Resolve
  // reports, Flatten stops"; that was HRW painting Resolve's stop as a flag, and the
  // corpus outcome matrix now pins Resolve as the one failed stage.)
  //
  // What genuinely differs is the resolve diagnostic, and that is the lesson:
  //   UndefinedRef           -> "unresolved component reference"
  //   MissingComponentClass  -> "unresolved type reference"
  // Modelica looks up a class and a component differently, so the compiler can
  // say *which kind* of name it could not find even though the outcome is the
  // same. Keep both: the pair is what makes the distinction visible.
  NoSuchBlock part;
  Real y;
equation
  y = time;
end MissingComponentClass;
