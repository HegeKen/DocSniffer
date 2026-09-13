import { Component, ErrorInfo, ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
  info: ErrorInfo | null;
}

export default class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, info: null };

  static getDerivedStateFromError(error: Error): State {
    return { error, info: null };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    this.setState({ error, info });
    console.error("[ErrorBoundary]", error, info);
  }

  reset = () => this.setState({ error: null, info: null });

  render() {
    if (this.state.error) {
      const { error, info } = this.state;
      return (
        <div style={{
          padding: 32,
          fontFamily: '-apple-system, "Segoe UI", Roboto, sans-serif',
          background: "#fff",
          minHeight: "100vh",
        }}>
          <h2 style={{ color: "#dc2626", marginTop: 0 }}>页面渲染出错</h2>
          <p style={{ color: "#374151" }}>切换到该标签时发生异常：</p>
          <pre style={{
            background: "#fef2f2",
            border: "1px solid #fecaca",
            padding: 12,
            borderRadius: 8,
            overflow: "auto",
            fontSize: 13,
            lineHeight: 1.6,
          }}>
            {error.name}: {error.message}
            {"\n\n"}
            {error.stack}
            {info?.componentStack && `\n组件栈:${info.componentStack}`}
          </pre>
          <button
            onClick={this.reset}
            style={{ marginTop: 16, padding: "8px 16px", fontSize: 14 }}
          >
            重试
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
