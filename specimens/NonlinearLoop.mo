model NonlinearLoop "Idealized proportional loop closed around a NONLINEAR plant"
  // purpose: Nonlinear plant → same structure as ProportionalLoop, but Newton to solve (structure ≠ numerics).
  // Arc 3 specimen (charter §4.2.3): structurally identical to ProportionalLoop —
  // the same three-equation algebraic loop, one coupled 3×3 block — but the plant
  // relation is nonlinear (`measurement = plantGain · command²`). Structural
  // analysis is blind to the nonlinearity (incidence only asks *which unknowns
  // appear*), so matching / BLT / tearing look the same; the difference shows up
  // *numerically*: once torn to the single iteration variable `command`, the
  // residual `f(command) = 0` is nonlinear, so a real solver must Newton-iterate
  // it. This is the bridge to the simulation/convergence-narrative work (see
  // docs/ideas.md #1): same structure, genuinely different solve.
  parameter Real reference = 1.0 "setpoint";
  parameter Real controllerGain = 10.0 "proportional gain Kp";
  parameter Real plantGain = 2.0 "nonlinear (quadratic) plant coefficient";
  Real error "reference − measurement";
  Real command "controller output = Kp · error";
  Real measurement "plant output, fed back";
equation
  error = reference - measurement;
  command = controllerGain * error;
  measurement = plantGain * command * command;   // nonlinear in the loop variable
  annotation(experiment(StartTime = 0, StopTime = 1, Interval = 0.001, Tolerance = 1e-6));
end NonlinearLoop;
