model OverInitRc "RC with over-specified (conflicting) initial conditions — an initialization blow-up"
  // purpose: Initialization blow-up — conflicting initial equations over-determine the capacitor state.
  Modelica.Electrical.Analog.Sources.ConstantVoltage src(V = 5);
  Modelica.Electrical.Analog.Basic.Resistor R(R = 100);
  Modelica.Electrical.Analog.Basic.Capacitor C(C = 1e-3);
  Modelica.Electrical.Analog.Basic.Ground gnd;
equation
  connect(src.p, R.p);
  connect(R.n, C.p);
  connect(C.n, src.n);
  connect(src.n, gnd.p);
initial equation
  C.v = 0;
  der(C.v) = 0;
end OverInitRc;
