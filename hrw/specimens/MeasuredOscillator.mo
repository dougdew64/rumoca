model MeasuredOscillator "A mass on a spring, its velocity read by a tachometer and smoothed by a first-order filter"
  // purpose: Initialization determinacy — three states whose initial values are settled three
  // different ways: by their own `fixed = true` start, by a value TRANSFERRED from a measured
  // alias, and not at all.
  //
  // `x` states its own fixed start, so it is seeded directly and produces no initial-value
  // pin. `v` states nothing, but `v_meas = v` proves the two equal, so the fixed start written
  // on the tachometer reading `v_meas` is transferred onto the state it determines: one
  // `definition` pin about `v`, sourced from `v_meas`. `v_filt` states a bare `start`, which is
  // a guess the initialization solve may move.
  //
  // Why that needs a specimen of its own: until this one, no curated specimen declared
  // `fixed = true`, so the Initialization pane was empty across the corpus and only a scratch
  // model in a test exercised the pin rendering. The pane should show `determined = 2` of
  // `state_scalars = 3`, `only_guessed = [v_filt]`, and exactly one pin. The pin list alone
  // would suggest one determined state; the `x` row is why the count cannot be read off it.
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
