#!/usr/bin/env wolframscript
(* ::Package:: *)

(* ===========================================================================
   oracle.wls -- adjudicate models against System Modeler, one row at a time.

   Usage:
     wolframscript -file hrw/scripts/oracle.wls
         -models  <file>   one .mo path OR one model name per line
         -out     <csv>    appended; rows already present are SKIPPED (resume)
        [-kinds   <csv>]   a survey/fidelity report to take the `kind` column from
        [-tend    <n>]     simulation end time, default 1
        [-timeout <s>]     per-model wall clock, default 120

   WHY THIS EXISTS AS A SCRIPT, AND NOT AS A SEQUENCE OF TOOL CALLS
   ---------------------------------------------------------------
   `docs/ideas.md` #43 records that Claude can run the oracle recipe himself
   through the Wolfram MCP server. On 2026-11-06 that was tried over the 28
   specimens as a single 14-model batch and it failed three times, costing 90
   minutes: **each System Modeler run compiles C, so a model costs ~2 minutes**,
   a 28-model sweep is about an hour, and the MCP tool aborts a call after 30
   minutes of silence. The kernel kept working after each abort and every result
   was discarded, because nothing had been written down.

   Four of those results were recovered afterwards from System Modeler's own
   `.mat` files in its temp directory -- which is evidence by accident, not a
   procedure.

   So this script does what `measure-fidelity.ps1` already does for the fidelity
   sweep: **one row written and flushed per model, and a resume that reads the
   output back.** An aborted invocation then loses at most the model in flight,
   and the next invocation continues. The sweep becomes repeatable rather than
   babysat.

   THE REPORT CONTRACT -- `docs/reports.md` IS THE AUTHORITY
   --------------------------------------------------------
   All three reports share their first four columns, `name`, `kind`, `outcome`,
   `message`, and **`name` is the join key**: the fully qualified model name,
   spelled identically. That is what lets the admissibility table be computed
   rather than eyeballed --

     survey compiles + fidelity green + oracle mismatch -> file it upstream
     survey compiles + fidelity FAILING + oracle mismatch -> inadmissible

   -- so a report keyed by file path or by a System Modeler identifier would be
   worthless for the one use it has. `reports.md` says retrofitting the key later
   would cost the join, which is why it is honoured from the first row.

   `kind` is JOINED, not recomputed. HRW's `survey::classify` is the one
   definition of that column; spelling a second one here in Wolfram is exactly
   the duplicate-definition shape `no_second_copy_of_the_msl_root_list` exists to
   forbid. With no `-kinds` report the column is left empty rather than guessed.
   =========================================================================== *)

argAfter[flag_] := Module[{i = FirstPosition[$ScriptCommandLine, flag]},
  If[MissingQ[i] || i[[1]] + 1 > Length[$ScriptCommandLine], Missing[],
    $ScriptCommandLine[[i[[1]] + 1]]]];

modelsFile = argAfter["-models"];
outFile    = argAfter["-out"];
kindsFile  = argAfter["-kinds"];
tEnd       = If[MissingQ[argAfter["-tend"]], 1, ToExpression[argAfter["-tend"]]];
timeout    = If[MissingQ[argAfter["-timeout"]], 120, ToExpression[argAfter["-timeout"]]];

If[MissingQ[modelsFile] || MissingQ[outFile],
  Print["oracle.wls: -models and -out are both required."]; Exit[2]];
If[! FileExistsQ[modelsFile],
  Print["oracle.wls: no such models file: " <> modelsFile]; Exit[2]];

(* ---------------------------------------------------------------- csv *)
(* Quote only when the field needs it, and double an embedded quote -- the same
   rule `survey::csv_field` follows, so one loader reads all three reports. *)
csvField[s_String] := If[StringContainsQ[s, {",", "\"", "\n"}],
  "\"" <> StringReplace[s, "\"" -> "\"\""] <> "\"", s];
csvField[x_] := csvField[ToString[x]];

(* ---------------------------------------------------------------- kinds *)
kindOf = <||>;
If[! MissingQ[kindsFile] && FileExistsQ[kindsFile],
  Module[{rows = Import[kindsFile, "CSV"]},
    If[Length[rows] > 1,
      kindOf = Association[(#[[1]] -> #[[2]]) & /@ Rest[rows]]]]];

(* ---------------------------------------------------------------- resume *)
(* Keyed by name, exactly like the fidelity wrapper's `--resume`. A row already
   present means the model has been adjudicated; re-running must not re-pay its
   two minutes. *)
done = {};
If[FileExistsQ[outFile],
  Module[{rows = Import[outFile, "CSV"]},
    If[Length[rows] > 1, done = #[[1]] & /@ Rest[rows]]],
  (* A fresh report carries the shared header, written as text rather than with
     `Export[..., "CSV"]` -- Export quotes every field, and `"name","kind",...`
     is not the header the survey and fidelity reports carry. One loader reads
     all three (`docs/reports.md`), so the bytes have to match, not just the
     column order. *)
  (* `BinaryFormat -> True` so the newline is LF. A Wolfram text stream on
     Windows translates "\n" to CRLF, and the survey and fidelity reports are LF
     -- a loader that splits on "\n" would then leave "\r" on every last field,
     which is precisely the bug that broke a name join during this day's work. *)
  Module[{h = OpenWrite[outFile, BinaryFormat -> True]},
    WriteString[h, "name,kind,outcome,message,t_end,secs\n"]; Close[h]]];

(* ---------------------------------------------------------------- input *)
(* An entry is either a path to a .mo file or a bare model name.

   For a FILE the model is the first class in it, because that is what HRW
   compiles (`ast.classes.keys().next()`). `Import[..., "MO"]` returns the LAST
   class instead, so the two conventions are opposite and `ideas.md` #43 records
   that the mismatch is silent: a five-class file once simulated its `Gnd`
   and reported success. Naming the model is the fix, and `sim["ModelName"]` is
   checked below as the guard.

   The first class is found textually, which is an approximation of HRW's AST
   order and is why the ModelName guard is not optional. *)
firstClassOf[path_] := Module[{text, hits},
  text = Quiet@Check[Import[path, "Text"], $Failed];
  If[text === $Failed, Return[Missing[]]];
  hits = StringCases[text,
    RegularExpression[
      "(?m)^[ \\t]*(?:partial[ \\t]+)?(?:model|block|class|connector|record|package|function|type)[ \\t]+([A-Za-z_][A-Za-z0-9_]*)"] :> "$1"];
  If[hits === {}, Missing[], First[hits]]];

entries = Select[StringTrim /@ ReadList[modelsFile, "String"], # =!= "" &];

(* ---------------------------------------------------------------- one model *)
adjudicate[entry_] := Module[
  {isFile, path, name, imp, sim, msgs = {}, outcome, message = "", t0, secs},
  isFile = StringEndsQ[entry, ".mo", IgnoreCase -> True];
  path = If[isFile, entry, Missing[]];
  name = If[isFile, firstClassOf[entry], entry];
  t0 = AbsoluteTime[];

  Which[
    MissingQ[name],
      outcome = "no-class"; message = "no class declaration found in " <> entry,
    True,
      (* Import registers every class in the file; the result is deliberately
         ignored, since it is the LAST class and we simulate by name. *)
      imp = If[isFile, Quiet@Check[Import[path, "MO"], $Failed], Null];
      If[isFile && imp === $Failed,
        outcome = "import-failed";
        message = "System Modeler could not import the file",
        (* else: simulate, collecting message names for the record *)
        sim = Block[{$MessageList = {}},
          Module[{r = Quiet@Check[
              TimeConstrained[SystemModelSimulate[name, tEnd], timeout, "timeout"],
              $Failed]},
            msgs = ToString /@ $MessageList; r]];
        Which[
          sim === "timeout",
            outcome = "timeout";
            message = "exceeded " <> ToString[timeout] <> "s",
          Head[sim] === SystemModelSimulationData,
            (* THE GUARD. A successful-looking run of the wrong model is the one
               failure mode `ideas.md` #43 calls out by name. *)
            If[sim["ModelName"] === name,
              outcome = "simulated",
              outcome = "wrong-model";
              message = "simulated " <> ToString[sim["ModelName"]] <> " instead"],
          True,
            outcome = "refused";
            message = If[msgs === {}, "SystemModelSimulate returned $Failed",
              StringTake[StringRiffle[DeleteDuplicates[msgs], "; "], UpTo[300]]]]]];

  secs = Round[AbsoluteTime[] - t0, 0.1];
  {If[MissingQ[name], entry, name], Lookup[kindOf, If[MissingQ[name], entry, name], ""],
   outcome, message, tEnd, secs}];

(* ---------------------------------------------------------------- sweep *)
(* Appended and closed per model. Holding the handle open across a two-minute
   System Modeler compile would put every result of an aborted run in a buffer
   that is never flushed -- which is the failure this script was written for. *)
Do[
  Module[{entry = entries[[i]], probe, row, stream},
    probe = If[StringEndsQ[entry, ".mo", IgnoreCase -> True], firstClassOf[entry], entry];
    If[! MissingQ[probe] && MemberQ[done, probe],
      Print["[skip] " <> probe <> " (already in report)"],
      row = adjudicate[entry];
      (* BinaryFormat for the same reason as the header: LF, not CRLF. *)
      stream = OpenAppend[outFile, BinaryFormat -> True];
      WriteString[stream, StringRiffle[csvField /@ row, ","] <> "\n"];
      Close[stream];
      Print["[" <> ToString[i] <> "/" <> ToString[Length[entries]] <> "] " <>
        ToString[row[[1]]] <> "  " <> ToString[row[[3]]] <>
        "  " <> ToString[row[[6]]] <> "s"]]],
  {i, Length[entries]}];

Print["oracle: wrote " <> outFile];
