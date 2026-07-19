import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { api, isDeploymentActive } from "../api";
import { EmptyState } from "../components/EmptyState";
import { StatusPill } from "../components/StatusPill";
import { timeAgo } from "../time";

export function DeploymentsPage() {
  const navigate = useNavigate();

  const deploymentsQuery = useQuery({
    queryKey: ["all-deployments"],
    queryFn: api.listDeployments,
    refetchInterval: (q) =>
      q.state.data?.some((d) => isDeploymentActive(d)) ? 2500 : false,
  });
  const projectsQuery = useQuery({
    queryKey: ["projects"],
    queryFn: api.listProjects,
  });

  const deployments = deploymentsQuery.data;
  const projectNames = new Map(
    (projectsQuery.data ?? []).map((p) => [p.id, p.name]),
  );

  return (
    <div>
      <h1 className="text-xl font-semibold tracking-tight">Deployments</h1>
      <p className="mt-1 text-sm text-zinc-500">
        Every deploy and rollback across your projects, newest first.
      </p>

      <div className="mt-8">
        {deploymentsQuery.isLoading ? null : !deployments ||
          deployments.length === 0 ? (
          <EmptyState
            title="Nothing deployed yet"
            description="Once a project deploys, its release history and logs show up here."
          />
        ) : (
          <div className="overflow-hidden rounded-xl border border-zinc-800 bg-zinc-900/40">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-zinc-800 text-left text-xs text-zinc-500">
                  <th className="px-4 py-3 font-medium">Project</th>
                  <th className="px-4 py-3 font-medium">Release</th>
                  <th className="hidden px-4 py-3 font-medium md:table-cell">
                    Image
                  </th>
                  <th className="px-4 py-3 font-medium">Status</th>
                  <th className="px-4 py-3 text-right font-medium">When</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-zinc-800/70">
                {deployments.map((d) => (
                  <tr
                    key={d.id}
                    onClick={() =>
                      navigate({
                        to: "/projects/$id",
                        params: { id: d.project_id },
                      })
                    }
                    className="cursor-pointer transition hover:bg-zinc-900/70"
                  >
                    <td className="px-4 py-3 text-zinc-200">
                      {projectNames.get(d.project_id) ?? "—"}
                      {d.kind === "rollback" && (
                        <span className="ml-2 rounded bg-zinc-500/10 px-1.5 py-0.5 text-[10px] font-medium text-zinc-400">
                          rollback
                        </span>
                      )}
                    </td>
                    <td className="px-4 py-3 font-mono text-zinc-400">
                      {d.release_spec.release_id.slice(0, 8)}
                    </td>
                    <td className="hidden max-w-64 truncate px-4 py-3 font-mono text-zinc-500 md:table-cell">
                      {d.release_spec.image}
                    </td>
                    <td className="px-4 py-3">
                      <StatusPill status={d.status} />
                    </td>
                    <td className="whitespace-nowrap px-4 py-3 text-right text-xs text-zinc-600">
                      {timeAgo(d.created_at)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
