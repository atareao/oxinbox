import { useEffect, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { Button, Card, Divider, Input, Typography, Space } from "antd";
import { GoogleOutlined, BugOutlined } from "@ant-design/icons";
import { getToken } from "../store/auth";

const { Title, Text } = Typography;

export default function LoginPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [devEmail, setDevEmail] = useState("user@oxinbox.app");
  const [error] = useState<string | null>(null);

  useEffect(() => {
    const token = searchParams.get("token");
    if (token) {
      localStorage.setItem("oxinbox_token", token);
      sessionStorage.setItem("oxinbox_token", token);
      navigate("/", { replace: true });
    }
    const existingToken = getToken();
    if (existingToken) {
      navigate("/", { replace: true });
    }
  }, [searchParams, navigate]);

  const handleDevLogin = () => {
    window.location.href = `/auth/dev-login?email=${encodeURIComponent(devEmail)}`;
  };

  return (
    <div style={{ display: "flex", justifyContent: "center", alignItems: "center", minHeight: "100vh", background: "#141414" }}>
      <Card style={{ width: 400, textAlign: "center" }}>
        <Title level={2}>oxinbox</Title>
        <Text type="secondary">Captura instantánea de tareas por voz</Text>
        <Divider />
        {error && <Text type="danger" style={{ display: "block", marginBottom: 16 }}>{error}</Text>}
        <Button type="primary" size="large" block icon={<GoogleOutlined />} onClick={() => { window.location.href = "/auth/login"; }}>
          Iniciar sesión
        </Button>
        <Divider><Text type="secondary" style={{ fontSize: 12 }}>Desarrollo</Text></Divider>
        <Space.Compact style={{ width: "100%" }}>
          <Input value={devEmail} onChange={(e) => setDevEmail(e.target.value)} placeholder="Email para dev login" />
          <Button icon={<BugOutlined />} onClick={handleDevLogin}>Dev</Button>
        </Space.Compact>
      </Card>
    </div>
  );
}