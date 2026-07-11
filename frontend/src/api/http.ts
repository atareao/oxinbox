export interface User {
  sub: string;
  email: string;
  name: string;
}

export interface Project {
  id: string;
  name: string;
  color: string | null;
  created_at: string;
}

export interface Context {
  id: string;
  name: string;
  color: string | null;
  created_at: string;
}

export interface Task {
  id: string;
  completed: boolean;
  priority: string | null;
  description: string;
  project_ids: string[];
  context_ids: string[];
  status: "inbox" | "todo" | "doing" | "done" | "someday";
  created_at: string;
  updated_at: string;
  completed_at: string | null;
  due_date: string | null;
}

import { getToken } from "../store/auth";

// ---------------------------------------------------------------------------
// Fetcher helper
// ---------------------------------------------------------------------------

async function fetcher<T>(path: string, opts?: { method?: string; body?: unknown }): Promise<T> {
  const token = getToken();
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (token) headers["Authorization"] = `Bearer ${token}`;

  const res = await fetch(path, {
    method: opts?.method ?? (opts?.body ? "POST" : "GET"),
    headers,
    body: opts?.body ? JSON.stringify(opts.body) : undefined,
  });

  if (!res.ok) {
    const text = await res.text().catch(() => "unknown error");
    throw new Error(`HTTP ${res.status}: ${text}`);
  }

  if (res.status === 204) return undefined as T;
  return res.json();
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

export async function fetchMe(): Promise<User> {
  return fetcher<User>("/api/me");
}

// ---------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------

export async function fetchTasks(): Promise<Task[]> {
  return fetcher<Task[]>("/api/tasks");
}

export async function createTask(data: {
  description: string;
  priority?: string | null;
  project_ids?: string[];
  context_ids?: string[];
  due_date?: string | null;
  status?: string;
}): Promise<Task> {
  return fetcher<Task>("/api/tasks", { method: "POST", body: data });
}

export async function updateTask(id: string, data: Partial<{
  description: string;
  priority: string | null;
  project_ids: string[];
  context_ids: string[];
  due_date: string | null;
  status: string;
}>): Promise<Task> {
  return fetcher<Task>(`/api/tasks/${id}`, { method: "PUT", body: data });
}

export async function deleteTask(id: string): Promise<void> {
  return fetcher<void>(`/api/tasks/${id}`, { method: "DELETE" });
}

// ---------------------------------------------------------------------------
// Projects / Contexts (read-only for display)
// ---------------------------------------------------------------------------

export async function fetchProjects(): Promise<Project[]> {
  return fetcher<Project[]>("/api/projects");
}

export async function fetchContexts(): Promise<Context[]> {
  return fetcher<Context[]>("/api/contexts");
}

// ---------------------------------------------------------------------------
// AI
// ---------------------------------------------------------------------------

export async function textCapture(text: string): Promise<Task> {
  return fetcher<Task>("/api/text-capture", { method: "POST", body: { text } });
}

export async function transcribeAudio(audio: Blob): Promise<string> {
  const token = getToken();
  const form = new FormData();
  form.append("audio", audio, "recording.webm");

  const res = await fetch("/api/transcribe", {
    method: "POST",
    headers: token ? { Authorization: `Bearer ${token}` } : undefined,
    body: form,
  });

  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return (await res.json()).text;
}

export async function queryTasks(query: string): Promise<{
  sql: string; results: Task[]; answer: string;
}> {
  return fetcher("/api/query", { method: "POST", body: { query } });
}

// ---------------------------------------------------------------------------
// Prompt Config
// ---------------------------------------------------------------------------

export interface PromptConfig {
  user_id: string;
  system_instructions: string;
  few_shot_examples: string;
  rules: string;
  updated_at: string;
}

export interface UpdatePromptRequest {
  system_instructions: string;
  few_shot_examples: string;
  rules: string;
}

export async function fetchPromptConfig(): Promise<PromptConfig> {
  return fetcher<PromptConfig>("/api/prompts");
}

export async function updatePromptConfig(data: UpdatePromptRequest): Promise<PromptConfig> {
  return fetcher<PromptConfig>("/api/prompts", { method: "PUT", body: data });
}