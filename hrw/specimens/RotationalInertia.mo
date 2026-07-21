model RotationalInertia "single inertia driven by an ideal torque source, via MSL"
  // purpose: Same physics as SingleInertia via MSL connectors — connector expansion; still index-1 (12 scalar blocks).
  Modelica.Mechanics.Rotational.Components.Inertia inertia(J = 1.0);
  Modelica.Mechanics.Rotational.Sources.Torque torque;
  Modelica.Blocks.Sources.Constant tau(k = 1.0);
equation
  connect(tau.y, torque.tau);
  connect(torque.flange, inertia.flange_a);
end RotationalInertia;
