model MixedLoop "Algebraic feedback loop bracketed by scalar solves — a mixed BLT structure"
  // Arc 3 specimen (charter §4.2.3): the same idealized proportional loop as
  // ProportionalLoop, but with a scalar computation *before* it (scale the
  // reference) and *after* it (scale the output). Neither bracketing equation is
  // part of the loop, so structural analysis sorts the system into THREE blocks
  // in evaluation order: a scalar source, the coupled 3×3 algebraic loop, then a
  // scalar sink. It is the smallest model whose BLT ordering does visible work —
  // the spy-plot shows green diagonal cells AND an orange coupled box together
  // (the earlier specimens are all-scalar or all-one-loop, never a mix).
  parameter Real reference = 1.0 "raw setpoint";
  parameter Real sensorGain = 0.5 "pre-scaling on the reference";
  parameter Real controllerGain = 10.0 "proportional gain Kp";
  parameter Real plantGain = 2.0 "ideal static plant gain";
  parameter Real outputGain = 3.0 "post-scaling on the measurement";
  Real setpoint "scaled reference — computed before the loop";
  Real error "reference − measurement";
  Real command "controller output = Kp · error";
  Real measurement "plant output, fed back";
  Real result "scaled measurement — computed after the loop";
equation
  setpoint = sensorGain * reference;      // scalar block (source): depends only on a parameter
  error = setpoint - measurement;         // ┐
  command = controllerGain * error;       // ├ coupled 3×3 algebraic loop
  measurement = plantGain * command;      // ┘
  result = outputGain * measurement;      // scalar block (sink): depends only on the loop's result
  annotation(experiment(StartTime = 0, StopTime = 1, Interval = 0.001, Tolerance = 1e-6));
end MixedLoop;
