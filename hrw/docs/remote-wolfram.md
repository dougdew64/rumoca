# Remote Wolfram — reaching this machine's Wolfram from another machine

**Purpose:** the plan for using Wolfram Desktop / System Modeler, installed on the main Windows
machine, from a Claude Code session running on a second machine (the Dell XPS, which runs Fedora).
**Status:** plan — **nothing here is built or tested.** Written 2026-11-26.
**Read when:** before working on the XPS with oracle runs or Wolfram tools in mind.

## What actually needs to reach Wolfram

**Not HRW.** Under charter Decision 19, HRW causes questions and the platform answers them; HRW
never calls Wolfram. Two things do, and both are Claude's:

- **The Wolfram MCP tools** in the conversation (`WolframLanguageEvaluator` and friends), which
  run `SystemModelSimulate`, evaluate Wolfram Language and read notebooks.
- **`scripts/oracle.wls`**, run with `wolframscript` — the System Modeler verdict on each
  specimen (`docs/reports/specimen-oracle-report.csv`). It needs `--` before its own flags.

So the question is only whether **Claude Code on the XPS** can reach a Wolfram kernel on the main
machine.

## Why it is feasible: the MCP server is a stdio process

On the main machine the Wolfram MCP server is configured in `%USERPROFILE%\.claude.json` as a
**stdio** server — Claude Code starts a local process and talks to it over stdin/stdout:

```json
"Wolfram": {
  "type": "stdio",
  "command": "C:\\Program Files\\Wolfram Research\\Wolfram\\15.0\\wolfram.exe",
  "args": ["-run", "PacletSymbol[\"Wolfram/AgentTools\",\"Wolfram`AgentTools`StartMCPServer\"][]",
           "-noinit", "-noprompt"],
  "env": { "MCP_SERVER_NAME": "WolframLanguage", "WOLFRAM_BASE": "…", "WOLFRAM_USERBASE": "…", … }
}
```

**A stdio server can be started over SSH unchanged**: SSH carries stdin and stdout. On the XPS the
same entry's `command` becomes `ssh <main machine> <launcher>`, and every evaluation, every
`SystemModelSimulate` and every oracle run executes on the main machine, under its installation and
licence, with results returning to the conversation on the XPS.

## The plan

### On the main machine (Windows)

1. **Install the OpenSSH server** — a built-in optional Windows feature; it was not installed on
   2026-11-26 (`Get-Service sshd` found nothing). Allow **key authentication only**.
2. **A launcher script** that sets the environment the MCP entry sets (`MCP_SERVER_NAME`,
   `WOLFRAM_BASE`, `WOLFRAM_USERBASE`, `APPDATA`, …) and then starts `wolfram.exe` with the same
   arguments. SSH sessions do not inherit Claude Code's per-server `env`, so the launcher must
   supply it.
3. **Stay awake and up.** The machine must be on, awake and online for the whole trip: no sleep
   or hibernate timeout (`cargo run -p hrw --example check_machine` reports both), and Windows
   Update set so it cannot reboot unattended. A reboot with nobody home ends the link.

### The network path

The machines will not share a network while travelling. Use a **private VPN between the two
machines** (Tailscale is the usual low-effort choice) rather than exposing SSH to the internet.

### On the XPS (Fedora)

4. **An MCP entry** whose `command` is `ssh`, with the main machine and the launcher as arguments.
5. **`ssh` for the oracle**: `ssh <main machine> wolframscript -file hrw/scripts/oracle.wls -- …`,
   run from the main machine's clone.

### Files: Wolfram sees only the main machine's disk

Any path in a Wolfram call is a **main-machine path**. Before a System Modeler run, bring the main
machine's clone up to date over SSH (`git pull` on `hrw-rebase-0.11.0`) so it holds the specimen
being checked — or pass the model text in the call rather than a path.

## Open questions — decide before relying on it

- **Licensing.** Whether Wolfram's licence terms permit driving this installation remotely is not
  known. Check before depending on it.
- **The alternative may be simpler.** Wolfram Desktop and System Modeler both exist for Linux. If
  the licence covers a second machine, installing them on the XPS removes the network dependency,
  the stay-awake requirement and the path mismatch at once. The licence decides between the two.
- **Timeouts.** A System Modeler run compiles C and takes about two minutes per model; the MCP
  tool aborts a call after about 30 minutes of silence (`oracle.wls`'s header records the cost of
  learning that). Network latency is negligible beside both.

## What this does not solve

**HRW builds and runs on Linux** *(corrected 2026-11-28; this said it never had)*. Since
2026-11-26 the context hook is a Rust binary (`hrw-context`), not PowerShell, and the full test
gate and step 10's Linux parts have passed on Fedora. What is still Windows-only: the corpus
fidelity sweep (`scripts/measure-fidelity.ps1`), and Claude's memory folder is named after the
machine's path. Claude's memory does not travel with
the repository; `setup-windows.md` → *Switching machines*.

## Estimated effort

About an hour, mostly on the main machine, **best done before leaving** while both machines are at
hand: SSH server and keys, the launcher, the VPN on both machines, the MCP entry on the XPS, and
one test evaluation plus one oracle run from the XPS.
