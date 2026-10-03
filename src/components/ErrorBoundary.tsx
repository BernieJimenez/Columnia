import { Component, type ErrorInfo, type ReactNode } from "react";

interface ErrorBoundaryProps {
  children: ReactNode;
  /** A new value clears a caught error, for example the active phase. */
  resetKey?: unknown;
  fallback: (error: Error, retry: () => void) => ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
  resetKey: unknown;
}

/**
 * ARQ-03: an exception while rendering shows a message instead of an empty
 * window. The data stays in the Rust engine, so retrying or moving to another
 * phase keeps the work.
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null, resetKey: this.props.resetKey };

  static getDerivedStateFromError(error: unknown): Partial<ErrorBoundaryState> {
    return { error: error instanceof Error ? error : new Error(String(error)) };
  }

  static getDerivedStateFromProps(props: ErrorBoundaryProps, state: ErrorBoundaryState): Partial<ErrorBoundaryState> | null {
    return props.resetKey === state.resetKey ? null : { error: null, resetKey: props.resetKey };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Columnia: error de interfaz", error, info.componentStack);
  }

  private retry = () => this.setState({ error: null });

  render() {
    return this.state.error ? this.props.fallback(this.state.error, this.retry) : this.props.children;
  }
}

/** What a phase shows when it fails: the sidebar and the other phases still work. */
export function PhaseErrorMessage({ error, onRetry }: { error: Error; onRetry: () => void }) {
  return (
    <section className="notice notice--error" role="alert" aria-labelledby="phase-error-title">
      <h2 id="phase-error-title">Esta etapa tuvo un error inesperado</h2>
      <p>Tus datos siguen cargados. Puedes reintentar o ir a otra etapa desde la barra lateral.</p>
      <p><small>Detalle: {error.message}</small></p>
      <button type="button" className="secondary-action" onClick={onRetry}>Reintentar</button>
    </section>
  );
}
