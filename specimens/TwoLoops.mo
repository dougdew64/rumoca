model TwoLoops "Two algebraic feedback loops in series — two separate coupled blocks"
  // Arc 3 specimen (charter §4.2.3): two idealized proportional loops, where the
  // first loop's command drives the second loop's setpoint. Each loop is its own
  // strongly-connected component, and the second depends on the first but not vice
  // versa — so structural analysis reports TWO coupled blocks (each size 2), one
  // after the other in BLT order, each torn independently. Where ProportionalLoop
  // shows a single orange box, this shows that a system can contain several
  // independent simultaneous blocks scheduled in sequence.
  parameter Real reference = 1.0 "setpoint for loop A";
  parameter Real gainA = 10.0 "loop-A proportional gain";
  parameter Real plantA = 2.0 "loop-A static plant gain";
  parameter Real gainB = 5.0 "loop-B proportional gain";
  parameter Real plantB = 3.0 "loop-B static plant gain";
  Real errorA "loop-A error";
  Real commandA "loop-A controller output (also loop-B's setpoint)";
  Real errorB "loop-B error";
  Real commandB "loop-B controller output";
equation
  errorA = reference - plantA * commandA;   // ┐ coupled block A (2×2)
  commandA = gainA * errorA;                // ┘
  errorB = commandA - plantB * commandB;    // ┐ coupled block B (2×2), driven by commandA
  commandB = gainB * errorB;                // ┘
  annotation(experiment(StartTime = 0, StopTime = 1, Interval = 0.001, Tolerance = 1e-6));
end TwoLoops;
