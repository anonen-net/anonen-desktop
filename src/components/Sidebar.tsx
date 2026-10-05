import React from "react";
import { useTranslation } from "react-i18next";
import { BookOpen, History, Cpu } from "lucide-react";
import AnonenMark from "./icons/AnonenMark";
import { useSettings } from "../hooks/useSettings";
import {
  GeneralSettings,
  HistorySettings,
  ModelsSettings,
  HowToSettings,
} from "./settings";

export type SidebarSection = keyof typeof SECTIONS_CONFIG;

interface IconProps {
  width?: number | string;
  height?: number | string;
  size?: number | string;
  className?: string;
  [key: string]: any;
}

interface SectionConfig {
  labelKey: string;
  icon: React.ComponentType<IconProps>;
  component: React.ComponentType;
  enabled: (settings: any) => boolean;
}

export const SECTIONS_CONFIG = {
  models: {
    labelKey: "sidebar.models",
    icon: Cpu,
    component: ModelsSettings,
    enabled: () => true,
  },
  history: {
    labelKey: "sidebar.history",
    icon: History,
    component: HistorySettings,
    enabled: () => true,
  },

  howto: {
    labelKey: "sidebar.howto",
    icon: BookOpen,
    component: HowToSettings,
    enabled: () => true,
  },

  general: {
    labelKey: "sidebar.general",
    icon: AnonenMark,
    component: GeneralSettings,
    enabled: () => true,
  },
} as const satisfies Record<string, SectionConfig>;

interface SidebarProps {
  activeSection: SidebarSection;
  onSectionChange: (section: SidebarSection) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
}) => {
  const { t } = useTranslation();
  const { settings } = useSettings();

  const availableSections = Object.entries(SECTIONS_CONFIG)
    .filter(([_, config]) => {
      const c: SectionConfig = config;
      return c.enabled(settings);
    })
    .map(([id, config]) => ({ id: id as SidebarSection, ...config }));

  return (
    <div className="flex flex-col w-40 h-full border-e border-border/20 items-center px-2 pt-4">
      <div className="flex flex-col w-full items-center gap-1">
        {availableSections.map((section) => {
          const Icon = section.icon;
          const isActive = activeSection === section.id;

          return (
            <div
              key={section.id}
              className={`flex gap-2 items-center p-2 w-full cursor-pointer transition-all border-2 rounded ${
                isActive
                  ? "border-border bg-logo-primary/20 shadow-hard-xs"
                  : "border-transparent hover:bg-surface-alt opacity-85 hover:opacity-100"
              }`}
              onClick={() => onSectionChange(section.id)}
            >
              <Icon width={24} height={24} className="shrink-0" />
              <p
                className={`text-sm font-mono truncate ${isActive ? "font-bold" : "font-medium"}`}
                title={t(section.labelKey)}
              >
                {t(section.labelKey)}
              </p>
            </div>
          );
        })}
      </div>
    </div>
  );
};
