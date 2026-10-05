import React from "react";

interface SettingsGroupProps {
  title?: string;
  description?: string;
  children: React.ReactNode;
}

export const SettingsGroup: React.FC<SettingsGroupProps> = ({
  title,
  description,
  children,
}) => {
  return (
    <div className="space-y-2">
      {title && (
        <div>
          <h2 className="inline-block border-2 border-border bg-surface rounded-sm px-2 py-0.5 font-mono text-xs font-bold uppercase tracking-wider">
            {title}
          </h2>
          {description && (
            <p className="text-xs text-muted mt-1">{description}</p>
          )}
        </div>
      )}
      <div className="bg-surface border-2 border-border rounded-md shadow-hard-sm overflow-visible">
        <div className="divide-y divide-border/20">{children}</div>
      </div>
    </div>
  );
};
