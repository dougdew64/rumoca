model SingleInertia "A single rotating inertia driven by an ideal torque source"
  parameter Real J = 1.0 "moment of inertia";
  parameter Real tau = 1.0 "applied torque";
  Real phi(start = 0.0) "angle";
  Real w(start = 0.0) "angular velocity";
equation
  der(phi) = w;
  J * der(w) = tau;
end SingleInertia;
