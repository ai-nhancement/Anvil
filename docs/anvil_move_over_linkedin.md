# LinkedIn version: Move Over, Claude Code. Make Room for Anvil.

---

Your AI coding agent lies about its homework.

Not maliciously. But it does it, constantly. You run a long session. It edits a dozen files, runs some commands, and hands you a confident summary: added the validation, updated the tests, handled the edge cases.

Sometimes the tests were never touched.

The model did not set out to deceive you. It produced a plausible account of the work, the same way it produces plausible code. But plausible and true are not the same thing, and over a long session they drift apart. I have watched every frontier model do it: Claude, GPT, Grok, Gemini, all of them.

Here is the real problem, and it is structural. We let the coder grade its own homework. The same model writes the code and certifies the code, in one unbroken loop, with no independent check anywhere in the middle. Of course it passes its own review. It always passes its own review.

A better coder does not fix this. A better author is still an author judging itself.

So I built Anvil. It keeps the capable coder, but it takes away the coder's authority to certify its own work. At two points, the plan and each build phase, the work stops and gets reviewed by two other models from two different vendors. They do not read the coder's summary and nod along. They are read-only investigators that open the actual files, check the actual diff, and cite the actual line. Do not trust the coder's account. Go check.

The coder still does the work. It just no longer gets to be the one who decides the work is done.

The model expresses. The system decides. The work is only finished when something other than the author confirms it.

That is the principle I build on. For everything.

Anvil is open source and in public beta. Any provider, cloud or local. My own setup runs a strong cloud model as the coder and two free local models as the reviewers. The expensive model writes. The free ones keep it honest.

Full essay: https://ai-nhancement.com/essays/move-over-claude-code

#AI #AICoding #SoftwareEngineering #DeveloperTools #OpenSource
