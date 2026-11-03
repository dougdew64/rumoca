model CartesianPendulum "A point mass on a rigid rod, written in Cartesian coordinates"
  // purpose: The canonical index-3 DAE — a NONLINEAR constraint that substitution cannot remove, so differentiation is the only route to index 1.
  parameter Real L = 1.0 "rod length";
  parameter Real m = 1.0 "point mass";
  parameter Real g = 9.81 "gravitational acceleration";
  Real x(start = 1.0) "horizontal position";
  Real y(start = 0.0) "vertical position";
  Real vx(start = 0.0) "horizontal velocity";
  Real vy(start = 0.0) "vertical velocity";
  Real lambda "rod tension per unit length — the constraint force";
equation
  der(x) = vx;
  der(y) = vy;
  m * der(vx) = -lambda * x;
  m * der(vy) = -lambda * y - m * g;
  x ^ 2 + y ^ 2 = L ^ 2;
end CartesianPendulum;
