model LoopWithInertia "Proportional servo loop closed around a real inertia — an algebraic loop inside a dynamic system"
  // purpose: A coupled BLT block AND a state in one model — tearing and integration together.
  // The companion to ProportionalLoop, which idealizes the dynamics away and says
  // so in its own comment: "A real servo inner loop integrates (the inertia is a
  // state), which breaks the loop into an ODE." That is true only when the sensor
  // is ideal. Here the inertia is restored *and* the loop survives, because the
  // sensor also picks up the commanded torque — a real effect in strain-gauge
  // torque sensing, where the gauge sees the drive as well as the motion.
  //
  // The cycle is command → measurement → error → command, exactly as in
  // ProportionalLoop, but now `w` is integrated alongside it. So the compiler must
  // do both jobs on one model: tear a simultaneous algebraic block, and hand a
  // state to the integrator.
  //
  // Every other specimen does one or the other. Measured 2026-08-16: the ten
  // specimens with derivatives had zero coupled blocks, and the four with coupled
  // blocks had zero derivatives — so any check needing both features found one of
  // them missing whichever specimen it chose, and its assertions about the other
  // silently ran zero times. This model exists to close that gap, and
  // `doc_citations::the_corpus_covers_every_feature_the_checkers_need` fails if it
  // ever stops exhibiting both.
  parameter Real reference = 1.0 "commanded angular velocity";
  parameter Real controllerGain = 10.0 "proportional gain Kp";
  parameter Real inertia = 0.5 "rotational inertia J";
  parameter Real sensorFeedthrough = 0.1 "how much of the drive torque the sensor sees";
  Real w(start = 0.0) "angular velocity — the state";
  Real error "reference − measurement";
  Real command "controller output torque";
  Real measurement "what the sensor reports";
equation
  inertia * der(w) = command;
  measurement = w + sensorFeedthrough * command;
  error = reference - measurement;
  command = controllerGain * error;
  annotation(experiment(StartTime = 0, StopTime = 1, Interval = 0.001, Tolerance = 1e-6));
end LoopWithInertia;
