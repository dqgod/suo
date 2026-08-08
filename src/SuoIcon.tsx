import suoIcon from "../src-tauri/icons/icon.png";

type SuoIconProps = {
  className?: string;
};

export function SuoIcon({ className }: SuoIconProps) {
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
