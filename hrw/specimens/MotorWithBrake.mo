model MotorWithBrake "DC motor with speed-limit events"
  // purpose: end-to-end tour specimen — exercises every compiler phase: MSL connectors, index reduction (EMF coupling), events (when/elsewhen speed limit), stiff dynamics (fast L/R + slow J).
  //
  // Physics: a constant-voltage DC source drives current through a resistor and
  // inductor into a rotational EMF, spinning up an inertial load. The load
  // accelerates from rest toward the motor's back-EMF-limited steady-state speed.
  // A when/elsewhen pair tracks whether speed exceeds a threshold, toggling a
  // discrete Boolean — the compiler's Events phase sees this as a zero-crossing
  // condition with discrete state update.
  //
  // What each compiler phase sees:
  //   Parse/Resolve    — MSL component types, connectors, parameters
  //   Instantiate      — parameter modifications (R, L, k, J, V)
  //   Typecheck        — SI unit types
  //   Flatten          — connector expansion: electrical pins (v, i) and
  //                      rotational flanges (phi, tau) with potential/flow
  //                      semantics produce equality + flow-sum equations
  //   DAE construction — states (i, phi, w), algebraics (constraint forces),
  //                      parameters, residual-form equations
  //   Index reduction  — the EMF's internal support creates a position-level
  //                      constraint, producing index > 1; Pantelides demotes it
  //   Structural       — structurally singular before reduction (expected for
  //                      high-index); nontrivial matching and BLT after
  //   Initialization   — start values for electrical + mechanical states
  //   Events           — when/elsewhen for speed-limit detection
  //   Solve/Simulate   — stiff: fast electrical (L/R ~ 1e-4 s) coupled to
  //                      slow mechanical (J/k^2 ~ 5 s); BDF required

  // ---- Electrical drive ----
  Modelica.Electrical.Analog.Sources.ConstantVoltage src(V = 12);
  Modelica.Electrical.Analog.Basic.Resistor R(R = 1.0);
  Modelica.Electrical.Analog.Basic.Inductor L(L = 1e-4, i(start = 0)) "small L -> fast electrical time constant";
  Modelica.Electrical.Analog.Basic.RotationalEMF emf(k = 0.1);
  Modelica.Electrical.Analog.Basic.Ground gnd;

  // ---- Mechanical load ----
  Modelica.Mechanics.Rotational.Components.Inertia load(J = 0.05, phi(start = 0), w(start = 0));

  // ---- Speed-limit detection ----
  parameter Real maxSpeed = 30.0 "Speed threshold [rad/s]";
  Boolean overSpeed(start = false) "True when load exceeds threshold";

equation
  // Electrical circuit: source -> R -> L -> EMF -> ground.
  connect(src.p, R.p);
  connect(R.n, L.p);
  connect(L.n, emf.p);
  connect(emf.n, src.n);
  connect(src.n, gnd.p);

  // EMF drives the load.
  connect(emf.flange, load.flange_a);

  // Speed-limit detection (discrete events).
  when load.w > maxSpeed then
    overSpeed = true;
  elsewhen load.w < maxSpeed * 0.5 then
    overSpeed = false;
  end when;

  annotation(experiment(StartTime = 0, StopTime = 0.5, Interval = 0.0005, Tolerance = 1e-6));
end MotorWithBrake;
