model UndefinedRef "References a name that does not exist — DELIBERATELY BROKEN"
  // purpose: DO NOT FIX. Diagnostic specimen for the RESOLVE failure path — a
  // reference to an undeclared name, the most basic authoring mistake there is.
  //
  // Breaks at: RESOLVE. `missingGain` is never declared, so name resolution fails
  // before instantiation is attempted.
  Real y;
equation
  y = missingGain * time;
end UndefinedRef;
