import React, { useCallback, useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";

let openFrames = 0;
let inertBefore = false;

export function holdBackground(root: { inert: boolean } | null): () => void {
  if (openFrames++ === 0 && root) {
    inertBefore = root.inert;
    root.inert = true;
  }
  let released = false;
  return () => {
    if (released) return;
    released = true;
    if (--openFrames === 0 && root) root.inert = inertBefore;
  };
}

let confirmBusy = false;

export function claimConfirm(): boolean {
  if (confirmBusy) return false;
  confirmBusy = true;
  return true;
}

export function releaseConfirm(): void {
  confirmBusy = false;
}

export const DialogFrame: React.FC<{
  labelledBy: string;
  onEscape: () => void;

  fit?: boolean;
  children: React.ReactNode;
}> = ({ labelledBy, onEscape, fit = false, children }) => {
  const onEscapeRef = useRef(onEscape);
  useEffect(() => {
    onEscapeRef.current = onEscape;
  }, [onEscape]);

  const cardRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const opener =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    const release = holdBackground(document.getElementById("root"));
    cardRef.current?.focus();
    return () => {
      release();
      if (opener?.isConnected) opener.focus();
    };
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      onEscapeRef.current();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div
        ref={cardRef}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-labelledby={labelledBy}
        className={`jp-phrase-wrap bg-background border border-mid-gray/20 rounded-lg p-6 ${
          fit ? "w-auto min-w-[16rem] max-w-md" : "max-w-md w-full"
        } mx-4 space-y-4 outline-none`}
      >
        {children}
      </div>
    </div>,
    document.body,
  );
};

export interface ConfirmOptions {
  title?: string;

  body: string;
  okLabel: string;
  cancelLabel: string;

  danger?: boolean;

  align?: "end" | "center";
}

export function useConfirmDialog(): {
  confirmInApp: (options: ConfirmOptions) => Promise<boolean>;
  confirmDialog: React.ReactNode;
} {
  const [options, setOptions] = useState<ConfirmOptions | null>(null);
  const resolver = useRef<((ok: boolean) => void) | null>(null);
  const owns = useRef(false);

  const mounted = useRef(false);
  const labelId = useId();

  const settle = useCallback((ok: boolean) => {
    const resolve = resolver.current;
    resolver.current = null;
    if (owns.current) {
      owns.current = false;
      releaseConfirm();
    }
    setOptions(null);
    resolve?.(ok);
  }, []);

  const confirmInApp = useCallback((next: ConfirmOptions) => {
    if (!mounted.current) return Promise.resolve(false);

    if (!claimConfirm()) return Promise.resolve(false);
    owns.current = true;
    return new Promise<boolean>((resolve) => {
      resolver.current = resolve;
      setOptions(next);
    });
  }, []);

  useEffect(() => {
    mounted.current = true;
    const pending = resolver;
    const mine = owns;
    return () => {
      mounted.current = false;
      pending.current?.(false);
      if (mine.current) {
        mine.current = false;
        releaseConfirm();
      }
    };
  }, []);

  const centered = options?.align === "center";
  const confirmDialog = options && (
    <DialogFrame
      labelledBy={labelId}
      onEscape={() => settle(false)}
      fit={centered}
    >
      {options.title ? (
        <>
          <h2
            id={labelId}
            className={`text-base font-semibold${centered ? " text-center" : ""}`}
          >
            {options.title}
          </h2>
          <p
            className={`text-sm text-text/70 whitespace-pre-line${centered ? " text-center" : ""}`}
          >
            {options.body}
          </p>
        </>
      ) : (
        <p
          id={labelId}
          className={`text-sm whitespace-pre-line${centered ? " text-center" : ""}`}
        >
          {options.body}
        </p>
      )}
      <div
        className={`flex ${centered ? "gap-4 justify-center" : "gap-2 justify-end"}`}
      >
        <button
          type="button"
          className="px-3 py-1.5 text-sm rounded border border-mid-gray/20 hover:bg-mid-gray/10 transition-colors"
          onClick={() => settle(false)}
        >
          {options.cancelLabel}
        </button>
        <button
          type="button"
          className={`px-3 py-1.5 text-sm rounded text-white hover:opacity-90 transition-opacity ${
            options.danger ? "bg-red-600" : "bg-logo-primary"
          }`}
          onClick={() => settle(true)}
        >
          {options.okLabel}
        </button>
      </div>
    </DialogFrame>
  );

  return { confirmInApp, confirmDialog };
}
