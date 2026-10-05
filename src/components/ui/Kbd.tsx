import React from "react";
import { formatKeyCombination } from "@/lib/utils/keyboard";
import { useOsType } from "@/hooks/useOsType";

interface KbdProps {
  binding: string;
  size?: "sm" | "md";
  className?: string;
}

export const Kbd: React.FC<KbdProps> = ({
  binding,
  size = "md",
  className = "",
}) => {
  const osType = useOsType();
  const parts = binding
    ? formatKeyCombination(binding, osType).split(" + ")
    : [];
  const cap =
    size === "sm"
      ? "min-w-[1.75rem] px-1.5 py-0.5 text-xs"
      : "min-w-[2.25rem] px-2.5 py-1 text-sm";
  return (
    <span
      className={`inline-flex items-center gap-1 align-middle whitespace-nowrap ${className}`}
    >
      {parts.map((part, index) => (
        <React.Fragment key={`${part}-${index}`}>
          {index > 0 && <span className="text-muted text-xs">+</span>}
          <kbd
            className={`inline-flex items-center justify-center rounded-md border-2 border-b-4 border-border bg-background font-semibold ${cap}`}
          >
            {part}
          </kbd>
        </React.Fragment>
      ))}
    </span>
  );
};
