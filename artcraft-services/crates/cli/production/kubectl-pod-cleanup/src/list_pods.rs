use std::ops::Index;
use std::process::Command;

use log::info;
use once_cell::sync::Lazy;
use regex::Regex;

use errors::AnyhowResult;

/// Information about pods in the cluster
/// Obtained by `kubectl get pods`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PodInfo {
  pub name: String,
  pub ready: String,
  pub status: String,
  pub restarts: String,
  pub age: String,
}

pub fn list_pods() -> AnyhowResult<Vec<PodInfo>> {
  info!("kube-pod-cleanup");

  let output = Command::new("kubectl")
      .args(["get", "pods"])
      .output()?;

  let stdout = String::from_utf8(output.stdout)?;

  parse_pod_status_output(&stdout)
}

const LIST_POD_REGEX : Lazy<Regex> = Lazy::new(|| {
  Regex::new(r"(\S+)\s+(\S+)\s+(\S+)\s+((\d+\s\(\S+ ago\))|(\S+))\s+(\S+).*")
      .expect("regex should parse")
});

fn parse_pod_status_output(stdout: &str) -> AnyhowResult<Vec<PodInfo>> {
  // Parse `kubectl get pods` lines
  let regex = LIST_POD_REGEX; // NB: To pass lint on const interior mutability (???)

  let lines = stdout.split("\n")
      .into_iter()
      .map(|line| line.trim().to_string())
      .collect::<Vec<String>>();

  Ok(lines.iter()
      .filter_map(|line| {
        regex.captures(line)
      })
      .skip(1) // NB: We skip the first line as that's the header.
      .map(|captures| {
        let name = captures.index(1);
        let ready = captures.index(2);
        let status = captures.index(3);
        let restarts = captures.index(4);
        let age = captures.index(7);
        PodInfo {
          name: name.to_string(),
          ready: ready.to_string(),
          status: status.to_string(),
          restarts: restarts.to_string(),
          age: age.to_string(),
        }
      })
      .collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
  use crate::list_pods::{parse_pod_status_output, PodInfo};

  #[test]
  fn test_parse() {
    let example_output_lines = r#"
NAME                               READY   STATUS                   RESTARTS        AGE
analytics-job-b8cbf88bd-c5ghc      1/1     Running                  0               90d
storyteller-web-8566f9bdc5-2kv6v   1/1     Running                  4 (2d12h ago)   4d23h
storyteller-web-8566f9bdc5-2fzlt   0/1     ContainerStatusUnknown   1               6d12h
storyteller-web-8566f9bdc5-247d4   0/1     Error                    0               6d12h
storyteller-web-8566f9bdc5-25lw6   0/1     Evicted                  0               2d16h
    "#;

    let pods = parse_pod_status_output(example_output_lines)
        .expect("should parse");

    assert_eq!(pods.get(0).unwrap().clone(), PodInfo {
      name: "analytics-job-b8cbf88bd-c5ghc".to_string(),
      ready: "1/1".to_string(),
      status: "Running".to_string(),
      restarts: "0".to_string(),
      age: "90d".to_string(),
    });

    assert_eq!(pods.get(1).unwrap().clone(), PodInfo {
      name: "storyteller-web-8566f9bdc5-2kv6v".to_string(),
      ready: "1/1".to_string(),
      status: "Running".to_string(),
      restarts: "4 (2d12h ago)".to_string(),
      age: "4d23h".to_string(),
    });

    assert_eq!(pods.get(2).unwrap().clone(), PodInfo {
      name: "storyteller-web-8566f9bdc5-2fzlt".to_string(),
      ready: "0/1".to_string(),
      status: "ContainerStatusUnknown".to_string(),
      restarts: "1".to_string(),
      age: "6d12h".to_string(),
    });
  }
}
