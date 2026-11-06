model ArrayFamily "A regular array equation family — the StructuredScalar compaction"
  // purpose: reach the BLT shape the rest of the corpus does not. A `for` equation over
  // an array produces a *regular equation family*: eight rows that are each their own
  // scalar block, which v0.10.0's structural report compacts into ONE
  // `BlockReport::StructuredScalar` carrying counts instead of eight separate lines.
  //
  // Why that needs a specimen of its own: `fidelity::check_f4` asserts the BLT blocks
  // PARTITION the equations, and a compacted family names no individual equation. From
  // v0.10.0 until 2026-11-06, F4 therefore reported every such block as "names no
  // equations" *and* counted the family's rows as holes in the partition — 24 false
  // violations across 20 MSL models, and simultaneously unable to detect a real
  // partition defect in any model containing a family.
  //
  // It went unnoticed for two releases because **the curated specimens did not reach
  // this shape**, so only the hours-long MSL sweep exercised it. This specimen moves
  // that coverage into the pre-commit suite, where a regression is found in seconds
  // rather than at the next corpus run.
  //
  // Deliberately the simplest model that produces the shape: one family, eight rows,
  // nothing else. The structural report should hold exactly one block, with
  // `first_row = 0` and `row_count = 8` against an 8-equation incidence — so the
  // partition is complete and F4 must be quiet.
  Real x[8](each start = 1.0);
equation
  for i in 1:8 loop
    der(x[i]) = -x[i];
  end for;
end ArrayFamily;
