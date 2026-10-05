import React, { useEffect, useRef, useState } from "react";
import { Tooltip } from "./Tooltip";

interface InfoTipProps {
  text: string;

  label: string;

  width?: number;
  className?: string;
}

export const InfoTip: React.FC<InfoTipProps> = ({
  text,
  label,
  width,
  className = "",
}) => {
  const [show, setShow] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!show) return;
    const onClickOutside = (event: MouseEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) {
        setShow(false);
      }
    };
    document.addEventListener("mousedown", onClickOutside);
    return () => document.removeEventListener("mousedown", onClickOutside);
  }, [show]);

  const toggle = () => setShow((value) => !value);

  return (
    <div
      ref={ref}
      className={`relative inline-flex items-center ${className}`}
      onMouseEnter={() => setShow(true)}
      onMouseLeave={() => setShow(false)}
      onClick={toggle}
    >
      <svg
        className="w-4 h-4 text-mid-gray cursor-help hover:text-logo-primary transition-colors duration-200 select-none"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
        aria-label={label}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            toggle();
          }
        }}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
        />
      </svg>
      {show && (
        <Tooltip targetRef={ref} position="top" width={width}>
          <p className="text-xs text-left leading-relaxed">{text}</p>
        </Tooltip>
      )}
    </div>
  );
};
