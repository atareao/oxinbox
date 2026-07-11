-- Full-field change tracking (application-managed, complement to trigger-based status_log)
CREATE TABLE IF NOT EXISTS field_changes (
    id SERIAL PRIMARY KEY,
    task_id UUID REFERENCES tasks(id) ON DELETE CASCADE NOT NULL,
    field_name TEXT NOT NULL,
    old_value TEXT,
    new_value TEXT NOT NULL,
    changed_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_field_changes_task_id ON field_changes(task_id);
CREATE INDEX IF NOT EXISTS idx_field_changes_changed_at ON field_changes(changed_at);