model DimensionMismatch "Assigns a 3-vector to a 2-vector — DELIBERATELY BROKEN"
  // purpose: DO NOT FIX. Diagnostic specimen for the TYPECHECK failure path — an
  // array dimension mismatch, which typecheck catches by evaluating dimensions
  // across the instantiated model.
  //
  // Breaks at: TYPECHECK. `small` has 2 elements and `big` has 3, so the equation
  // relating them is dimensionally inconsistent.
  Real small[2];
  Real big[3];
equation
  small = big;
  big = {1.0, 2.0, 3.0};
end DimensionMismatch;
