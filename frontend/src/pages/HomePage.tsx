import { useState, useEffect, useCallback, useMemo } from "react";
import { Typography, Input, Select, Space } from "antd";
import { SearchOutlined, FilterOutlined } from "@ant-design/icons";
import {
  fetchTasks,
  fetchProjects,
  fetchContexts,
  deleteTask,
  type Task,
  type Project,
  type Context,
} from "../api/http";
import VoiceInput from "../components/VoiceInput";
import TextCapture from "../components/TextCapture";
import TaskList from "../components/TaskList";

const { Title } = Typography;

// ---------------------------------------------------------------------------
// Persistencia de filtros en localStorage
// ---------------------------------------------------------------------------

const FILTERS_KEY = "oxinbox:filters";

interface FilterState {
  searchText: string;
  statusFilter: string[];
  projectFilter: string[];
  contextFilter: string[];
  priorityFilter: string[];
}

function loadFilters(): FilterState {
  try {
    const raw = localStorage.getItem(FILTERS_KEY);
    if (raw) return JSON.parse(raw);
  } catch {
    /* ignore */
  }
  return {
    searchText: "",
    statusFilter: [],
    projectFilter: [],
    contextFilter: [],
    priorityFilter: [],
  };
}

function saveFilters(filters: FilterState) {
  try {
    localStorage.setItem(FILTERS_KEY, JSON.stringify(filters));
  } catch {
    /* ignore */
  }
}

// ---------------------------------------------------------------------------
// Opciones estáticas
// ---------------------------------------------------------------------------

const statusOptions = [
  { value: "inbox", label: "Inbox" },
  { value: "todo", label: "Todo" },
  { value: "doing", label: "Doing" },
  { value: "done", label: "Done" },
  { value: "someday", label: "Someday" },
];

const priorityFilterOptions = [
  { value: "A", label: "A" },
  { value: "B", label: "B" },
  { value: "C", label: "C" },
  { value: "D", label: "D" },
];

// ---------------------------------------------------------------------------
// Componente
// ---------------------------------------------------------------------------

export default function HomePage() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [loading, setLoading] = useState(true);
  const [projectsMap, setProjectsMap] = useState<Record<string, Project>>({});
  const [contextsMap, setContextsMap] = useState<Record<string, Context>>({});

  // Estados de filtros — inicializados desde localStorage
  const [searchText, setSearchText] = useState(() => loadFilters().searchText);
  const [statusFilter, setStatusFilter] = useState<string[]>(
    () => loadFilters().statusFilter,
  );
  const [projectFilter, setProjectFilter] = useState<string[]>(
    () => loadFilters().projectFilter,
  );
  const [contextFilter, setContextFilter] = useState<string[]>(
    () => loadFilters().contextFilter,
  );
  const [priorityFilter, setPriorityFilter] = useState<string[]>(
    () => loadFilters().priorityFilter,
  );

  // Persistir cada vez que cambia un filtro
  useEffect(() => {
    saveFilters({ searchText, statusFilter, projectFilter, contextFilter, priorityFilter });
  }, [searchText, statusFilter, projectFilter, contextFilter, priorityFilter]);

  const loadMaps = useCallback(async () => {
    try {
      const [projects, contexts] = await Promise.all([
        fetchProjects(),
        fetchContexts(),
      ]);
      setProjectsMap(Object.fromEntries(projects.map((p) => [p.id, p])));
      setContextsMap(Object.fromEntries(contexts.map((c) => [c.id, c])));
    } catch {
      /* ignore */
    }
  }, []);

  const loadTasks = useCallback(async () => {
    setLoading(true);
    try {
      setTasks(await fetchTasks());
    } catch (e) {
      console.error("Error loading tasks:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadTasks();
    loadMaps();
  }, [loadTasks, loadMaps]);

  const handleDelete = async (id: string) => {
    try {
      await deleteTask(id);
      setTasks((prev) => prev.filter((t) => t.id !== id));
    } catch (e) {
      console.error("Delete error:", e);
    }
  };

  const handleTaskCreated = (task?: Task) => {
    if (task) {
      setTasks((prev) => [task, ...prev]);
    } else {
      loadTasks();
    }
  };

  // Filtrado local
  const filteredTasks = useMemo(() => {
    let result = tasks;

    if (searchText.trim()) {
      const q = searchText.trim().toLowerCase();
      result = result.filter((t) => t.description.toLowerCase().includes(q));
    }

    if (statusFilter.length > 0) {
      result = result.filter((t) => statusFilter.includes(t.status));
    }

    if (projectFilter.length > 0) {
      result = result.filter((t) =>
        t.project_ids.some((id) => projectFilter.includes(id)),
      );
    }

    if (contextFilter.length > 0) {
      result = result.filter((t) =>
        t.context_ids.some((id) => contextFilter.includes(id)),
      );
    }

    if (priorityFilter.length > 0) {
      result = result.filter(
        (t) => t.priority && priorityFilter.includes(t.priority),
      );
    }

    return result;
  }, [tasks, searchText, statusFilter, projectFilter, contextFilter, priorityFilter]);

  const projectOptions = Object.values(projectsMap).map((p) => ({
    value: p.id,
    label: p.name,
  }));

  const contextOptions = Object.values(contextsMap).map((c) => ({
    value: c.id,
    label: c.name,
  }));

  const activeFilterCount =
    (searchText.trim() ? 1 : 0) +
    statusFilter.length +
    projectFilter.length +
    contextFilter.length +
    priorityFilter.length;

  return (
    <div>
      <VoiceInput onTaskCreated={() => loadTasks()} />
      <TextCapture onTaskCreated={handleTaskCreated} />

      {/* Barra de filtros */}
      <div
        style={{
          marginBottom: 12,
          padding: "8px 12px",
          background: "rgba(17,17,24,0.8)",
          borderRadius: 8,
          border: "1px solid #2a2a3e",
        }}
      >
        <Space direction="vertical" style={{ width: "100%" }} size={8}>
          <Input
            placeholder="Buscar tareas..."
            prefix={<SearchOutlined style={{ color: "#64647a" }} />}
            value={searchText}
            onChange={(e) => setSearchText(e.target.value)}
            allowClear
            style={{ fontSize: 13 }}
          />

          <div
            style={{
              display: "flex",
              flexWrap: "wrap",
              gap: 6,
              alignItems: "center",
            }}
          >
            <span
              style={{
                fontSize: 11,
                color: "#64647a",
                whiteSpace: "nowrap",
                display: "flex",
                alignItems: "center",
                gap: 4,
              }}
            >
              <FilterOutlined style={{ fontSize: 11 }} />
              {activeFilterCount > 0
                ? `${activeFilterCount} activo${activeFilterCount !== 1 ? "s" : ""}`
                : "Filtrar"}
            </span>

            <Select
              mode="multiple"
              size="small"
              placeholder="Estado"
              value={statusFilter}
              onChange={setStatusFilter}
              options={statusOptions}
              style={{ minWidth: 110, maxWidth: 180 }}
              maxTagCount={1}
              allowClear
            />

            <Select
              mode="multiple"
              size="small"
              placeholder="Proyecto"
              value={projectFilter}
              onChange={setProjectFilter}
              options={projectOptions}
              style={{ minWidth: 110, maxWidth: 180 }}
              maxTagCount={1}
              allowClear
            />

            <Select
              mode="multiple"
              size="small"
              placeholder="Contexto"
              value={contextFilter}
              onChange={setContextFilter}
              options={contextOptions}
              style={{ minWidth: 110, maxWidth: 180 }}
              maxTagCount={1}
              allowClear
            />

            <Select
              mode="multiple"
              size="small"
              placeholder="Prioridad"
              value={priorityFilter}
              onChange={setPriorityFilter}
              options={priorityFilterOptions}
              style={{ minWidth: 100, maxWidth: 140 }}
              maxTagCount={1}
              allowClear
            />
          </div>
        </Space>
      </div>

      {/* Contador */}
      <div style={{ marginBottom: 12 }}>
        <Title
          level={5}
          style={{ margin: 0, color: "#9494a8", fontWeight: 400 }}
        >
          {filteredTasks.length} tarea{filteredTasks.length !== 1 ? "s" : ""}
          {filteredTasks.length !== tasks.length &&
            ` (${tasks.length} totales)`}
        </Title>
      </div>

      <TaskList
        tasks={filteredTasks}
        projectsMap={projectsMap}
        contextsMap={contextsMap}
        onDelete={handleDelete}
        onRefresh={loadTasks}
        loading={loading}
      />
    </div>
  );
}