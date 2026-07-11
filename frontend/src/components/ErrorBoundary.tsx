import { Component, type ReactNode } from "react";
import { Button, Result } from "antd";

interface Props { children: ReactNode }
interface State { hasError: boolean; error?: Error }

export default class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false };

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  render() {
    if (this.state.hasError) {
      return (
        <Result
          status="error"
          title="Algo salió mal"
          subTitle={this.state.error?.message}
          extra={
            <Button type="primary" onClick={() => { this.setState({ hasError: false }); window.location.reload(); }}>
              Recargar
            </Button>
          }
        />
      );
    }
    return this.props.children;
  }
}