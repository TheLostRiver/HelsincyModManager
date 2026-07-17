import type { ModalSurfaceProps } from "./ModalSurface";
import { ModalSurface } from "./ModalSurface";

export type DetailSheetProps = Omit<ModalSurfaceProps, "kind">;

export function DetailSheet(props: DetailSheetProps) {
  return <ModalSurface {...props} kind="sheet" />;
}
