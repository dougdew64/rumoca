model MissingComponentClass "Declares a component of a class that does not exist — DELIBERATELY BROKEN"
  // purpose: DO NOT FIX. Diagnostic specimen for the INSTANTIATE failure path —
  // a component whose *type* cannot be found, as opposed to a reference to an
  // undeclared *variable*, which is `UndefinedRef` and stops at RESOLVE.
  //
  // Breaks at: RESOLVE reports it, FLATTEN stops. Verified 2026-08-05 with
  // `cargo run -p hrw --example failure_map`, which contradicted the first draft
  // of this comment — it claimed INSTANTIATE, on the theory that a missing *class*
  // is a different problem from a missing *variable*. It is not, to Rumoca: both
  // are name resolution, both are recovered at Resolve, and both stop at Flatten
  // with the same message.
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
