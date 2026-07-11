import { useState } from "react";
import {
  List,
  Tag,
  Typography,
  Button,
  Space,
  Select,
  Skeleton,
  Tooltip,
  Input,
} from "antd";
import {
  DeleteOutlined,
  EditOutlined,
  CloseOutlined,
  CheckOutlined,
} from "@ant-design/icons";
import {
  updateTask,
  type Task,
  type Project,
  type Context,
} from "../api/http";

const { Text } = Typography;

const statusColors: Record<string, string> = {
  inbox: "default",
  todo: "blue",
  doing: "orange",
  done: "green",
  someday: "purple",
};

const statusOptions = [
  { value: "inbox", label: "Inbox" },
  { value: "todo", label: "Todo" },
  { value: "doing", label: "Doing" },
  { value: "done", label: "Done" },
  { value: "someday", label: "Someday" },
];

const priorityOptions = [
  { value: "", label: "—" },
  { value: "A", label: "A" },
  { value: "B", label: "B" },
  { value: "C", label: "C" },
  { value: "D", label: "D" },
];

// ---------------------------------------------------------------------------
// Tipos
// ---------------------------------------------------------------------------

interface Props {
  tasks: Task[];
  projectsMap: Record<string, Project>;
  contextsMap: Record<string, Context>;
  onDelete: (id: string) => void;
  onRefresh: () => void;
  loading?: boolean;
}

interface EditState {
  description: string;
  status: string;
  priority: string;
  project_ids: string[];
  context_ids: string[];
}

// ---------------------------------------------------------------------------
// Componente
// ---------------------------------------------------------------------------

export default function TaskList({
  tasks,
  projectsMap,
  contextsMap,
  onDelete,
  onRefresh,
  loading,
}: Props) {
  const [saving, setSaving] = useState<Record<string, boolean>>({});
  const [editingTask, setEditingTask] = useState<string | null>(null);
  const [editState, setEditState] = useState<EditState | null>(null);

  // Iniciar edición — copia los valores actuales de la tarea
  const startEditing = (task: Task) => {
    setEditingTask(task.id);
    setEditState({
      description: task.description,
      status: task.status,
      priority: task.priority || "",
      project_ids: [...task.project_ids],
      context_ids: [...task.context_ids],
    });
  };

  // Cancelar edición
  const cancelEditing = () => {
    setEditingTask(null);
    setEditState(null);
  };

  // Guardar cambios
  const saveEditing = async (taskId: string) => {
    if (!editState) return;
    setSaving((prev) => ({ ...prev, [taskId]: true }));
    try {
      await updateTask(taskId, {
        description: editState.description,
        status: editState.status,
        priority: editState.priority || null,
        project_ids: editState.project_ids,
        context_ids: editState.context_ids,
      });
      cancelEditing();
      onRefresh();
    } catch {
      /* error */
    } finally {
      setSaving((prev) => ({ ...prev, [taskId]: false }));
    }
  };

  const projectOptions = Object.values(projectsMap).map((p) => ({
    value: p.id,
    label: p.name,
  }));

  const contextOptions = Object.values(contextsMap).map((c) => ({
    value: c.id,
    label: c.name,
  }));

  if (loading) return <TaskListSkeleton />;

  return (
    <List
      className="stagger-enter"
      dataSource={tasks}
      locale={{ emptyText: "No hay tareas. Usa el micrófono o escribe una." }}
      renderItem={(task) => {
        const isEditing = editingTask === task.id;

        // =================================================================
        // MODO EDICIÓN — la tarea se da la vuelta
        // =================================================================
        if (isEditing && editState) {
          return (
            <List.Item
              className="task-enter"
              style={{
                background: "rgba(30,30,48,0.5)",
                borderRadius: 8,
                padding: "12px 16px",
              }}
            >
              <div style={{ flex: 1, minWidth: 0 }}>
                <div
                  style={{
                    display: "flex",
                    flexDirection: "column",
                    gap: 8,
                  }}
                >
                  {/* Descripción */}
                  <Input
                    size="small"
                    value={editState.description}
                    onChange={(e) =>
                      setEditState((prev) =>
                        prev ? { ...prev, description: e.target.value } : prev,
                      )
                    }
                    placeholder="Descripción de la tarea"
                  />

                  {/* Fila: Estado + Prioridad */}
                  <div style={{ display: "flex", gap: 8 }}>
                    <div style={{ flex: 1 }}>
                      <Text
                        type="secondary"
                        style={{ fontSize: 11, display: "block", marginBottom: 2 }}
                      >
                        Estado
                      </Text>
                      <Select
                        size="small"
                        value={editState.status}
                        onChange={(v) =>
                          setEditState((prev) =>
                            prev ? { ...prev, status: v } : prev,
                          )
                        }
                        options={statusOptions}
                        style={{ width: "100%" }}
                      />
                    </div>
                    <div style={{ flex: 1 }}>
                      <Text
                        type="secondary"
                        style={{ fontSize: 11, display: "block", marginBottom: 2 }}
                      >
                        Prioridad
                      </Text>
                      <Select
                        size="small"
                        value={editState.priority}
                        onChange={(v) =>
                          setEditState((prev) =>
                            prev ? { ...prev, priority: v } : prev,
                          )
                        }
                        options={priorityOptions}
                        style={{ width: "100%" }}
                      />
                    </div>
                  </div>

                  {/* Fila: Proyecto + Contexto */}
                  <div style={{ display: "flex", gap: 8 }}>
                    <div style={{ flex: 1 }}>
                      <Text
                        type="secondary"
                        style={{ fontSize: 11, display: "block", marginBottom: 2 }}
                      >
                        Proyecto
                      </Text>
                      <Select
                        size="small"
                        mode="multiple"
                        value={editState.project_ids}
                        onChange={(v) =>
                          setEditState((prev) =>
                            prev ? { ...prev, project_ids: v } : prev,
                          )
                        }
                        options={projectOptions}
                        placeholder="+proyecto"
                        style={{ width: "100%" }}
                        maxTagCount={2}
                      />
                    </div>
                    <div style={{ flex: 1 }}>
                      <Text
                        type="secondary"
                        style={{ fontSize: 11, display: "block", marginBottom: 2 }}
                      >
                        Contexto
                      </Text>
                      <Select
                        size="small"
                        mode="multiple"
                        value={editState.context_ids}
                        onChange={(v) =>
                          setEditState((prev) =>
                            prev ? { ...prev, context_ids: v } : prev,
                          )
                        }
                        options={contextOptions}
                        placeholder="@contexto"
                        style={{ width: "100%" }}
                        maxTagCount={2}
                      />
                    </div>
                  </div>

                  {/* Botones: Guardar / Cancelar */}
                  <div
                    style={{
                      display: "flex",
                      justifyContent: "flex-end",
                      gap: 6,
                      marginTop: 4,
                    }}
                  >
                    <Button
                      size="small"
                      icon={<CloseOutlined />}
                      onClick={cancelEditing}
                      disabled={saving[task.id]}
                    >
                      Cancelar
                    </Button>
                    <Button
                      size="small"
                      type="primary"
                      icon={<CheckOutlined />}
                      onClick={() => saveEditing(task.id)}
                      loading={saving[task.id]}
                    >
                      Guardar
                    </Button>
                  </div>
                </div>
              </div>
            </List.Item>
          );
        }

        // =================================================================
        // MODO VISTA — etiquetas + botón editar
        // =================================================================
        return (
          <List.Item
            className="task-enter"
            actions={[
              <Tooltip key="edit" title="Editar">
                <Button
                  type="text"
                  size="small"
                  icon={<EditOutlined />}
                  onClick={() => startEditing(task)}
                  style={{ color: "#9494a8" }}
                />
              </Tooltip>,
              <Tooltip key="delete" title="Eliminar">
                <Button
                  type="text"
                  danger
                  size="small"
                  icon={<DeleteOutlined />}
                  onClick={() => onDelete(task.id)}
                />
              </Tooltip>,
            ]}
            style={{
              opacity: task.status === "done" ? 0.5 : 1,
              textDecoration:
                task.status === "done" ? "line-through" : undefined,
            }}
          >
            <div style={{ flex: 1, minWidth: 0 }}>
              {/* Descripción */}
              <Text strong style={{ fontSize: 15 }}>
                {task.description}
              </Text>
              <br />

              {/* Tags: prioridad, estado, proyectos, contextos, fecha */}
              <Space size={4} wrap style={{ marginTop: 4 }}>
                {/* Prioridad */}
                {task.priority && (
                  <Tag color="warning" style={{ fontSize: 10 }}>
                    [{task.priority}]
                  </Tag>
                )}

                {/* Estado (siempre visible) */}
                <Tag color={statusColors[task.status]} style={{ fontSize: 10 }}>
                  {task.status}
                </Tag>

                {/* Proyectos */}
                {task.project_ids.map((id) => {
                  const p = projectsMap[id];
                  return p ? (
                    <Tag
                      key={id}
                      color={p.color || "cyan"}
                      style={{ fontSize: 10 }}
                    >
                      +{p.name}
                    </Tag>
                  ) : null;
                })}

                {/* Contextos */}
                {task.context_ids.map((id) => {
                  const c = contextsMap[id];
                  return c ? (
                    <Tag
                      key={id}
                      color={c.color || "geekblue"}
                      style={{ fontSize: 10 }}
                    >
                      @{c.name}
                    </Tag>
                  ) : null;
                })}

                {/* Fecha */}
                {task.due_date && (
                  <Text type="secondary" style={{ fontSize: 11 }}>
                    📅 {new Date(task.due_date).toLocaleDateString("es")}
                  </Text>
                )}
              </Space>
            </div>
          </List.Item>
        );
      }}
    />
  );
}

// ---------------------------------------------------------------------------
// Skeleton
// ---------------------------------------------------------------------------

function TaskListSkeleton() {
  return (
    <List
      dataSource={[1, 2, 3, 4, 5]}
      renderItem={() => (
        <List.Item>
          <div style={{ width: "100%" }}>
            <Skeleton
              active
              title={false}
              paragraph={{ rows: 1, width: "80%" }}
            />
          </div>
        </List.Item>
      )}
    />
  );
}