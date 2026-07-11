CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS vector;
CREATE EXTENSION IF NOT EXISTS pg_search CASCADE;

CREATE TABLE projects (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    color TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE TABLE contexts (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    color TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE TABLE tasks (
    id UUID PRIMARY KEY,
    user_id TEXT NOT NULL,
    completed BOOLEAN DEFAULT FALSE NOT NULL,
    priority CHAR(1) CHECK (priority >= 'A' AND priority <= 'Z'),
    description TEXT NOT NULL,
    project_ids UUID[] DEFAULT '{}'::UUID[] NOT NULL,
    context_ids UUID[] DEFAULT '{}'::UUID[] NOT NULL,
    status VARCHAR(20) DEFAULT 'inbox' NOT NULL,

    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    completed_at TIMESTAMP WITH TIME ZONE,
    due_date DATE,

    embedding vector(1024)
);

CREATE TABLE task_history (
    id SERIAL PRIMARY KEY,
    task_id UUID REFERENCES tasks(id) ON DELETE CASCADE NOT NULL,
    from_status VARCHAR(20),
    to_status VARCHAR(20) NOT NULL,
    changed_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE INDEX idx_tasks_bm25 ON tasks USING bm25 (id, description, project_ids, context_ids) WITH (key_field='id');
CREATE INDEX idx_tasks_embedding ON tasks USING hnsw (embedding vector_cosine_ops);

-- BEFORE trigger: mutate row fields (updated_at, completed_at, completed)
CREATE OR REPLACE FUNCTION process_task_modifications()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;

    IF NEW.status = 'done' AND (OLD IS NULL OR OLD.status IS DISTINCT FROM 'done') THEN
        NEW.completed_at = CURRENT_TIMESTAMP;
        NEW.completed = TRUE;
    ELSIF NEW.status IS DISTINCT FROM 'done' THEN
        NEW.completed_at = NULL;
        NEW.completed = FALSE;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- AFTER trigger: log status changes (avoids FK violation on insert)
CREATE OR REPLACE FUNCTION log_task_status_change()
RETURNS TRIGGER AS $$
BEGIN
    IF (OLD IS NULL OR OLD.status IS DISTINCT FROM NEW.status) THEN
        INSERT INTO task_history (task_id, from_status, to_status)
        VALUES (NEW.id, OLD.status, NEW.status);
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_tasks_telemetry
BEFORE INSERT OR UPDATE ON tasks
FOR EACH ROW EXECUTE FUNCTION process_task_modifications();

CREATE TRIGGER trigger_task_history_log
AFTER INSERT OR UPDATE ON tasks
FOR EACH ROW EXECUTE FUNCTION log_task_status_change();
