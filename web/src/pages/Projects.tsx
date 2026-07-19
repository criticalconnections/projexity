import { EmptyState } from "../components/EmptyState";

export function ProjectsPage() {
  return (
    <div>
      <h1 className="text-xl font-semibold tracking-tight">Projects</h1>
      <p className="mt-1 text-sm text-zinc-500">
        A project connects a Git repository to a deploy target.
      </p>
      <div className="mt-8">
        <EmptyState
          title="No projects yet"
          description="Connect a GitHub repository and Projexity will build and deploy it to your own server — live HTTPS URL included."
          actionLabel="New project"
        />
      </div>
    </div>
  );
}
