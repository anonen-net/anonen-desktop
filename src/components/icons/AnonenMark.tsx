import React from "react";

const AnonenMark = ({
  width,
  height,
  className,
}: {
  width?: number | string;
  height?: number | string;
  className?: string;
}) => {
  return (
    <svg
      width={width}
      height={height}
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        d="M7 2 H17 A5 5 0 0 1 22 7 V12 A5 5 0 0 1 17 17 H12 L5 22.5 V16.4 A5 5 0 0 1 2 12 V7 A5 5 0 0 1 7 2 Z
           M7.5 8 a1 1 0 0 1 2 0 v3 a1 1 0 0 1 -2 0 Z
           M11 6 a1 1 0 0 1 2 0 v7 a1 1 0 0 1 -2 0 Z
           M14.5 8 a1 1 0 0 1 2 0 v3 a1 1 0 0 1 -2 0 Z"
        fill="currentColor"
      />
    </svg>
  );
};

export default AnonenMark;
