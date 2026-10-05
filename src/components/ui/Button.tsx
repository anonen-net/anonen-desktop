import React from "react";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?:
    | "primary"
    | "primary-soft"
    | "secondary"
    | "danger"
    | "danger-ghost"
    | "ghost";
  size?: "sm" | "md" | "lg";
}

export const Button: React.FC<ButtonProps> = ({
  children,
  className = "",
  variant = "primary",
  size = "md",
  ...props
}) => {
  const baseClasses =
    "font-mono font-semibold border-2 rounded focus:outline-none transition-all disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer";

  const pressable =
    "shadow-hard-xs hover:shadow-none hover:translate-x-[2px] hover:translate-y-[2px]";

  const variantClasses = {
    primary: `text-background bg-text border-border ${pressable} hover:text-white hover:bg-background-ui hover:border-background-ui`,
    "primary-soft": `text-text bg-logo-primary/15 border-border ${pressable} hover:bg-logo-primary/25`,
    secondary: `text-text bg-surface border-border ${pressable} hover:bg-surface-alt`,
    danger: `text-white bg-red-600 border-border ${pressable} hover:bg-red-700`,
    "danger-ghost":
      "text-red-600 border-transparent hover:bg-red-500/10 focus:bg-red-500/20",
    ghost:
      "text-current border-transparent hover:bg-surface-alt focus:bg-surface-alt",
  };

  const sizeClasses = {
    sm: "px-2 py-1 text-xs",
    md: "px-4 py-[5px] text-sm",
    lg: "px-4 py-2 text-base",
  };

  return (
    <button
      className={`${baseClasses} ${variantClasses[variant]} ${sizeClasses[size]} ${className}`}
      {...props}
    >
      {children}
    </button>
  );
};
