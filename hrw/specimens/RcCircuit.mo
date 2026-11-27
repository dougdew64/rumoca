model RcCircuit "A resistor–capacitor circuit — consistent initialization of the capacitor voltage"
  // purpose: Initialization — a well-posed RC whose one state, the capacitor voltage, has only a bare start, so its lowered initialization plan is empty.
  Modelica.Electrical.Analog.Sources.ConstantVoltage src(V = 5);
  Modelica.Electrical.Analog.Basic.Resistor R(R = 100);
  Modelica.Electrical.Analog.Basic.Capacitor C(C = 1e-3);
  Modelica.Electrical.Analog.Basic.Ground gnd;
equation
  connect(src.p, R.p);
  connect(R.n, C.p);
  connect(C.n, src.n);
  connect(src.n, gnd.p);
  annotation(experiment(StartTime = 0, StopTime = 1, Interval = 0.001, Tolerance = 1e-6));
end RcCircuit;
