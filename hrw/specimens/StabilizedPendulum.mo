model StabilizedPendulum "The Cartesian pendulum with its constraint differentiated by hand and damped (Baumgarte, 1972)"
  // purpose: Index reduction performed BY THE MODELLER — the same physics as CartesianPendulum, written so it reaches the solver already at index 1.
  parameter Real L = 1.0 "rod length";
  parameter Real m = 1.0 "point mass";
  parameter Real g = 9.81 "gravitational acceleration";
  parameter Real alpha = 10.0 "constraint damping gain (velocity level)";
  parameter Real beta = 10.0 "constraint stiffness gain (position level)";
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
  // The position constraint x^2 + y^2 = L^2, differentiated twice and fed back:
  // ddot(C) + 2*alpha*dot(C) + beta^2*C = 0, which is index 1 in lambda.
  2 * (vx ^ 2 + vy ^ 2) + 2 * (x * der(vx) + y * der(vy)) + 4 * alpha * (x * vx + y * vy) + beta ^ 2 * (x ^ 2 + y ^ 2 - L ^ 2) = 0;
end StabilizedPendulum;
