import React from "react";

interface BadgeProps {
  children: React.ReactNode;
  variant?: "primary" | "success" | "secondary" | "discount" | "surcharge";
  className?: string;

  title?: string;
}

const Badge: React.FC<BadgeProps> = ({
  children,
  variant = "primary",
  className = "",
  title,
}) => {
  const variantClasses = {
    primary: "bg-logo-primary",
    success: "bg-green-500/20 text-green-400",
    secondary: "bg-mid-gray/20 text-text/70",
    discount: "bg-green-500/15 text-green-700 dark:text-green-400",
    surcharge: "bg-amber-500/15 text-amber-700 dark:text-amber-400",
  };

  return (
    <span
      title={title}
      className={`inline-flex items-center px-3 py-1 rounded-full text-xs font-medium ${variantClasses[variant]} ${className}`}
    >
      {children}
    </span>
  );
};

export default Badge;
