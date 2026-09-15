//! The Lumen sandbox egress proxy.
//!
//! Port of `backend/lumen/sandbox_proxy/`, in progress. The proxy sits between
//! every sandbox and the network: it terminates TLS, resolves which sandbox a
//! connection came from, injects credentials the sandbox never sees, and gates
//! actions against policy.
//!
//! Ported so far: the CA bootstrap and its file-backed store, and the action
//! matching engine the gate decides with. See `backend/native/VERIFICATION.md`
//! for what is proven and what is not.

pub mod ca;
pub mod ca_file;
pub mod catalog;
pub mod matching;

pub use ca::{CaBootstrap, CaError, CaStore, MaterializedCa};
pub use ca_file::FileCaStore;
pub use matching::{
    actions_requiring_approval, apply_credential_gate, recognize_actions, AllMatchedActions,
    EndpointPolicy, EndpointSpec, GatedAppKind, GatedTarget, MatchedAction, ProxiedRequest,
};
