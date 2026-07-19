import { EmptyState } from "../components/EmptyState";

export function DeploymentsPage() {
  return (
    <div>
      <h1 className="text-xl font-semibold tracking-tight">Deployments</h1>
      <p className="mt-1 text-sm text-zinc-500">
        Every build and release across your projects, with live logs.
      </p>
      <div className="mt-8">
        <EmptyState
          title="Nothing deployed yet"
          description="Once a project deploys, its build logs, release history, and rollbacks show up here."
        />
      </div>
    </div>
  );
}
