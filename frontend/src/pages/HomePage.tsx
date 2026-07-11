import { useState, useEffect, useCallback, useMemo } from "react";
import { Typography, Tabs, Input, Select, Space } from "antd";
import { SearchOutlined, FilterOutlined } from "@ant-design/icons";
import {
  fetchTasks,
  fetchProjects,
  fetchContexts,
  deleteTask,
  updateTask,
  type Task,
  type Project,
  type Context,
} from "../api/http";
import VoiceInput from "../components/VoiceInput";
import TextCapture from "../components/TextCapture";
import TaskList from "../components/TaskList";

const { Title } = Typography;

// ---------------------------------------------------------------------------
// Configuración de tabs (solo icono)
// ---------------------------------------------------------------------------

interface TabConfig {
  key: string;
  icon: string;
  statuses: string[];
}

const TABS: TabConfig[] = [
  { key: "inbox", icon: "📥", statuses: ["inbox"] },
  { key: "todo", icon: "📋", statuses: ["todo"] },
  { key: "doing", icon: "⚡", statuses: ["doing"] },
  { key: "done", icon: "✅", statuses: ["done"] },
  { key: "someday", icon: "🔮", statuses: ["someday"] },
];

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
  const [activeTab, setActiveTab] = useState("inbox");
  const [projectsMap, setProjectsMap] = useState<Record<string, Project>>({});
  const [contextsMap, setContextsMap] = useState<Record<string, Context>>({});

  // Estados de filtros
  const [searchText, setSearchText] = useState(() => loadFilters().searchText);
  const [statusFilter] = useState<string[]>(
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

  // Persistir filtros
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

  const handleStatusChange = async (id: string, newStatus: string) => {
    try {
      const updated = await updateTask(id, { status: newStatus });
      setTasks((prev) => prev.map((t) => (t.id === id ? updated : t)));
    } catch (e) {
      console.error("Status change error:", e);
    }
  };

  const handleTaskCreated = (task?: Task) => {
    if (task) {
      setTasks((prev) => [task, ...prev]);
    } else {
      loadTasks();
    }
  };

  // Obtener el tab activo
  const activeTabConfig = TABS.find((t) => t.key === activeTab) || TABS[0];

  // Tareas filtradas por status del tab activo
  const tabTasks = useMemo(() => {
    return tasks.filter((t) => activeTabConfig.statuses.includes(t.status));
  }, [tasks, activeTabConfig]);

  // Filtros de búsqueda aplicados sobre las tareas del tab
  const filteredTasks = useMemo(() => {
    let result = tabTasks;

    if (searchText.trim()) {
      const q = searchText.trim().toLowerCase();
      result = result.filter((t) => t.description.toLowerCase().includes(q));
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
  }, [tabTasks, searchText, projectFilter, contextFilter, priorityFilter]);

  const activeFilterCount =
    (searchText.trim() ? 1 : 0) +
    projectFilter.length +
    contextFilter.length +
    priorityFilter.length;

  const projectOptions = Object.values(projectsMap).map((p) => ({
    value: p.id,
    label: p.name,
  }));

  const contextOptions = Object.values(contextsMap).map((c) => ({
    value: c.id,
    label: c.name,
  }));

  // -----------------------------------------------------------------------
  // Barra de filtros reutilizable
  // -----------------------------------------------------------------------

  const FilterBar = () => (
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
  );

  // -----------------------------------------------------------------------
  // Tab items (solo icono + contador pequeño)
  // -----------------------------------------------------------------------

  const tabItems = TABS.map((tab) => ({
    key: tab.key,
    label: (
      <span style={{ fontSize: 18, position: "relative" }}>
        {tab.icon}
        <span
          style={{
            position: "absolute",
            top: -6,
            right: -10,
            fontSize: 10,
            color: "#64647a",
            fontWeight: 600,
          }}
        >
          {tasks.filter((t) => tab.statuses.includes(t.status)).length}
        </span>
      </span>
    ),
    children: (
      <>
        <div style={{ marginBottom: 12 }}>
          <Title
            level={5}
            style={{ margin: 0, color: "#9494a8", fontWeight: 400 }}
          >
            {filteredTasks.length} tarea{filteredTasks.length !== 1 ? "s" : ""}
            {filteredTasks.length !== tabTasks.length &&
              ` (${tabTasks.length} en ${tab.icon})`}
          </Title>
        </div>

        <TaskList
          tasks={filteredTasks}
          projectsMap={projectsMap}
          contextsMap={contextsMap}
          onDelete={handleDelete}
          onRefresh={loadTasks}
          onStatusChange={handleStatusChange}
          onTaskClick={handleStatusChange}
          loading={loading}
        />
      </>
    ),
  }));

  // -----------------------------------------------------------------------
  // Render
  // -----------------------------------------------------------------------

  return (
    <div>
      {/* Input de creación — fuera de las tabs, siempre visible */}
      <div style={{ marginBottom: 8 }}>
        <div style={{ display: "flex", gap: 8, alignItems: "flex-start" }}>
          <div style={{ flex: 1, minWidth: 0 }}>
            <TextCapture onTaskCreated={handleTaskCreated} />
          </div>
          <VoiceInput onTaskCreated={() => loadTasks()} />
        </div>
      </div>

      <FilterBar />

      <Tabs
        activeKey={activeTab}
        onChange={setActiveTab}
        items={tabItems}
        size="small"
        style={{ marginBottom: 0 }}
        tabBarStyle={{ marginBottom: 12 }}
      />
    </div>
  );
}
