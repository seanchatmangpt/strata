
## Annealing Doctrine (repo clauses)

Global law: [[entropic-annealing-doctrine]]. Repo-specific clauses:

- **Serialized court runs** — N19/N20 dossier generation is single-lane;
  concurrent generator invocations share /tmp scratch and produced the rustc
  LTO ICE class. Lockfile or serialize, never parallel-generate. Mechanism landed: mkdir-lock guard at the top of `bin/generate_nato_dossier.sh` (exit 3 on concurrent run).
- **Every court needs a vacuity sweep** — no EXECUTED-GREEN grade without a
  positive control that must fire.
- **Skeptic/audit lanes do not grade their own builders** — a lane's receipt
  is only standing if another lane witnessed the run.
