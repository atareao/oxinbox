import { useState, useRef } from "react";
import { Modal, Input, Button, Typography, Space, Spin } from "antd";
import { AudioOutlined, SendOutlined, StopOutlined } from "@ant-design/icons";
import { getToken } from "../store/auth";
import { queryTasks } from "../api/http";

const { Text, Paragraph } = Typography;

interface Props {
  open: boolean;
  onClose: () => void;
}

export default function QueryDialog({ open, onClose }: Props) {
  const [mode, setMode] = useState<"text" | "voice">("text");
  const [text, setText] = useState("");
  const [loading, setLoading] = useState(false);
  const [response, setResponse] = useState<{ sql: string; answer: string } | null>(null);
  const [recording, setRecording] = useState(false);
  const mediaRecorder = useRef<MediaRecorder | null>(null);
  const chunks = useRef<Blob[]>([]);

  const handleTextSubmit = async () => {
    const q = text.trim();
    if (!q || loading) return;
    setLoading(true);
    setResponse(null);
    try {
      const res = await queryTasks(q);
      setResponse(res);
    } catch { /* ignore */ }
    finally { setLoading(false); }
  };

  const startVoiceRecording = async () => {
    try {
      chunks.current = [];
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const mr = new MediaRecorder(stream, { mimeType: "audio/webm" });
      mediaRecorder.current = mr;

      mr.ondataavailable = (e) => {
        if (e.data.size > 0) chunks.current.push(e.data);
      };

      mr.onstop = async () => {
        stream.getTracks().forEach((t) => t.stop());
        const blob = new Blob(chunks.current, { type: "audio/webm" });

        setLoading(true);
        setResponse(null);

        try {
          // Transcribe first
          const token = getToken();
          const form = new FormData();
          form.append("audio", blob, "recording.webm");
          const transcribeRes = await fetch("/api/transcribe", {
            method: "POST",
            headers: token ? { Authorization: `Bearer ${token}` } : undefined,
            body: form,
          });

          if (transcribeRes.ok) {
            const { text: transcribed } = await transcribeRes.json();
            const res = await queryTasks(transcribed);
            setResponse(res);
            setText(transcribed);
          }
        } catch { /* ignore */ }
        finally { setLoading(false); }
      };

      mr.start();
      setRecording(true);
    } catch { /* no mic */ }
  };

  const stopVoiceRecording = () => {
    mediaRecorder.current?.stop();
    setRecording(false);
  };

  const reset = () => {
    setText("");
    setResponse(null);
    setLoading(false);
    setMode("text");
  };

  return (
    <Modal
      title="Preguntar"
      open={open}
      onCancel={() => { reset(); onClose(); }}
      footer={null}
      width={520}
    >
      <Space direction="vertical" style={{ width: "100%" }}>
        {mode === "text" ? (
          <Input
            placeholder="Ej: ¿qué tareas tengo para hoy?"
            value={text}
            onChange={(e) => setText(e.target.value)}
            onPressEnter={handleTextSubmit}
            suffix={
              <Space>
                <Button type="text" size="small" icon={<AudioOutlined />} onClick={() => setMode("voice")} />
                <Button type="text" size="small" icon={<SendOutlined />} onClick={handleTextSubmit} disabled={!text.trim() || loading} />
              </Space>
            }
            disabled={loading}
          />
        ) : (
          <div style={{ textAlign: "center", padding: 16 }}>
            {!recording ? (
              <Button
                shape="circle"
                size="large"
                icon={<AudioOutlined style={{ fontSize: 20 }} />}
                onClick={startVoiceRecording}
                style={{ width: 56, height: 56 }}
              />
            ) : (
              <Button
                danger
                shape="circle"
                size="large"
                icon={<StopOutlined style={{ fontSize: 20 }} />}
                onClick={stopVoiceRecording}
                className="pulse-recording"
                style={{ width: 56, height: 56 }}
              />
            )}
            <div style={{ marginTop: 8 }}>
              <Text type="secondary" style={{ fontSize: 12 }}>
                {recording ? "Grabando... pulsa para detener" : "Pulsa para hablar"}
              </Text>
            </div>
          </div>
        )}

        {mode === "voice" && !recording && (
          <Button type="link" size="small" onClick={() => setMode("text")}>
            Escribir en su lugar
          </Button>
        )}

        {loading && (
          <div style={{ textAlign: "center", padding: 16 }}>
            <Spin />
            <div><Text type="secondary" style={{ fontSize: 12 }}>Procesando...</Text></div>
          </div>
        )}

        {response && (
          <div className="fade-in-up" style={{ background: "#181825", padding: 16, borderRadius: 8, marginTop: 8 }}>
            <Paragraph style={{ fontSize: 14, lineHeight: 1.6, margin: 0 }}>
              {response.answer}
            </Paragraph>
            {response.sql && (
              <details style={{ marginTop: 8 }}>
                <summary style={{ fontSize: 11, color: "#64647a", cursor: "pointer" }}>SQL</summary>
                <pre style={{ fontSize: 11, marginTop: 4, padding: 8, background: "#111118", borderRadius: 4, overflow: "auto", color: "#9494a8" }}>
                  {response.sql}
                </pre>
              </details>
            )}
          </div>
        )}
      </Space>
    </Modal>
  );
}