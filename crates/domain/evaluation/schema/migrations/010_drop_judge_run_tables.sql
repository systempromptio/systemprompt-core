-- The standalone judge-run surface (admin evals run|replay|list|show) is gone;
-- supervised experiments own every paid evaluation. Nothing writes these tables.
DROP TABLE IF EXISTS eval_judge_calls;
DROP TABLE IF EXISTS eval_pairs;
DROP TABLE IF EXISTS eval_results;
DROP TABLE IF EXISTS eval_runs;
DROP TABLE IF EXISTS eval_rubrics;
