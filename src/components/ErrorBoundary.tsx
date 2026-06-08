import { Component, type ReactNode } from "react";
import { useI18n } from "../i18n";

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  showDetail: boolean;
}

class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null, showDetail: false };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error, showDetail: false };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error("[ErrorBoundary]", error, info.componentStack);
  }

  private t(key: string): string {
    return useI18n.getState().t(key);
  }

  private handleReload = () => {
    window.location.reload();
  };

  private handleReset = () => {
    localStorage.clear();
    window.location.reload();
  };

  private toggleDetail = () => {
    this.setState((prev) => ({ showDetail: !prev.showDetail }));
  };

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) return this.props.fallback;

      return (
        <div className="flex flex-col items-center justify-center h-screen bg-bg-primary text-text-primary p-8">
          <div className="max-w-md w-full bg-bg-secondary border border-border rounded-xl p-6 text-center">
            <div className="w-12 h-12 rounded-full bg-error/15 flex items-center justify-center mx-auto mb-4">
              <span className="text-error text-2xl">!</span>
            </div>
            <h2 className="text-lg font-semibold mb-2">{this.t("errorBoundary.title")}</h2>
            <p className="text-sm text-text-secondary mb-4">
              {this.t("errorBoundary.description")}
            </p>

            <button
              className="text-xs text-text-secondary hover:text-text-primary underline mb-4"
              onClick={this.toggleDetail}
            >
              {this.state.showDetail
                ? this.t("errorBoundary.hideDetail")
                : this.t("errorBoundary.showDetail")}
            </button>

            {this.state.showDetail && (
              <pre className="text-xs text-error bg-error/10 rounded-lg p-3 mb-4 text-left overflow-auto max-h-40 whitespace-pre-wrap break-all">
                {this.state.error?.message}
                {"\n\n"}
                {this.state.error?.stack}
              </pre>
            )}

            <div className="flex gap-3 justify-center">
              <button
                className="px-4 py-2 rounded-lg bg-primary text-white text-sm font-medium hover:bg-primary-hover transition-colors"
                onClick={this.handleReload}
              >
                {this.t("errorBoundary.reload")}
              </button>
              <button
                className="px-4 py-2 rounded-lg bg-bg-tertiary text-text-secondary text-sm font-medium hover:text-text-primary transition-colors"
                onClick={this.handleReset}
              >
                {this.t("errorBoundary.reset")}
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;
