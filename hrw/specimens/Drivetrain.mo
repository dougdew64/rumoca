model Drivetrain "Motor–gearbox–link drivetrain: electrical → rotational → translational"
  // purpose: Ideal gears → high-index, structurally singular DAE (needs index reduction — Arc 4).
  // Arc 2 specimen (charter §4.2.2): an open power-transmission chain that
  // crosses three physical domains, so instantiate/flatten do nontrivial work —
  // connector expansion and flow-sum generation over electrical pins (i),
  // rotational flanges (tau), and translational flanges (f). Portable Modelica
  // subset, MSL 4.1.0 only, no MultiBody. (The 2D planar mechanics — revolute
  // joints, closed-loop linkages — belong to the Matching/BLT and Pantelides arcs.)

  // --- Electrical: a DC source drives the motor windings (R–L) into the EMF. ---
  Modelica.Electrical.Analog.Sources.ConstantVoltage src(V = 12);
  Modelica.Electrical.Analog.Basic.Resistor R(R = 0.5);
  Modelica.Electrical.Analog.Basic.Inductor L(L = 0.05);
  Modelica.Electrical.Analog.Basic.RotationalEMF emf(k = 0.1);
  Modelica.Electrical.Analog.Basic.Ground gnd;

  // --- Rotational: rotor inertia → gearbox (ratio) → output shaft inertia. ---
  Modelica.Mechanics.Rotational.Components.Inertia rotor(J = 1e-4);
  Modelica.Mechanics.Rotational.Components.IdealGear gear(ratio = 5);
  Modelica.Mechanics.Rotational.Components.Inertia shaft(J = 5e-4);

  // --- Rotational → translational: a rack-and-pinion drives the load (the
  //     "link"), sprung to a fixed mount so the chain is well-posed. ---
  Modelica.Mechanics.Rotational.Components.IdealGearR2T r2t(ratio = 200);
  Modelica.Mechanics.Translational.Components.Mass load(m = 2);
  Modelica.Mechanics.Translational.Components.SpringDamper mount(c = 1000, d = 20);
  Modelica.Mechanics.Translational.Components.Fixed wall;
equation
  // Electrical loop.
  connect(src.p, R.p);
  connect(R.n, L.p);
  connect(L.n, emf.p);
  connect(emf.n, src.n);
  connect(src.n, gnd.p);
  // Rotational chain.
  connect(emf.flange, rotor.flange_a);
  connect(rotor.flange_b, gear.flange_a);
  connect(gear.flange_b, shaft.flange_a);
  connect(shaft.flange_b, r2t.flangeR);
  // Translational load.
  connect(r2t.flangeT, load.flange_a);
  connect(load.flange_b, mount.flange_a);
  connect(mount.flange_b, wall.flange);
  annotation(experiment(StartTime = 0, StopTime = 2, Interval = 0.001, Tolerance = 1e-6));
end Drivetrain;
