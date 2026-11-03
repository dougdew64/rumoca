model BenchActuator "A DC motor spinning up an inertial load — the canonical STIFF pairing (fast electrical, slow mechanical)"
  // purpose: Stiffness — fast winding L/R dynamics coupled to a slow rotor inertia; BDF's reason to exist.
  Modelica.Electrical.Analog.Sources.ConstantVoltage src(V = 12);
  Modelica.Electrical.Analog.Basic.Resistor R(R = 1.0);
  Modelica.Electrical.Analog.Basic.Inductor L(L = 1e-4, i(start = 0)) "small L -> fast electrical time constant (~1e-4 s)";
  Modelica.Electrical.Analog.Basic.RotationalEMF emf(k = 0.1);
  Modelica.Mechanics.Rotational.Components.Inertia load(J = 0.05, phi(start = 0), w(start = 0)) "large J -> slow mechanical";
  Modelica.Electrical.Analog.Basic.Ground gnd;
equation
  connect(src.p, R.p);
  connect(R.n, L.p);
  connect(L.n, emf.p);
  connect(emf.n, src.n);
  connect(src.n, gnd.p);
  connect(emf.flange, load.flange_a);
  annotation(experiment(StartTime = 0, StopTime = 0.5, Interval = 0.0005, Tolerance = 1e-6));
end BenchActuator;
