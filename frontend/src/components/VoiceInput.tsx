import { useState, useCallback, useRef } from "react";
import { Button, Typography } from "antd";
import { AudioOutlined, LoadingOutlined, CheckCircleOutlined, CloseCircleOutlined } from "@ant-design/icons";
import { getToken } from "../store/auth";

const { Text } = Typography;

type Step = "idle" | "connecting" | "recording" | "transcribing" | "parsing" | "creating" | "done" | "error";

interface Props {
  onTaskCreated: () => void;
}

export default function VoiceInput({ onTaskCreated }: Props) {
  const [step, setStep] = useState<Step>("idle");
  const [transcript, setTranscript] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const ws = useRef<WebSocket | null>(null);
  const stream = useRef<MediaStream | null>(null);
  const recorder = useRef<MediaRecorder | null>(null);

  const closeAll = useCallback(() => {
    recorder.current?.stop();
    stream.current?.getTracks().forEach((t) => t.stop());
    ws.current?.close();
    recorder.current = null;
    stream.current = null;
    ws.current = null;
  }, []);

  const startRecording = async () => {
    try {
      setErrorMsg(null);
      setTranscript(null);
      setStep("connecting");

      const token = getToken();
      if (!token) {
        setStep("error");
        setErrorMsg("No autenticado");
        return;
      }

      const mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true });
      stream.current = mediaStream;

      const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
      const host = window.location.host;
      const socket = new WebSocket(`${proto}//${host}/api/voice?token=${token}`);
      ws.current = socket;

      socket.onopen = () => {
        setStep("recording");

        const mediaRecorder = new MediaRecorder(mediaStream, { mimeType: "audio/webm" });
        recorder.current = mediaRecorder;

        mediaRecorder.ondataavailable = (e) => {
          if (e.data.size > 0 && socket.readyState === WebSocket.OPEN) {
            e.data.arrayBuffer().then((buf) => socket.send(buf));
          }
        };

        mediaRecorder.start(250);
      };

      socket.onmessage = (e) => {
        try {
          const msg = JSON.parse(e.data);
          switch (msg.type) {
            case "status":
              if (msg.step === "transcribing") setStep("transcribing");
              else if (msg.step === "parsing") setStep("parsing");
              else if (msg.step === "creating") setStep("creating");
              break;
            case "transcription":
              setTranscript(msg.text);
              break;
            case "task_created":
              setStep("done");
              setTimeout(() => {
                setStep("idle");
                onTaskCreated();
              }, 1200);
              break;
            case "parse_error":
              setStep("error");
              setErrorMsg("No se pudo interpretar el audio");
              break;
            case "error":
              setStep("error");
              setErrorMsg(msg.message || "Error de procesamiento");
              break;
            case "cancelled":
              setStep("idle");
              break;
          }
        } catch { /* ignore */ }
      };

      socket.onerror = () => {
        setStep("error");
        setErrorMsg("Error de conexión");
        closeAll();
      };

      socket.onclose = () => {
        if (step === "recording") setStep("idle");
      };
    } catch {
      setStep("error");
      setErrorMsg("No se pudo acceder al micrófono");
    }
  };

  const stopRecording = () => {
    recorder.current?.stop();
    recorder.current = null;
    stream.current?.getTracks().forEach((t) => t.stop());
    stream.current = null;

    if (ws.current?.readyState === WebSocket.OPEN) {
      setStep("transcribing");
      ws.current.send(JSON.stringify({ command: "transcribe" }));
    }
  };

  // ---- render ----

  const isBusy = step === "transcribing" || step === "parsing" || step === "creating";

  return (
    <div style={{ textAlign: "center", marginBottom: 16 }}>
      {step === "idle" && (
        <Button
          type="primary"
          shape="circle"
          size="large"
          icon={<AudioOutlined style={{ fontSize: 24 }} />}
          onClick={startRecording}
          style={{ width: 64, height: 64 }}
        />
      )}

      {step === "connecting" && (
        <Button
          shape="circle"
          size="large"
          icon={<LoadingOutlined style={{ fontSize: 24 }} />}
          disabled
          style={{ width: 64, height: 64 }}
        />
      )}

      {step === "recording" && (
        <Button
          danger
          shape="circle"
          size="large"
          className="pulse-recording"
          icon={<AudioOutlined style={{ fontSize: 24 }} />}
          onClick={stopRecording}
          style={{ width: 64, height: 64, borderColor: "#ff4d4f" }}
        />
      )}

      {isBusy && (
        <Button
          shape="circle"
          size="large"
          icon={<LoadingOutlined style={{ fontSize: 24 }} />}
          disabled
          style={{ width: 64, height: 64 }}
        />
      )}

      {step === "done" && (
        <Button
          type="primary"
          shape="circle"
          size="large"
          icon={<CheckCircleOutlined style={{ fontSize: 24, color: "#22c55e" }} />}
          style={{ width: 64, height: 64, borderColor: "#22c55e" }}
        />
      )}

      {step === "error" && (
        <Button
          danger
          shape="circle"
          size="large"
          icon={<CloseCircleOutlined style={{ fontSize: 24 }} />}
          onClick={() => setStep("idle")}
          style={{ width: 64, height: 64 }}
        />
      )}

      {/* State label */}
      <div style={{ marginTop: 8 }}>
        {step === "recording" && (
          <Text type="warning" style={{ fontSize: 12 }}>
            Grabando... pulsa de nuevo para enviar
          </Text>
        )}
        {step === "transcribing" && (
          <Text type="secondary" style={{ fontSize: 12 }}>Transcribiendo...</Text>
        )}
        {step === "parsing" && (
          <Text type="secondary" style={{ fontSize: 12 }}>Analizando...</Text>
        )}
        {step === "creating" && (
          <Text type="secondary" style={{ fontSize: 12 }}>Creando tarea...</Text>
        )}
        {step === "done" && (
          <Text type="success" style={{ fontSize: 12 }}>Tarea creada ✓</Text>
        )}
        {step === "error" && (
          <div>
            <Text type="danger" style={{ fontSize: 12 }}>{errorMsg || "Error"}</Text>
            <Button type="link" size="small" onClick={() => setStep("idle")}>
              Reintentar
            </Button>
          </div>
        )}
      </div>

      {/* Transcript preview */}
      {transcript && isBusy && (
        <div style={{ marginTop: 8, padding: "6px 12px", background: "#181825", borderRadius: 6 }}>
          <Text style={{ fontSize: 13, fontStyle: "italic" }}>"{transcript}"</Text>
        </div>
      )}
    </div>
  );
}