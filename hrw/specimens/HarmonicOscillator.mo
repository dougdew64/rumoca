model HarmonicOscillator "An undamped harmonic oscillator, whose exact solution is a cosine"
  // purpose: Integrator ORDER made visible — the exact solution is cos(omega*t), so a method's error can be measured rather than assumed.
  parameter Real omega = 1.0 "angular frequency";
  Real x(start = 1.0) "position";
  Real v(start = 0.0) "velocity";
equation
  der(x) = v;
  der(v) = -omega ^ 2 * x;
end HarmonicOscillator;
