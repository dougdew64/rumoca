model BouncingBall "A ball bouncing under gravity — the archetypal hybrid model (state event + reinit)"
  // purpose: Events / hybrid structure — a state event (contact) with reinit; equation set is smooth between bounces.
  parameter Real g = 9.81 "gravity";
  parameter Real e = 0.8 "coefficient of restitution";
  Real h(start = 1.0) "height";
  Real v(start = 0.0) "velocity";
equation
  der(h) = v;
  der(v) = -g;
  when h <= 0 then
    reinit(v, -e * pre(v));
  end when;
  annotation(experiment(StartTime = 0, StopTime = 3, Interval = 0.001, Tolerance = 1e-6));
end BouncingBall;
