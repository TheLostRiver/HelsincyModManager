import { AppShell } from "./app/AppShell";
import { FirstLaunchDashboard } from "./features/dashboard/FirstLaunchDashboard";

export function App() {
  return (
    <AppShell>
      <FirstLaunchDashboard />
    </AppShell>
  );
}
