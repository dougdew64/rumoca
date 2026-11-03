model ProportionalLoop "Idealized proportional servo inner loop — a purely algebraic feedback loop"
  // purpose: Idealized algebraic feedback loop → one coupled BLT block (tearing).
  // Arc 3 specimen (charter §4.2.3): an ideal proportional feedback loop closed
  // around *instantaneous* relations. A real servo inner loop integrates (the
  // inertia is a state), which breaks the loop into an ODE. Here the dynamics
  // are idealized away — every relation is algebraic — so the feedback closes on
  // itself with no integrator to cut it: error → command → measurement → error.
  //
  // Structurally that cycle is a single strongly-connected component: a genuine
  // *simultaneous algebraic block* (a coupled BLT block, size 3), the object of
  // study for this arc. It is not analytically hard (one linear solve), but it
  // is the smallest thing that forces matching + Tarjan to report a coupled SCC
  // and tearing to pick an iteration variable — which is exactly what makes the
  // spy-plot show an orange box instead of only diagonal cells.
  parameter Real reference = 1.0 "setpoint";
  parameter Real controllerGain = 10.0 "proportional gain Kp";
  parameter Real plantGain = 2.0 "ideal (static) plant gain — no integrator";
  Real error "reference − measurement";
  Real command "controller output = Kp · error";
  Real measurement "plant output, fed back to the summing junction";
equation
  error = reference - measurement;
  command = controllerGain * error;
  measurement = plantGain * command;
  annotation(experiment(StartTime = 0, StopTime = 1, Interval = 0.001, Tolerance = 1e-6));
end ProportionalLoop;
