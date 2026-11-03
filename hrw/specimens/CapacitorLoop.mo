model CapacitorLoop "A capacitor directly across an ideal voltage source — an ill-posed initialization (RC blow-up)"
  // purpose: RC initialization blow-up — a capacitor across an ideal source over-constrains its (state) voltage.
  Modelica.Electrical.Analog.Sources.ConstantVoltage src(V = 5);
  Modelica.Electrical.Analog.Basic.Capacitor C(C = 1e-3);
  Modelica.Electrical.Analog.Basic.Ground gnd;
equation
  connect(src.p, C.p);
  connect(src.n, C.n);
  connect(src.n, gnd.p);
  annotation(experiment(StartTime = 0, StopTime = 1, Interval = 0.001, Tolerance = 1e-6));
end CapacitorLoop;
