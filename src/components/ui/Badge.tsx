import React from "react";

interface BadgeProps {
  children: React.ReactNode;
  variant?:
    | "primary"
    | "success"
    | "secondary"
    | "discount"
    | "surcharge"
    | "preview";

  size?: "md" | "sm";
  className?: string;

  title?: string;
}

const Badge: React.FC<BadgeProps> = ({
  children,
  variant = "primary",
  size = "md",
  className = "",
  title,
}) => {
  const variantClasses = {
    primary: "bg-logo-primary",
    success: "bg-green-500/20 text-green-400",
    secondary: "bg-mid-gray/20 text-text/70",
    discount: "bg-green-500/15 text-green-700 dark:text-green-400",
    surcharge: "bg-amber-500/15 text-amber-700 dark:text-amber-400",

    preview: "bg-violet-500/15 text-violet-700 dark:text-violet-300",
  };
  const sizeClasses = {
    md: "px-3 py-1 text-xs",

    sm: "px-1.5 py-0.5 text-[10px]",
  };

  return (
    <span
      title={title}
      className={`inline-flex items-center whitespace-nowrap rounded-full font-medium ${sizeClasses[size]} ${variantClasses[variant]} ${className}`}
    >
      {children}
    </span>
  );
};

export default Badge;
