import { useEffect, useId, useRef, type ReactNode } from "react";
import { appearanceRedesign } from "./i18n/appearance-redesign";
import "./SettingsDialog.css";

type SettingsDialogProps = {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
  className?: string;
  footer?: ReactNode;
  closeLabel?: string;
};

export default function SettingsDialog({ open, title, onClose, children, className = "", footer, closeLabel = appearanceRedesign.close }: SettingsDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(null);
  const titleId = useId();

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (open && !dialog.open) {
      returnFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      dialog.showModal();
      (dialog.querySelector<HTMLElement>("[data-dialog-autofocus]") ?? dialog.querySelector<HTMLElement>("[autofocus]") ?? dialog.querySelector<HTMLElement>("input, button"))?.focus();
    } else if (!open && dialog.open) {
      dialog.close();
      returnFocusRef.current?.focus();
    }
    return () => {
      if (dialog.open) dialog.close();
      returnFocusRef.current?.focus();
    };
  }, [open]);

  return <dialog ref={dialogRef} className={`settings-dialog ${className}`} aria-labelledby={titleId} onCancel={(event) => { event.preventDefault(); onClose(); }}>
    <header className="settings-dialog-header"><h2 id={titleId}>{title}</h2><button type="button" className="settings-dialog-close" aria-label={closeLabel} onClick={onClose}>×</button></header>
    <div className="settings-dialog-body">{children}</div>
    {footer && <footer className="settings-dialog-footer">{footer}</footer>}
  </dialog>;
}
