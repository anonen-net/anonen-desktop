import React from "react";

interface LowVolumeIconProps {
  width?: number;
  height?: number;
  color?: string;
}

const LowVolumeIcon: React.FC<LowVolumeIconProps> = ({
  width = 24,
  height = 24,
  color = "#FBBF24",
}) => {
  return (
    <svg
      width={width}
      height={height}
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <path d="M12 2L1 21h22L12 2zm0 4l7.53 13H4.47L12 6z" fill={color} />
      <rect x="11" y="10" width="2" height="5" rx="1" fill={color} />
      <circle cx="12" cy="17.5" r="1" fill={color} />
    </svg>
  );
};

export default LowVolumeIcon;
