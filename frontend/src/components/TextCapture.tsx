import { useState, useRef } from "react";
import { Input, Spin, Typography } from "antd";
import { SendOutlined } from "@ant-design/icons";
import { textCapture, type Task } from "../api/http";

const { Text } = Typography;

interface Props {
  onTaskCreated: (task: Task) => void;
}

export default function TextCapture({ onTaskCreated }: Props) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const handleSubmit = async (value: string) => {
    const text = value.trim();
    if (!text || loading) return;

    setLoading(true);
    setError(null);

    try {
      const task = await textCapture(text);
      onTaskCreated(task);
      if (inputRef.current) inputRef.current.value = "";
    } catch (e) {
      setError(e instanceof Error ? e.message : "Error al crear tarea");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ marginBottom: 12 }}>
      <Input
        ref={inputRef as React.Ref<any>}
        placeholder="Escribe una tarea... (+proyecto @contexto prioridad:A)"
        suffix={
          loading ? <Spin size="small" /> : <SendOutlined style={{ color: "#9494a8", cursor: "pointer" }} onClick={() => handleSubmit(inputRef.current?.value || "")} />
        }
        onPressEnter={(e) => handleSubmit((e.target as HTMLInputElement).value)}
        disabled={loading}
        style={{ fontSize: 14 }}
      />
      {error && (
        <Text type="danger" style={{ fontSize: 11, display: "block", marginTop: 4 }}>
          {error}
        </Text>
      )}
    </div>
  );
}