import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { motion } from "motion/react";
import { Rocket } from "lucide-react";
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
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Deployments</h1>
        <p className="mt-1 text-sm text-zinc-500">
          Every deploy and rollback across your projects, newest first.
        </p>
      </div>

      <div className="mt-8">
        {deploymentsQuery.isLoading ? null : !deployments ||
          deployments.length === 0 ? (
          <EmptyState
            icon={Rocket}
            title="Nothing deployed yet"
            description="Once a project deploys, its release history and logs show up here."
          />
        ) : (
          <div className="card">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-white/[0.06] text-left text-[11px] uppercase tracking-wider text-zinc-500">
                  <th className="sticky top-0 z-10 rounded-tl-xl bg-[#0c0c0e]/90 px-4 py-3 font-medium backdrop-blur">
                    Project
                  </th>
                  <th className="sticky top-0 z-10 bg-[#0c0c0e]/90 px-4 py-3 font-medium backdrop-blur">
                    Release
                  </th>
                  <th className="sticky top-0 z-10 hidden bg-[#0c0c0e]/90 px-4 py-3 font-medium backdrop-blur md:table-cell">
                    Image
                  </th>
                  <th className="sticky top-0 z-10 bg-[#0c0c0e]/90 px-4 py-3 font-medium backdrop-blur">
                    Status
                  </th>
                  <th className="sticky top-0 z-10 rounded-tr-xl bg-[#0c0c0e]/90 px-4 py-3 text-right font-medium backdrop-blur">
                    When
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/[0.04]">
                {deployments.map((d, i) => (
                  <motion.tr
                    key={d.id}
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{ delay: Math.min(i, 12) * 0.025, duration: 0.2 }}
                    onClick={() =>
                      navigate({
                        to: "/projects/$id",
                        params: { id: d.project_id },
                      })
                    }
                    className="cursor-pointer transition-colors duration-150 hover:bg-white/[0.03]"
                  >
                    <td className="px-4 py-3 text-zinc-200">
                      {projectNames.get(d.project_id) ?? "—"}
                      {d.kind === "rollback" && (
                        <span className="ml-2 rounded border border-white/10 bg-white/[0.04] px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider text-zinc-400">
                          rollback
                        </span>
                      )}
                    </td>
                    <td className="px-4 py-3">
                      <span className="chip-mono">
                        {d.release_spec.release_id.slice(0, 8)}
                      </span>
                    </td>
                    <td className="hidden max-w-64 truncate px-4 py-3 font-mono text-[13px] text-zinc-500 md:table-cell">
                      {d.release_spec.image}
                    </td>
                    <td className="px-4 py-3">
                      <StatusPill status={d.status} />
                    </td>
                    <td className="whitespace-nowrap px-4 py-3 text-right text-xs text-zinc-600">
                      {timeAgo(d.created_at)}
                    </td>
                  </motion.tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
