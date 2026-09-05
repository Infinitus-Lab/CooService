import { useEffect, useRef, type ReactNode } from 'react';

export default function Modal({
  open,
  title,
  onClose,
  children,
  footer,
  showCancel = true,
}: {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  /** footer 自带动作集（含取消）时关掉内置取消，避免双按钮 */
  showCancel?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog ref={ref} className="modal" onClose={onClose}>
      <div className="modal-box max-w-lg">
        <h3 className="text-lg font-bold">{title}</h3>
        <div className="space-y-3 py-4">{children}</div>
        <div className="modal-action">
          {showCancel && (
            <button type="button" className="btn btn-ghost" onClick={onClose}>
              取消
            </button>
          )}
          {footer}
        </div>
      </div>
      <form method="dialog" className="modal-backdrop">
        <button type="submit">close</button>
      </form>
    </dialog>
  );
}
