import { Layout, Button, Typography, Space } from "antd";
import { LogoutOutlined, QuestionOutlined, SettingOutlined } from "@ant-design/icons";
import { Outlet, useNavigate } from "react-router-dom";
import { clearToken } from "../store/auth";
import { useState } from "react";
import QueryDialog from "./QueryDialog";

const { Content } = Layout;
const { Text } = Typography;

export default function AppLayout() {
  const navigate = useNavigate();
  const [queryOpen, setQueryOpen] = useState(false);

  return (
    <>
      <Layout style={{ minHeight: "100vh" }}>
        {/* Top bar */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "12px 20px",
            borderBottom: "1px solid #1e1e2e",
            background: "#0c0c14",
            position: "sticky",
            top: 0,
            zIndex: 100,
          }}
        >
          <Space>
            <Text className="logo-text" style={{ fontSize: 18, fontWeight: 700 }}>
              oxinbox
            </Text>
          </Space>
          <Space size={4}>
            <Button
              type="text"
              icon={<SettingOutlined />}
              onClick={() => navigate("/settings/prompts")}
              style={{ color: "#9494a8" }}
            />
            <Button
              type="text"
              icon={<QuestionOutlined />}
              onClick={() => setQueryOpen(true)}
              style={{ color: "#9494a8" }}
            />
            <Button
              type="text"
              icon={<LogoutOutlined />}
              onClick={() => { clearToken(); navigate("/login", { replace: true }); }}
              style={{ color: "#9494a8" }}
            />
          </Space>
        </div>

        <Content style={{ maxWidth: 640, margin: "0 auto", padding: "16px", width: "100%" }}>
          <Outlet />
        </Content>
      </Layout>

      <QueryDialog open={queryOpen} onClose={() => setQueryOpen(false)} />
    </>
  );
}