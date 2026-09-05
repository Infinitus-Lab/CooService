import { useState, type ReactNode } from 'react';
import Modal from './Modal';

export interface ConfirmState {
  message: string;
  resolve: (ok: boolean) => void;
}

/**
 * 统一的确认弹窗（替代浏览器原生 confirm）。
 * 返回 [弹窗节点(渲染在页面尾部), askConfirm(message) → Promise<boolean>]。
 */
export function useConfirm(): [ReactNode, (message: string) => Promise<boolean>] {
  const [state, setState] = useState<ConfirmState | null>(null);

  const askConfirm = (message: string) =>
    new Promise<boolean>((resolve) => setState({ message, resolve }));

  const close = (ok: boolean) => {
    state?.resolve(ok);
    setState(null);
  };

  const element = (
    <Modal
      open={state !== null}
      title="确认操作"
      onClose={() => close(false)}
      showCancel={false}
      footer={
        <>
          <button className="btn" onClick={() => close(false)}>
            取消
          </button>
          <button className="btn btn-error" onClick={() => close(true)}>
            确认
          </button>
        </>
      }
    >
      <p className="text-sm">{state?.message}</p>
    </Modal>
  );

  return [element, askConfirm];
}