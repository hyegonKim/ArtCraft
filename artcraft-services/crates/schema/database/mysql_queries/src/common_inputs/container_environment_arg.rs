
/// Equivalent of the `bootstrap` crate's ContainerEnvironment, but without introducing a dependency.
/// A lot of job systems use this as an argument, so it can be converted, cached, and reused.
pub struct ContainerEnvironmentArg {
  pub hostname: String,
  pub cluster_name: String,
}
