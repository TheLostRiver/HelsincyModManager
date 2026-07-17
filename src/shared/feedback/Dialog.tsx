import type { ModalSurfaceProps } from "./ModalSurface";
import { ModalSurface } from "./ModalSurface";

export type DialogProps = Omit<ModalSurfaceProps, "kind">;

export function Dialog(props: DialogProps) {
  return <ModalSurface {...props} kind="dialog" />;
}
