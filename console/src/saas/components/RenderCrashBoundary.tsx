import { Component, type ErrorInfo, type ReactNode } from "react";
import { AlertTriangle, RotateCcw } from "lucide-react";

type Props = {
  children: ReactNode;
  resetKey?: string;
  surface: string;
};

type State = {
  error: Error | null;
};

export class RenderCrashBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(`[console] ${this.props.surface} crashed:`, error, info.componentStack);
  }

  componentDidUpdate(prevProps: Props) {
    if (prevProps.resetKey !== this.props.resetKey && this.state.error) {
      this.setState({ error: null });
    }
  }

  render() {
    if (!this.state.error) return this.props.children;

    return (
      <div className="min-h-[360px] grid place-items-center p-6 font-display">
        <div className="w-full max-w-xl rounded-md border border-espelette/30 bg-espelette/[0.06] p-5 text-sm">
          <div className="flex items-center gap-2 text-espelette">
            <AlertTriangle className="h-4 w-4" />
            <div className="font-medium">This view crashed while rendering.</div>
          </div>
          <pre className="mt-3 max-h-32 overflow-auto rounded-md border border-espelette/20 bg-paper p-3 text-[11px] text-ink/80">
            {this.state.error.message}
          </pre>
          <button
            onClick={() => this.setState({ error: null })}
            className="mt-4 inline-flex items-center gap-1.5 rounded-md bg-biscay px-3 py-1.5 text-xs font-medium text-white hover:bg-biscay-2"
          >
            <RotateCcw className="h-3.5 w-3.5" />
            Try again
          </button>
        </div>
      </div>
    );
  }
}
