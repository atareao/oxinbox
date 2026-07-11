import { useState } from "react";
import { Input, Spin, Typography } from "antd";
import { SendOutlined } from "@ant-design/icons";
import { textCapture, type Task } from "../api/http";

const { Text } = Typography;

interface Props {
  onTaskCreated: (task: Task) => void;
}

export default function TextCapture({ onTaskCreated }: Props) {
  const [value, setValue] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (text: string) => {
    if (!text || loading) return;

    setLoading(true);
    setError(null);

    try {
      const task = await textCapture(text);
      onTaskCreated(task);
      setValue("");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Error al crear tarea");
    } finally {
      setLoading(false);
    }
  };

  const handlePressEnter = () => {
    handleSubmit(value.trim());
  };

  return (
    <div style={{ marginBottom: 12 }}>
      <Input
        value={value}
        onChange={(e) => setValue(e.target.value)}
        placeholder="Escribe una tarea... (+proyecto @contexto prioridad:A)"
        suffix={
          loading ? <Spin size="small" /> : <SendOutlined style={{ color: "#9494a8", cursor: "pointer" }} onClick={() => handleSubmit(value.trim())} />
        }
        onPressEnter={handlePressEnter}
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