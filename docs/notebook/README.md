# HRW Lab Notebook

One file per specimen (`<Specimen>.md`), recording what we learn about how
Rumoca processes it. This is **HRW's own record** — specimen-driven observations
tied to concrete IR — kept deliberately distinct from the Rumoca clone's
`docs/understanding`, which is Doug's canonical, general explanation of each
compiler phase.

**Authorship: Doug writes these.** Writing the synthesis in your own words is the
learning; that rep is the point, not overhead to optimize away. Claude drafts a
section only when asked, and challenges/tightens what's written — but the
understanding recorded here is yours.

**How it fills up:** the [Claude bridge](../../src/bridge.rs) makes the chat
conversations happen (point at an IR node → "Ask Claude about this" → ask in the
Claude Code chat). Those answers are ephemeral. When one is a keeper, *you*
promote it into that specimen's notebook file below.

Start a new specimen from [`_TEMPLATE.md`](_TEMPLATE.md).
