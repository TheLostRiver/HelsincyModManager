import { AppShell } from "./app/AppShell";
import { DashboardPage } from "./features/dashboard/DashboardPage";

export function App() {
  return (
    <AppShell>
      <DashboardPage />
    </AppShell>
  );
}
