import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { motion } from "motion/react";
import { api, isDeploymentActive, type Project } from "../api";
import { EmptyState } from "../components/EmptyState";
import { StatusPill } from "../components/StatusPill";
import { timeAgo } from "../time";

export function ProjectsPage() {
  const { data: projects, isLoading } = useQuery({
    queryKey: ["projects"],
    queryFn: api.listProjects,
    refetchInterval: (q) =>
      q.state.data?.some((p) => isDeploymentActive(p.latest_deployment))
        ? 2500
        : false,
  });

  return (
    <div>
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">Projects</h1>
          <p className="mt-1 text-sm text-zinc-500">
            Apps deployed to your own servers, live HTTPS URL included.
          </p>
        </div>
        <Link
          to="/projects/new"
          className="rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500"
        >
          New project
        </Link>
      </div>

      <div className="mt-8">
        {isLoading ? null : !projects || projects.length === 0 ? (
          <EmptyStateWithLink />
        ) : (
          <div className="grid gap-4 md:grid-cols-2">
            {projects.map((p, i) => (
              <motion.div
                key={p.id}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: i * 0.04, duration: 0.25, ease: "easeOut" }}
              >
                <ProjectCard project={p} />
              </motion.div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function EmptyStateWithLink() {
  return (
    <div className="relative">
      <EmptyState
        title="No projects yet"
        description="Point Projexity at a Docker image and it will run it on your server behind an HTTPS reverse proxy. Git deploys are coming next."
      />
      <div className="absolute inset-x-0 bottom-16 flex justify-center">
        <Link
          to="/projects/new"
          className="rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500"
        >
          Create your first project
        </Link>
      </div>
    </div>
  );
}

function ProjectCard({ project }: { project: Project }) {
  const navigate = useNavigate();
  const domain = project.domains[0];
  return (
    <div
      role="link"
      tabIndex={0}
      onClick={() =>
        navigate({ to: "/projects/$id", params: { id: project.id } })
      }
      onKeyDown={(e) => {
        if (e.key === "Enter")
          navigate({ to: "/projects/$id", params: { id: project.id } });
      }}
      className="cursor-pointer rounded-xl border border-zinc-800 bg-zinc-900/40 p-5 transition hover:border-zinc-700"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h2 className="truncate font-medium text-zinc-100">{project.name}</h2>
          <p className="mt-0.5 truncate font-mono text-sm text-zinc-500">
            {project.image ?? "no image yet"}
          </p>
        </div>
        <StatusPill status={project.latest_deployment?.status ?? null} />
      </div>
      <div className="mt-4 flex items-center justify-between gap-3 text-sm">
        {domain ? (
          <a
            href={`https://${domain}`}
            target="_blank"
            rel="noreferrer"
            onClick={(e) => e.stopPropagation()}
            className="truncate text-emerald-400 hover:underline"
          >
            {domain}
          </a>
        ) : (
          <span className="text-zinc-600">no domain yet</span>
        )}
        <span className="shrink-0 text-xs text-zinc-600">
          created {timeAgo(project.created_at)}
        </span>
      </div>
    </div>
  );
}
