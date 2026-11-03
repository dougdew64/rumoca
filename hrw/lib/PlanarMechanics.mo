package PlanarMechanics "Hand-built planar (2D) mechanics — portable Modelica subset, no MSL MultiBody"
  // PARKED (2026-07-20): this library is complete and parses, but its intended
  // Arc-4 specimen — a four-bar linkage — is DEFERRED. A planar linkage's
  // loop-closure is a NONLINEAR holonomic constraint (x²+y²=L²), and Rumoca's
  // Rust-path index reduction at pin 8cdc7419 does not reduce that class (verified
  // on the barest Cartesian pendulum; see DECISIONS.md). Arc 4 was reframed around
  // Drivetrain, whose high index comes from *linear* gear constraints (which Rumoca
  // DOES reduce). Un-park when Rumoca gains nonlinear-constraint reduction (a
  // possible upstream contribution) or the four-bar is otherwise pursued.
  //
  // HRW's own small planar mechanics library (charter §4.1/§4.3): mechanical
  // specimens are built from these primitives rather than from MSL MultiBody,
  // which is the most demanding package a young compiler can ingest. Planar (2D)
  // mechanics exhibits every archetype the 3D version does — including the
  // loop-closure constraint that makes a closed kinematic chain an index-3 DAE
  // (Arc 4) — without SO(3) machinery.
  //
  // Written in the portable subset: plain `Real` (no SIunits), scalar equations,
  // no Wolfram- or MSL-specific constructs. Sign conventions follow the standard
  // planar-mechanics libraries: a `Frame` carries absolute position (x, y) and
  // orientation (phi) as potentials, and the cut-force (fx, fy) + cut-torque (t)
  // as flows (which therefore sum to zero across a connection).

  connector Frame "Planar mechanical connection point: absolute pose + cut-load"
    Real x "absolute x position";
    Real y "absolute y position";
    Real phi "absolute orientation angle";
    flow Real fx "cut force, x";
    flow Real fy "cut force, y";
    flow Real t "cut torque about z";
  end Frame;

  model Fixed "Rigid anchor to the inertial frame"
    Frame frame;
    parameter Real x0 = 0 "anchor x";
    parameter Real y0 = 0 "anchor y";
    parameter Real phi0 = 0 "anchor orientation";
  equation
    frame.x = x0;
    frame.y = y0;
    frame.phi = phi0;
    // Reaction force/torque at the anchor is whatever the connection carries;
    // it is left implicit (the flow variables balance through the connect set).
  end Fixed;

  model FixedTranslation "Massless rigid rod: frame_b is a fixed offset from frame_a"
    Frame frame_a;
    Frame frame_b;
    parameter Real L = 1.0 "rod length along frame_a's local x-axis";
  equation
    // Kinematics: frame_b sits length L from frame_a along the body axis, and a
    // rigid rod carries no relative rotation.
    frame_b.x = frame_a.x + L * cos(frame_a.phi);
    frame_b.y = frame_a.y + L * sin(frame_a.phi);
    frame_b.phi = frame_a.phi;
    // Statics (massless): net force zero, net torque about frame_a zero. The
    // moment of frame_b's cut-force about frame_a is r × F with
    // r = (L cos phi, L sin phi).
    frame_a.fx + frame_b.fx = 0;
    frame_a.fy + frame_b.fy = 0;
    frame_a.t + frame_b.t + L * cos(frame_a.phi) * frame_b.fy - L * sin(frame_a.phi) * frame_b.fx = 0;
  end FixedTranslation;

  model Revolute "Ideal frictionless pin joint: one rotational DOF"
    Frame frame_a;
    Frame frame_b;
    parameter Real tau = 0.0 "driving torque about the joint axis (0 = free joint)";
    Real phi(start = 0.0) "relative rotation of frame_b w.r.t. frame_a";
    Real w(start = 0.0) "relative angular velocity";
  equation
    // Coincident pivot; frame_b rotated by the joint angle.
    frame_b.x = frame_a.x;
    frame_b.y = frame_a.y;
    frame_b.phi = frame_a.phi + phi;
    w = der(phi);
    // The pin transmits force; Newton's third law on the cut-force.
    frame_a.fx + frame_b.fx = 0;
    frame_a.fy + frame_b.fy = 0;
    // Torque balance across the joint, and the axis carries only the driving
    // torque `tau` (0 for an ideal free joint).
    frame_a.t + frame_b.t = 0;
    frame_b.t = tau;
  end Revolute;

  model Body "Rigid body: point mass + rotational inertia at its frame"
    Frame frame_a;
    parameter Real m = 1.0 "mass";
    parameter Real I = 0.1 "moment of inertia about the centre of mass";
    parameter Real gx = 0.0 "gravity acceleration, x";
    parameter Real gy = -9.81 "gravity acceleration, y";
    Real vx(start = 0.0) "velocity x";
    Real vy(start = 0.0) "velocity y";
    Real w(start = 0.0) "angular velocity";
  equation
    vx = der(frame_a.x);
    vy = der(frame_a.y);
    w = der(frame_a.phi);
    // Newton–Euler: mass times acceleration = cut-force + gravity; inertia times
    // angular acceleration = cut-torque.
    m * der(vx) = frame_a.fx + m * gx;
    m * der(vy) = frame_a.fy + m * gy;
    I * der(w) = frame_a.t;
  end Body;
end PlanarMechanics;
