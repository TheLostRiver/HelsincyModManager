import type { ReactNode } from "react";
import { AppFrame } from "./frame/AppFrame";

type AppShellProps = {
  children: ReactNode;
};

export function AppShell({ children }: AppShellProps) {
  return <AppFrame>{children}</AppFrame>;
}
