use std::thread;
use std::time::{Duration, Instant};

use log::{error, info};

use errors::{anyhow, AnyhowResult};

use crate::delete_pods::delete_pod_batch;
use crate::list_pods::list_pods;
use crate::pod_store::PodStore;

/// If kubectl returns faster than this, it may be spamming the API.
const TIMEOUT_TRIGGER_THRESHOLD : Duration = Duration::from_millis(500);

const TIMEOUT_DURATION : Duration = Duration::from_secs(5);

/// If we fail to delete pods this number of times, exit the thread.
const FAILURE_TRIGGER_THRESHOLD : usize = 5;

const POD_STATUSES_TO_KEEP : [&str; 2] = [
  "ContainerCreating",
  "Running",
];

const POD_STATUSES_TO_ALWAYS_CLEAR : [&str; 18] = [
  "Completed",
  "ContainerStatusUnknown", // TODO: Maybe don't include?
  "CrashLoopBackOff",
  "ErrImagePull",
  "Error",
  "Evicted",
  "Init:ContainerStatusUnknown", // TODO: Maybe don't include?
  "Init:ErrImagePull",
  "Init:Error",
  "Init:ImagePullBackOff",
  "Init:OOMKilled",
  "NodeAffinity",
  "NodeShutdown",
  "OOMKilled",
  "OutOfnvidia.com/gpu",
  "Terminated",
  "Terminating",
  "UnexpectedAdmissionError",
];

const POD_STATUSES_TO_SOMETIMES_CLEAR : [&str; 5] = [
  "Init:1/2",
  "Init:1/3",
  "Init:2/3",
  "Pending",
  "PodInitializing",
  //"ContainerStatusUnknown",
  //"Init:0/2",
  //"Init:ContainerStatusUnknown",
];


pub fn delete_pods_threaded(num_tasks: usize, batch_size: usize) -> AnyhowResult<()> {
  let pods_to_delete = list_pods_to_delete()?;
  let pod_store = PodStore::from_names(pods_to_delete);

  let mut join_handles = Vec::with_capacity(batch_size);

  for _ in 0 ..num_tasks {
    let pod_store_cloned = pod_store.clone();

    // NB: Using std::thread instead of Tokio since this API is blocking.
    let handle = thread::spawn(move || {
      delete_pod_thread(pod_store_cloned, batch_size);
    });

    join_handles.push(handle);
  }

  for handle in join_handles {
    if let Err(err) = handle.join() {
      return Err(anyhow!("threading error: {:?}", err));
    }
  }

  Ok(())
}

fn list_pods_to_delete() -> AnyhowResult<Vec<String>> {
  let all_pods = list_pods()?;

  let pods_to_delete = all_pods.iter()
      .filter(|pod| !POD_STATUSES_TO_KEEP.contains(&pod.status.as_str()))
      .filter(|pod| {
        POD_STATUSES_TO_ALWAYS_CLEAR.contains(&pod.status.as_str()) ||
            POD_STATUSES_TO_SOMETIMES_CLEAR.contains(&pod.status.as_str())
      })
      .collect::<Vec<_>>();

  let pod_names_to_delete = pods_to_delete.iter()
      .map(|pod| pod.name.to_string())
      .collect::<Vec<_>>();

  Ok(pod_names_to_delete)
}

fn delete_pod_thread(pod_store: PodStore, batch_size: usize) {
  let mut failure_to_progress_count = 0;

  loop {
    let batch = match pod_store.grab_batch(batch_size) {
      Ok(batch) => batch,
      Err(err) => {
        error!("threading error: {:?}", err);
        return;
      }
    };

    if batch.is_empty() {
      return;
    }

    if batch.len() < batch_size {
      failure_to_progress_count += 1;
    } else {
      failure_to_progress_count = 0;
    }

    if failure_to_progress_count > FAILURE_TRIGGER_THRESHOLD {
      error!("Failure to make progress.");
      return;
    }

    let pod_names = batch.into_iter()
        .map(|n| n.to_string())
        .collect::<Vec<String>>();

    let start = Instant::now();

    if let Err(err) = delete_pod_batch(&pod_names) {
      error!("threading error: {:?}", err);
      return;
    }

    let duration = Instant::now().duration_since(start);

    if duration.lt(&TIMEOUT_TRIGGER_THRESHOLD) {
      thread::sleep(TIMEOUT_DURATION);
    }
  }
}

fn reload_pods_thread(pod_store: PodStore) -> AnyhowResult<()> {
  loop {
    //info!("Clearing {} / {} pods...", pods_to_clear.len(), all_pods.len());

    let pods = list_pods_to_delete()?;
    info!("{} pods to delete", pods.len());

    pod_store.replace_pods(pods)?;

    thread::sleep(Duration::from_secs(20));
  }
}
