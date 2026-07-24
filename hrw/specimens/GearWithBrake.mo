model GearWithBrake "Geared oscillator with automatic speed-limiting brake"
  // purpose: end-to-end tour specimen — exercises every compiler phase: MSL connectors, index reduction, events, stiff dynamics.
  //
  // Physics: a constant torque drives a rotor through an ideal gear (5:1) to a
  // load inertia restrained by a rotational spring-damper anchored to a fixed
  // frame. The system accelerates and oscillates. When the load speed exceeds a
  // threshold, a braking torque engages (via a when clause controlling a Torque
  // source); the brake releases when speed drops below half the threshold,
  // producing a limit-cycle with discrete braking events.
  //
  // What each compiler phase sees:
  //   Parse/Resolve    — MSL component types, connectors, parameters
  //   Instantiate      — parameter modifications (J, ratio, c, d)
  //   Typecheck        — SI unit types, array dimensions
  //   Flatten          — connector expansion: rotational flanges (phi, tau)
  //                      with potential/flow semantics produce equality +
  //                      flow-sum equations
  //   DAE construction — states (phi, w), algebraics (gear constraint forces),
  //                      parameters, residual-form equations
  //   Index reduction  — the ideal gear constraint phi_a = ratio * phi_b is
  //                      position-level, creating index > 1; Pantelides
  //                      differentiates it, introducing dummy derivatives
  //   Structural       — structurally singular before reduction (expected for
  //                      high-index); nontrivial matching and BLT after
  //   Initialization   — start values must satisfy the gear constraint
  //   Events           — when/elsewhen for brake engagement/release
  //   Solve/Simulate   — stiff spring + gear constraint favours BDF

  // ---- Components (MSL 4.1.0 rotational) ----
  Modelica.Mechanics.Rotational.Sources.ConstantTorque motor(tau_constant = 2.0);
  Modelica.Mechanics.Rotational.Components.Inertia rotor(J = 0.01);
  Modelica.Mechanics.Rotational.Components.IdealGear gear(ratio = 5);
  Modelica.Mechanics.Rotational.Components.Inertia load(J = 0.5);
  Modelica.Mechanics.Rotational.Components.SpringDamper spring(c = 100, d = 2);
  Modelica.Mechanics.Rotational.Components.Fixed frame;
  Modelica.Mechanics.Rotational.Sources.Torque brakeTorque;

  // ---- Brake control ----
  parameter Real maxSpeed = 5.0 "Load speed threshold for brake engagement [rad/s]";
  parameter Real brakeForce = 20.0 "Braking torque magnitude [N.m]";
  Boolean braking(start = false) "True when brake is engaged";

equation
  // Power transmission chain.
  connect(motor.flange, rotor.flange_a);
  connect(rotor.flange_b, gear.flange_a);
  connect(gear.flange_b, load.flange_a);

  // Load restrained by spring-damper to fixed frame.
  connect(load.flange_b, spring.flange_a);
  connect(spring.flange_b, frame.flange);

  // Brake torque applied to load.
  connect(brakeTorque.flange, load.flange_b);

  // Brake engagement logic (discrete events).
  when load.w > maxSpeed then
    braking = true;
  elsewhen load.w < maxSpeed * 0.5 then
    braking = false;
  end when;

  // Braking torque opposes load motion when engaged.
  brakeTorque.tau = if braking then
    (if load.w > 0 then -brakeForce else brakeForce)
  else 0;

  annotation(experiment(StartTime = 0, StopTime = 5, Interval = 0.001, Tolerance = 1e-6));
end GearWithBrake;
