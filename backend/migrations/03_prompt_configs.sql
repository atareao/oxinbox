-- Migration: Prompt configs table for user-customizable LLM prompts

CREATE TABLE IF NOT EXISTS prompt_configs (
    user_id              TEXT        NOT NULL,
    system_instructions  TEXT        NOT NULL DEFAULT '',
    few_shot_examples    TEXT        NOT NULL DEFAULT '',
    rules                TEXT        NOT NULL DEFAULT '',
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id)
);