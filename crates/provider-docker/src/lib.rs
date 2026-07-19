//! Docker-on-VPS deploy provider (M1–M2).
//!
//! Architecture (see docs/): agentless SSH transport — the `openssh` crate
//! multiplexes one master connection per server and forwards the remote
//! `/var/run/docker.sock` to a local unix socket that `bollard` connects to.
//! Everything above the [`transport`] module is forbidden from knowing SSH
//! exists, so a future outbound-dialing agent (NAT'd home labs) is a drop-in
//! replacement for `NodeChannel`.
//!
//! Deploy choreography: preflight → ensure image → start green container on
//! the `projexity` network → health gate → atomic Caddy cutover (full
//! desired-state config `POST /load` on the SSH-forwarded admin API) → verify
//! through the front door → drain and retire blue (keep its image; that image
//! is the rollback).

pub mod bootstrap;
pub mod docker;
pub mod preflight;
pub mod transport;

/// Labels stamped on every resource we create. The reconciler only ever
/// touches resources carrying [`LABEL_MANAGED`] — users run other things on
/// these boxes.
pub const LABEL_MANAGED: &str = "projexity.managed";
pub const LABEL_APP: &str = "projexity.app";
pub const LABEL_RELEASE: &str = "projexity.release";

/// Name of the shared docker network apps and Caddy join.
pub const NETWORK_NAME: &str = "projexity";

/// Container name of the managed Caddy proxy.
pub const CADDY_CONTAINER: &str = "pjx-caddy";
