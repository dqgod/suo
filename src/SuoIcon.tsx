import { useId } from "react";
import suoIcon from "../src-tauri/icons/icon.png";

export type SettingsIconStyle = "transparentColor" | "monochrome" | "original";

type SuoIconProps = {
  className?: string;
  iconStyle?: SettingsIconStyle;
};

export function SuoIcon({ className, iconStyle = "transparentColor" }: SuoIconProps) {
  const gradientId = useId();

  if (iconStyle === "original") {
    return (
      <img
        className={className}
        src={suoIcon}
        alt=""
        aria-hidden="true"
        draggable={false}
      />
    );
  }

  const monochrome = iconStyle === "monochrome";
  return (
    <svg
      className={className}
      viewBox="80 44 352 424"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
      focusable="false"
    >
      {!monochrome && (
        <defs>
          <linearGradient id={gradientId} x1="128" y1="80" x2="386" y2="438" gradientUnits="userSpaceOnUse">
            <stop stopColor="#B8AFFF" />
            <stop offset=".55" stopColor="#8B7CFF" />
            <stop offset="1" stopColor="#58D4FF" />
          </linearGradient>
        </defs>
      )}
      <path d="M256 70 408 210 256 442 104 210 256 70Z" fill={monochrome ? "currentColor" : "#8B7CFF"} opacity=".1" />
      <g fill="none" stroke={monochrome ? "currentColor" : `url(#${gradientId})`} strokeLinecap="round" strokeLinejoin="round">
        <path d="M256 70 408 210 256 442 104 210 256 70Z" strokeWidth="23" />
        <path d="m108 210 148 58 148-58M256 268v168" strokeWidth="19" />
      </g>
      <circle cx="256" cy="268" r="13" fill={monochrome ? "currentColor" : "#E9E6FF"} />
    </svg>
  );
}
