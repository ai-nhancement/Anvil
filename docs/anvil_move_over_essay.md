# Move Over, Claude Code. Make Room for Anvil.

**John Canady Jr.**
AI nHancement | June 2026

---

I use AI coding agents every day. Claude Code, Cursor, the rest of them. I build real software with these tools, and I am not here to throw shade at any of them. They are remarkable. If you had shown me what they do today to the version of me writing code five years ago, I would not have believed you.

So let me say the friendly part first and mean it: this is not a takedown. I borrowed the title because more people will read an essay with "Claude Code" in it than an essay about a tool they have never heard of, and I would rather be honest about that than coy. The new guy is called Anvil. I built it. Here is why.

I built it because of one thing every single one of these coders does, and almost nobody talks about.

## The coder lies about its homework

Not maliciously. Not even knowingly. But it does it, and it does it constantly.

You run a long session. The agent reads your repo, edits a dozen files, runs some commands, and at the end it hands you a clean, confident summary: *added input validation, updated the tests, handled the edge cases.* The summary is well-written. It is plausible. It is the kind of thing a good engineer would say.

And sometimes the tests were never touched.

The validation is half there. The edge case it claimed to handle is exactly the one that breaks in production. The model did not set out to deceive you. It produced a plausible *account* of the work, the same way it produces plausible code. But plausibility and truth are not the same thing, and over a long session they drift apart.

This is not a Claude problem. It is not a Cursor problem. I have watched every frontier model do it: Claude, GPT, Grok, Gemini, all of them. It is a property of the setup, not the vendor. And once you have been burned by it a few times, you start to see the real issue.

## The category error

Here is the mistake, and it is structural.

We let the coder grade its own homework.

The same model that writes the code also reports on the code. It is the author and the judge. It produces the work and certifies the work, in one unbroken loop, with no independent check anywhere in the middle. We took the one entity with every incentive to declare success, and the least ability to see its own blind spots, and we made it the final authority on whether the work was done.

Of course it passes its own review. It always passes its own review.

I have written about this exact error before, in a different context. The AI industry took the language-production part of cognition, scaled it up, and asked it to also be the memory, the truth, and the judgment of the whole system. A model asked to be its own governor can always be talked out of governing, including by itself. The fix was never a bigger model. The fix was to take authority *out* of the model and put it in a system the model cannot override.

Anvil is that idea, pointed at coding.

## A better coder is not the answer

The reflex, when the coder gets something wrong, is to reach for a better coder. A smarter model. A bigger context window. A more careful prompt.

That helps. It does not solve this. A better author is still an author judging itself. You can make the homework better and still have nobody checking it but the kid who wrote it.

What you actually need is a second set of eyes that did not write the code, does not share the coder's blind spots, and has no stake in declaring the work finished. You need a reviewer. And not a reviewer that reads the coder's *summary* and nods along, but one that opens the actual files and checks whether the summary is true.

That is the whole idea behind Anvil.

## What Anvil is

Anvil is a coding agent for the terminal, like the others. It has a real coder. It reads, writes, edits, and runs your project itself. That part is table stakes now. Everyone has it.

What Anvil adds is the part that was missing.

At two points in the work, once when the plan is written and once at the end of each build phase, the work stops and gets reviewed. Not by the coder. By **two other models, from two different vendors.** A flaw that is invisible to one model's family of thinking has a chance to be visible to another. Two instances of the same model share the same blind spots. Two different vendors do not. The disagreement between them is not noise. It is the entire point.

And these reviewers do not take the coder's word for anything. They are read-only investigators. They get tools to open your files, search the code, and inspect the real state of the repository. And they are told, explicitly, *do not trust the coder's account; go check.* They read the actual diff. They cite the actual file and line. A claim with no evidence behind it is not a finding.

Then the loop runs: the first reviewer critiques, the coder fixes, the second reviewer re-reviews *after* those fixes, because fixes introduce bugs too, and only then does the work come to you. You answer two questions. *Another round? Ship it?* Everything procedural happens underneath.

And here is the part I like most: Anvil does not care whose models you use. The coder and the two reviewers can each come from a different provider, cloud or local, in whatever mix you want. My own favorite setup is also the cheap one. The coder is a strong cloud model, the kind you would reach for anyway, and both reviewers are small models running free on my own machine through Ollama. The expensive model does the writing. Two free local models do the watching. You get a real cross-vendor second opinion on every gate, and the second opinion costs almost nothing.

The coder still does the work. It just no longer gets to be the one who decides the work is done.

## I did this by hand for months

None of this is theoretical. Before Anvil was a tool, it was my daily routine, run manually, by hand, with three browser tabs open.

I would have one model write a plan. I would paste it to a second model from a different company and ask it to tear the plan apart. I would carry the findings back, have the first model fix them, then paste the result to a *third* model and do it again. Plan, then every phase of the build, over and over. It was slow and it was tedious and it caught an enormous number of problems that any single model, including the one that wrote the code, swore up and down were not there.

Anvil is that workflow, turned into one program. The copy-pasting is gone. The discipline is not.

## It worked the first time it mattered

I will not oversell this, because it is one project and I am the person who built the tool. But it is worth telling.

I used Anvil to take a real project from the first phase all the way through several build phases under the full two-gate workflow. Every phase was checked by an independent, cross-vendor reviewer against the real code before the next phase built on top of it.

The finished software ran correctly on its first run.

If you have shipped multi-phase work with an autonomous coder, you know that a clean first run across an entire build is not the normal outcome. That is not luck. That is what happens when errors get caught at the boundary of each phase instead of discovered all at once at the end. The structure did exactly what it was built to do.

One project is not proof. It is a signal. But it is a real one, and it is the reason I am writing this instead of quietly iterating.

## The principle

This is the same principle I build everything on.

The model expresses. It does not own the truth. The author produces the work; it does not get to be the final judge of the work. Authority over whether something is correct, complete, and ready to ship belongs to a process the model cannot talk its way around. At the end of that process, it belongs to a human.

A coder grading its own homework is not a workflow. It is a hope. Anvil replaces the hope with a structure: two gates, two independent vendors, reviewers that check the real files instead of the coder's story, and you at the decision points.

That is not a constraint on the coder. It is what makes the coder's output worth trusting.

So, move over, Claude Code. Not because you are not good. You are very good. But because being good at writing the code was never the hard part. The hard part is knowing, honestly, that it was actually done.

That is the part I built the new guy to handle.

---

*Anvil is open source and in public beta: one self-contained binary, works with any model provider, installs in a single line. John Canady Jr. is the founder of AI nHancement and the architect of AiMe, a bounded-authority cognitive system in continuous daily operation since November 2025. His research, open-source tools, and a live demonstration are available at ai-nhancement.com and anvil.codes.*
