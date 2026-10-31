use std::process::Command;

use log::{debug, info};
use rand::prelude::IteratorRandom;
use rand::thread_rng;

use errors::AnyhowResult;

pub fn delete_pods(mut pod_names: Vec<String>, batch_size: usize) -> AnyhowResult<()> {
  // Randomly delete from the pods so several processes can be spun up at once (prior to
  // implementing multithreading here.)
  let mut batch : Vec<String>;

  while !pod_names.is_empty() {
    batch = Vec::with_capacity(batch_size);

    while !pod_names.is_empty() && batch.len() < batch_size {
      if let Some((index, element)) = pod_names.iter()
          .enumerate()
          .choose(&mut thread_rng()) {
        batch.push(element.to_string());
        pod_names.remove(index);
      }
    }

    delete_pod_batch(&batch)?;
  }

  Ok(())
}

pub fn delete_pod_batch(pod_names: &Vec<String>) -> AnyhowResult<()> {
  let mut args = Vec::from(["delete".to_string(), "pods".to_string()]);

  args.extend_from_slice(pod_names);

  info!("Deleting batch of {} pods", (args.len() - 2));

  let output = Command::new("kubectl")
      .args(args)
      .output()?;

  let stdout = String::from_utf8(output.stdout)?;

  debug!("Output: {}", stdout);

  Ok(())
}
