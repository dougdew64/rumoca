model OverDeterminedShaft "A shaft with one equation too many — DELIBERATELY BROKEN"
  // purpose: DO NOT FIX. The over-determined half of the balance check — 3 equations, 2 unknowns.
  //
  // The mirror of `UnbalancedShaft`, which is short one equation and reports
  // `balance = -1`. This one is long one equation and reports `balance = +1`, so
  // the sign in that message becomes something a reader can test rather than a
  // claim they have to accept. Until this file existed, every unbalanced specimen
  // in the corpus reported -1 and half of `dae-construction.md` Act 5 was prose.
  //
  // Breaks at: DAE CONSTRUCTION, the same phase and the same check as
  // `UnbalancedShaft` — MLS §4.9, equations must equal unknowns.
  //
  // **The extra equation is deliberately CONSISTENT, not contradictory**, and that
  // is the whole design of this specimen. `w = der(phi)` says exactly what
  // `der(phi) = w` already said, so there is no conflict to find — and the
  // compiler rejects the model anyway, because the balance check is arithmetic on
  // counts and never asks whether the surplus equation agrees. A contradictory
  // third equation would be rejected too, and would confuse two lessons: "one
  // equation too many" and "these equations disagree".
  parameter Real J = 1.0 "moment of inertia";
  parameter Real tau = 1.0 "applied torque";
  Real phi(start = 0.0) "angle";
  Real w(start = 0.0) "angular velocity";
equation
  der(phi) = w;
  J * der(w) = tau;
  w = der(phi);
  annotation(experiment(StartTime = 0, StopTime = 1, Interval = 0.001, Tolerance = 1e-6));
end OverDeterminedShaft;
