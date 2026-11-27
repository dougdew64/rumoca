model MeasuredOscillator "A mass on a spring, its velocity read by a tachometer and smoothed by a first-order filter"
  // purpose: Initialization — three states whose initial values are settled three different
  // ways: by their own `fixed = true` start, by a value TRANSFERRED from a measured alias, and
  // not at all.
  //
  // In the plan Rumoca lowers: `x` states its own fixed start, so it is a GIVEN state and has
  // no row. `v` states nothing, but `v_meas = v` proves the two equal, so the fixed start written
  // on the tachometer reading `v_meas` becomes one `Solved` row for `v`, located at `v_meas`'s
  // declaration. `v_filt` states a bare `start`, which is only a guess: it is in neither list.
  //
  // Why that needs a specimen of its own: until this one, no curated specimen declared
  // `fixed = true`, so the Initialization pane was empty across the corpus. The pane should show
  // `given_states = [x]`, `unknowns = [v]`, and exactly one row. The rows alone would suggest one
  // settled state; `given_states` is why the count cannot be read off them.
  parameter Real m = 1 "mass";
  parameter Real k = 4 "spring stiffness";
  parameter Real T = 0.1 "filter time constant";
  Real x(start = 0.5, fixed = true) "position, fixed by its own declaration";
  Real v "velocity, with no start of its own";
  Real v_meas(start = 1.0, fixed = true) "tachometer reading: the measured velocity, stated at t = 0";
  Real v_filt(start = 0.0) "filtered reading, whose start is only a guess";
equation
  der(x) = v;
  m * der(v) = -k * x;
  v_meas = v;
  T * der(v_filt) = v_meas - v_filt;
  annotation(experiment(StartTime = 0, StopTime = 1, Interval = 0.001, Tolerance = 1e-6));
end MeasuredOscillator;
