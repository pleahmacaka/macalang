# Baseline

pass@1 over the mechanically ported HumanEval set, by what the model
was told. Regenerate with `maca run apps/evals/run.maca` and commit
the change. The model was `./apps/evals/models/claude.sh`.

| condition | passed | of | pass@1 |
|---|---|---|---|
| no spec | 1 | 25 | 4% |
| spec | 8 | 25 | 32% |
| spec + one check retry | 10 | 25 | 40% |

