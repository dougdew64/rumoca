model UnclosedModel "Missing its 'end' clause — DELIBERATELY BROKEN"
  // purpose: DO NOT FIX. Diagnostic specimen for the PARSE failure path — the
  // earliest way a file can fail, before any name means anything.
  //
  // Breaks at: PARSE. There is no `end UnclosedModel;`, so the grammar never
  // completes and no AST is produced. Every later phase is unreachable, which
  // makes this the specimen that shows a stage bundle at its emptiest.
  Real x;
equation
  der(x) = -x;
