model StiffDecay "Two decaying modes whose rates differ by a factor of a thousand"
  // purpose: Stiffness at its smallest — the fast mode is numerically dead by t = 0.01 yet would dictate an explicit method's step size for the whole run.
  parameter Real slowRate = 1.0 "decay rate of the mode that carries the answer";
  parameter Real fastRate = 1000.0 "decay rate of the mode that vanishes almost at once";
  Real slow(start = 1.0) "the mode that carries the answer";
  Real fast(start = 1.0) "the mode that is gone almost immediately";
equation
  der(slow) = -slowRate * slow;
  der(fast) = -fastRate * fast;
end StiffDecay;
