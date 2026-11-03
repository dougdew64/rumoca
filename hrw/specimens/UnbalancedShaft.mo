model UnbalancedShaft "A shaft whose drive torque no equation determines — DELIBERATELY BROKEN"
  // purpose: DO NOT FIX. Diagnostic specimen for the DAE-construction failure path —
  // a declared variable with no equation to determine it. The most common Modelica
  // authoring error there is: declare a variable, forget its equation.
  //
  // Breaks at: DAE CONSTRUCTION (not structural analysis). Rumoca's balance check
  // catches a missing equation before the matching ever runs, which is earlier and
  // more specific than a structural singularity — good compiler behaviour, and the
  // reason this specimen exists separately from `CapacitorLoop` (which is balanced by
  // count and singular by structure).
  //
  // The bug is `tau` below: declared, never assigned. Every other specimen in this
  // corpus is well-posed apart from the deliberately ill-posed ones; this is one of
  // them, and repairing it would delete the test.
  parameter Real J = 0.5 "moment of inertia";
  Real phi(start = 0) "angle";
  Real w(start = 0) "angular velocity";
  Real tau "drive torque — NO EQUATION DETERMINES THIS, and that is the point";
equation
  der(phi) = w;
  J * der(w) = tau;
end UnbalancedShaft;
