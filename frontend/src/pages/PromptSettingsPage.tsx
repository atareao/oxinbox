import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import {
  Card, Tabs, Input, Button, Typography, Space, message,
  Spin, Alert, Tag, Divider,
} from "antd";
import {
  SaveOutlined, ReloadOutlined, ArrowLeftOutlined,
} from "@ant-design/icons";
import { fetchPromptConfig, updatePromptConfig, PromptConfig } from "../api/http";

const { Title, Paragraph } = Typography;
const { TextArea } = Input;

export default function PromptSettingsPage() {
  const navigate = useNavigate();
  const [config, setConfig] = useState<PromptConfig | null>(null);
  const [systemInstructions, setSystemInstructions] = useState("");
  const [fewShotExamples, setFewShotExamples] = useState("");
  const [rules, setRules] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [activeTab, setActiveTab] = useState("instructions");
  const [testInput, setTestInput] = useState("");
  const [testOutput, setTestOutput] = useState("");

  useEffect(() => {
    loadConfig();
  }, []);

  async function loadConfig() {
    setLoading(true);
    try {
      const cfg = await fetchPromptConfig();
      setConfig(cfg);
      setSystemInstructions(cfg.system_instructions);
      setFewShotExamples(cfg.few_shot_examples);
      setRules(cfg.rules);
    } catch (err) {
      message.error("Error al cargar configuración");
      console.error(err);
    } finally {
      setLoading(false);
    }
  }

  async function handleSave() {
    setSaving(true);
    try {
      const updated = await updatePromptConfig({
        system_instructions: systemInstructions,
        few_shot_examples: fewShotExamples,
        rules,
      });
      setConfig(updated);
      message.success("Configuración guardada ✅");
    } catch (err) {
      message.error("Error al guardar");
      console.error(err);
    } finally {
      setSaving(false);
    }
  }

  function handleReset() {
    setSystemInstructions("");
    setFewShotExamples("");
    setRules("");
    message.info("Se usarán los valores por defecto al guardar (campos vacíos)");
  }

  function getPreviewPrompt(): string {
    const si = systemInstructions || "(usará valores por defecto)";
    const fs = fewShotExamples || "(usará valores por defecto)";
    const r = rules || "(usará valores por defecto)";
    return [
      `=== INSTRUCCIONES DEL SISTEMA ===`,
      si,
      ``,
      `--- EJEMPLOS ---`,
      ``,
      fs,
      ``,
      `--- REGLAS Y FORMATO ---`,
      ``,
      r,
    ].join("\n");
  }

  function handleTest() {
    setTestOutput(getPreviewPrompt());
  }

  if (loading) {
    return (
      <div style={{ display: "flex", justifyContent: "center", padding: 80 }}>
        <Spin size="large" />
      </div>
    );
  }

  return (
    <div style={{ maxWidth: 900, margin: "0 auto" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
        <Space>
          <Button
            type="text"
            icon={<ArrowLeftOutlined />}
            onClick={() => navigate("/")}
            style={{ color: "#9494a8" }}
          />
          <Title level={3} style={{ margin: 0 }}>
            ⚙️ Ajustes de IA
          </Title>
        </Space>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={handleReset}>
            Restaurar defaults
          </Button>
          <Button type="primary" icon={<SaveOutlined />} onClick={handleSave} loading={saving}>
            Guardar
          </Button>
        </Space>
      </div>

      <Alert
        message="Placeholders disponibles"
        description={
          <div style={{ fontSize: 13 }}>
            Usa <Tag>{"{{source}}"}</Tag> <Tag>{"{{projects}}"}</Tag> <Tag>{"{{contexts}}"}</Tag>{" "}
            <Tag>{"{{recent_tasks}}"}</Tag> en las instrucciones. El sistema los reemplazará
            automáticamente al llamar al LLM.
          </div>
        }
        type="info"
        showIcon
        style={{ marginBottom: 16 }}
      />

      <Card>
        <Tabs
          activeKey={activeTab}
          onChange={setActiveTab}
          items={[
            {
              key: "instructions",
              label: "📝 Instrucciones",
              children: (
                <div>
                  <Paragraph type="secondary">
                    Prompt principal del sistema. Define el rol del asistente, el contexto
                    del usuario y cómo debe comportarse el LLM. Usa {"{{source}}"} para el
                    tipo de entrada (texto/voz), {"{{projects}}"}, {"{{contexts}}"} y{" "}
                    {"{{recent_tasks}}"} para datos del usuario.
                  </Paragraph>
                  <TextArea
                    rows={18}
                    value={systemInstructions}
                    onChange={(e) => setSystemInstructions(e.target.value)}
                    placeholder="Dejar vacío para usar valores por defecto"
                    style={{ fontFamily: "monospace", fontSize: 13 }}
                  />
                </div>
              ),
            },
            {
              key: "examples",
              label: "🔍 Ejemplos few-shot",
              children: (
                <div>
                  <Paragraph type="secondary">
                    Pares usuario/respuesta que muestran al LLM cómo debe parsear las
                    tareas. Cada ejemplo separado por una línea en blanco. Incluye
                    descripción, prioridad, proyecto, contexto y fecha si aplica.
                  </Paragraph>
                  <TextArea
                    rows={18}
                    value={fewShotExamples}
                    onChange={(e) => setFewShotExamples(e.target.value)}
                    placeholder="Dejar vacío para usar valores por defecto"
                    style={{ fontFamily: "monospace", fontSize: 13 }}
                  />
                </div>
              ),
            },
            {
              key: "rules",
              label: "📏 Reglas",
              children: (
                <div>
                  <Paragraph type="secondary">
                    Reglas de extracción, marcadores explícitos (@nombre, +proyecto),
                    inferencia de proyecto/contexto desde lenguaje natural, y formato
                    de respuesta esperado.
                  </Paragraph>
                  <TextArea
                    rows={18}
                    value={rules}
                    onChange={(e) => setRules(e.target.value)}
                    placeholder="Dejar vacío para usar valores por defecto"
                    style={{ fontFamily: "monospace", fontSize: 13 }}
                  />
                </div>
              ),
            },
            {
              key: "preview",
              label: "👁️ Vista previa",
              children: (
                <div>
                  <Paragraph type="secondary">
                    Así se verá el prompt ensamblado con tus configuraciones. Los
                    placeholders como {"{{projects}}"} se muestran tal cual (el backend
                    los reemplaza con datos reales).
                  </Paragraph>
                  <pre
                    style={{
                      background: "#1a1a2e",
                      color: "#e0e0e0",
                      padding: 16,
                      borderRadius: 8,
                      fontSize: 13,
                      fontFamily: "monospace",
                      whiteSpace: "pre-wrap",
                      maxHeight: 500,
                      overflow: "auto",
                    }}
                  >
                    {getPreviewPrompt()}
                  </pre>
                </div>
              ),
            },
            {
              key: "test",
              label: "🧪 Probar",
              children: (
                <div>
                  <Paragraph type="secondary">
                    Escribe un texto de ejemplo y haz clic en "Generar preview" para ver
                    cómo se ensamblaría el prompt completo.
                  </Paragraph>
                  <Space direction="vertical" style={{ width: "100%" }}>
                    <Input.TextArea
                      rows={3}
                      value={testInput}
                      onChange={(e) => setTestInput(e.target.value)}
                      placeholder='Ej: "Añade pepinillos a la lista de la compra"'
                    />
                    <Button icon={<SaveOutlined />} onClick={handleTest}>
                      Generar preview
                    </Button>
                    {testOutput && (
                      <>
                        <Divider />
                        <pre
                          style={{
                            background: "#1a1a2e",
                            color: "#e0e0e0",
                            padding: 16,
                            borderRadius: 8,
                            fontSize: 13,
                            fontFamily: "monospace",
                            whiteSpace: "pre-wrap",
                            maxHeight: 500,
                            overflow: "auto",
                          }}
                        >
                          {testOutput}
                        </pre>
                      </>
                    )}
                  </Space>
                </div>
              ),
            },
          ]}
        />
      </Card>

      {config && (
        <Card size="small" style={{ marginTop: 16 }}>
          <Paragraph type="secondary" style={{ margin: 0 }}>
            Última actualización: {new Date(config.updated_at).toLocaleString("es-ES")}
            &nbsp;·&nbsp; Usuario: {config.user_id}
          </Paragraph>
        </Card>
      )}
    </div>
  );
}