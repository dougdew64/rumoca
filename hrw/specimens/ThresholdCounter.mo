model ThresholdCounter "Counts how often an oscillation crosses a threshold, upward and either way"
  // purpose: Events — `edge` and `change` are not primitives. Each expands into `pre`:
  // `edge(above)` becomes `above and not pre(above)` and `change(above)` becomes
  // `above <> pre(above)`, and the Events pane shows the expanded algebra.
  //
  // One relation, `x > threshold`, drives both counters, so the two `when` clauses differ
  // ONLY in the operator: `rises` counts upward crossings (`edge`), `crossings` counts every
  // crossing (`change`). With x = cos(10 t) and a threshold of 0.5, the crossings before
  // t = 1 fall at about t = 0.105 (down), 0.524 (up) and 0.733 (down), so a run to t = 1 ends
  // with rises = 1 and crossings = 3, and a run to t = 2 (the notebook trace's horizon) with
  // rises = 3 and crossings = 7. Both counters are 0 at t = 0: `x` starts above the threshold,
  // so `above` is true from the start and nothing has changed.
  //
  // **Rumoca 0.10.4 disagrees, and System Modeler is the oracle.** System Modeler 15.0
  // reproduces the counts above exactly. Rumoca starts BOTH counters at 1 and ends one
  // high (4 and 8 at t = 2): it takes `pre(above)` at the initial instant from `above`'s
  // default start (`false`) instead of from its initialized value (`true`), so both `when`
  // clauses fire during initialization. `docs/upstream-issues.md` D3. The Events pane's
  // expansion is right; the run's numbers are not.
  //
  // Why that needs a specimen of its own: until this one only `BouncingBall` used `pre`, and
  // no curated specimen used `edge` or `change`, so the expansion the Events pane exists to
  // show was covered only by a scratch model in a test.
  parameter Real omega = 10 "angular frequency";
  parameter Real threshold = 0.5 "level the position is compared against";
  Real x(start = 1, fixed = true) "position";
  Real v(start = 0, fixed = true) "velocity";
  Boolean above "the position is above the threshold";
  Integer rises(start = 0, fixed = true) "upward crossings, counted with edge";
  Integer crossings(start = 0, fixed = true) "crossings in either direction, counted with change";
equation
  der(x) = v;
  der(v) = -omega ^ 2 * x;
  above = x > threshold;
  when edge(above) then
    rises = pre(rises) + 1;
  end when;
  when change(above) then
    crossings = pre(crossings) + 1;
  end when;
  annotation(experiment(StartTime = 0, StopTime = 1, Interval = 0.001, Tolerance = 1e-6));
end ThresholdCounter;
