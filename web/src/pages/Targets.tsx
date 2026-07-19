import { EmptyState } from "../components/EmptyState";

export function TargetsPage() {
  return (
    <div>
      <h1 className="text-xl font-semibold tracking-tight">Targets</h1>
      <p className="mt-1 text-sm text-zinc-500">
        Servers and clusters your apps deploy onto. Bring any Linux VPS or a
        Kubernetes cluster.
      </p>
      <div className="mt-8">
        <EmptyState
          title="No targets connected"
          description="Connect a server over SSH and Projexity will install Docker and a reverse proxy with automatic HTTPS — or point it at your Kubernetes cluster."
          actionLabel="Connect server"
        />
      </div>
    </div>
  );
}
