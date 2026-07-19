import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { motion } from "motion/react";
import {
  ExternalLink,
  GitBranch,
  LayoutGrid,
  Package,
  Plus,
} from "lucide-react";
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
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Projects</h1>
          <p className="mt-1 text-sm text-zinc-500">
            Apps deployed to your own servers, live HTTPS URL included.
          </p>
        </div>
        <Link to="/projects/new" className="btn-primary shrink-0">
          <Plus className="h-4 w-4" strokeWidth={1.75} />
          New project
        </Link>
      </div>

      <div className="mt-8">
        {isLoading ? null : !projects || projects.length === 0 ? (
          <EmptyState
            icon={LayoutGrid}
            title="No projects yet"
            description="Point Projexity at a Docker image and it will run it on your server behind an HTTPS reverse proxy. Git deploys are coming next."
            action={
              <Link to="/projects/new" className="btn-primary">
                <Plus className="h-4 w-4" strokeWidth={1.75} />
                Create your first project
              </Link>
            }
          />
        ) : (
          <div className="grid gap-4 md:grid-cols-2">
            {projects.map((p, i) => (
              <motion.div
                key={p.id}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: i * 0.03, duration: 0.22, ease: "easeOut" }}
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

function ProjectCard({ project }: { project: Project }) {
  const navigate = useNavigate();
  const domain = project.domains[0];
  const SourceIcon = project.repo ? GitBranch : Package;
  const sourceLine =
    project.repo ?? project.image ?? "no image yet";
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
      className="card card-hover group cursor-pointer p-5"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h2 className="truncate font-medium tracking-tight text-zinc-100">
            {project.name}
          </h2>
          <p className="mt-1 flex items-center gap-1.5 truncate font-mono text-[13px] text-zinc-500">
            <SourceIcon
              className="h-3.5 w-3.5 shrink-0 text-zinc-600"
              strokeWidth={1.75}
            />
            <span className="truncate">{sourceLine}</span>
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
            className="group/link flex min-w-0 items-center gap-1 font-mono text-[13px] text-emerald-400 transition-colors hover:text-emerald-300"
          >
            <span className="truncate">{domain}</span>
            <ExternalLink
              className="h-3 w-3 shrink-0 opacity-0 transition-opacity group-hover/link:opacity-100"
              strokeWidth={1.75}
            />
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
