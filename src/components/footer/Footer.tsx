import React from "react";

import ModelSelector from "../model-selector";
import UpdateChecker from "../update-checker";

const Footer: React.FC = () => {
  return (
    <div className="w-full border-t border-border/20 pt-3">
      <div className="flex justify-between items-center text-xs px-4 pb-3 text-muted">
        <div className="flex items-center gap-4">
          <ModelSelector />
        </div>

        <UpdateChecker />
      </div>
    </div>
  );
};

export default Footer;
