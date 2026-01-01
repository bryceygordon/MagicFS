# 🤝 Session Handover: The Snowflake Upgrade

## Context
We have successfully stabilized the "Experimentation Infrastructure" allowing us to swap AI models using git branches. 

**Current State (Branch: experiment/snowflake-xs):**
- Runs **Snowflake Arctic Embed XS**.
- **Status:** Stable, compiles, tests pass.
- **Problem:** Search relevance score is `0.41`, which is too low for a "Magical" feeling. The 384-dimensional vector space is too crowded.

## 🎯 Goal for Next Session
**Upgrade to Snowflake Arctic Medium (768 dims).**
We need to apply the specific code changes to:
1.  `src/oracle.rs` (Model selection)
2.  `src/storage/repository.rs` (Schema width)
3.  `src/main.rs` (Data isolation path)

## 📝 Branch Info
You are now on `experiment/snowflake-m`.
- **Base:** `experiment/snowflake-xs` (The stable XS code)
- **Task:** Apply the "Medium" upgrade code.
