import {
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
} from "@tanstack/react-router";
import { AppLayout } from "./layout/AppLayout";
import { LoginPage } from "./pages/Login";
import { ProjectsPage } from "./pages/Projects";
import { TargetsPage } from "./pages/Targets";
import { ConnectServerPage } from "./pages/ConnectServer";
import { DeploymentsPage } from "./pages/Deployments";
import { NewProjectPage } from "./pages/NewProject";
import { ProjectDetailPage } from "./pages/ProjectDetail";

const rootRoute = createRootRoute({
  component: () => <Outlet />,
});

const loginRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/login",
  component: LoginPage,
});

const appRoute = createRoute({
  getParentRoute: () => rootRoute,
  id: "app",
  component: AppLayout,
});

const projectsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/",
  component: ProjectsPage,
});

const newProjectRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/projects/new",
  component: NewProjectPage,
});

const projectDetailRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/projects/$id",
  component: ProjectDetailRouteComponent,
});

function ProjectDetailRouteComponent() {
  const { id } = projectDetailRoute.useParams();
  return <ProjectDetailPage id={id} />;
}

const targetsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/targets",
  component: TargetsPage,
});

const connectServerRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/targets/new",
  component: ConnectServerPage,
});

const deploymentsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/deployments",
  component: DeploymentsPage,
});

const routeTree = rootRoute.addChildren([
  loginRoute,
  appRoute.addChildren([
    projectsRoute,
    newProjectRoute,
    projectDetailRoute,
    targetsRoute,
    connectServerRoute,
    deploymentsRoute,
  ]),
]);

export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
