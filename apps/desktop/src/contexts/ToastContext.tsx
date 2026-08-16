import { AlertCircle, CheckCircle2, Info, X } from "lucide-react";
import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";

export type ToastTone = "info" | "success" | "error";

interface Toast {
  id: number;
  tone: ToastTone;
  message: string;
}

interface ToastApi {
  show: (tone: ToastTone, message: string) => void;
  info: (message: string) => void;
  success: (message: string) => void;
  error: (message: string) => void;
}

const ToastContext = createContext<ToastApi | null>(null);

export function useToast(): ToastApi {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToast must be used within <ToastProvider>");
  return ctx;
}

const AUTO_DISMISS_MS = 5000;

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(1);

  const dismiss = useCallback((id: number) => {
    setToasts((list) => list.filter((t) => t.id !== id));
  }, []);

  const show = useCallback((tone: ToastTone, message: string) => {
    const id = nextId.current++;
    setToasts((list) => [...list, { id, tone, message }]);
  }, []);

  const api: ToastApi = {
    show,
    info: (m) => show("info", m),
    success: (m) => show("success", m),
    error: (m) => show("error", m),
  };

  return (
    <ToastContext.Provider value={api}>
      {children}
      <ToastViewport toasts={toasts} onDismiss={dismiss} />
    </ToastContext.Provider>
  );
}

function ToastViewport({
  toasts,
  onDismiss,
}: {
  toasts: Toast[];
  onDismiss: (id: number) => void;
}) {
  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[60] flex flex-col gap-2 items-end">
      {toasts.map((t) => (
        <ToastRow key={t.id} toast={t} onDismiss={() => onDismiss(t.id)} />
      ))}
    </div>
  );
}

function ToastRow({ toast, onDismiss }: { toast: Toast; onDismiss: () => void }) {
  useEffect(() => {
    const handle = setTimeout(onDismiss, AUTO_DISMISS_MS);
    return () => clearTimeout(handle);
  }, [onDismiss]);

  const accent =
    toast.tone === "error"
      ? "text-red-400"
      : toast.tone === "success"
        ? "text-green-400"
        : "text-blue-400";
  const Icon =
    toast.tone === "error" ? AlertCircle : toast.tone === "success" ? CheckCircle2 : Info;

  return (
    <div className="pointer-events-auto bg-surface border border-border rounded-lg shadow-popover px-3 py-2 flex items-center gap-2 font-inter text-[13px] text-fg max-w-sm leading-tight">
      <span className={`shrink-0 flex items-center justify-center ${accent}`}>
        <Icon size={16} />
      </span>
      <span className="flex-1 min-w-0 break-words">{toast.message}</span>
      <button
        type="button"
        onClick={onDismiss}
        className="shrink-0 flex items-center justify-center text-fg-subtle hover:text-fg transition-colors"
        aria-label="Dismiss"
      >
        <X size={14} />
      </button>
    </div>
  );
}
