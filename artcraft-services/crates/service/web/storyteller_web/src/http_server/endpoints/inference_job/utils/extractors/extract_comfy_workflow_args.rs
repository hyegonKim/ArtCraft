use anyhow::anyhow;

use errors::AnyhowResult;
use mysql_queries::payloads::generic_inference_args::generic_inference_args::PolymorphicInferenceArgs;
use mysql_queries::payloads::generic_inference_args::inner_payloads::workflow_payload::WorkflowArgs;

pub fn extract_comfy_workflow_args(args: &PolymorphicInferenceArgs) -> Option<WorkflowArgs> {
  extract_comfy_workflow_args_fallible(args).ok().flatten()
}

pub fn extract_comfy_workflow_args_fallible(args: &PolymorphicInferenceArgs) -> AnyhowResult<Option<WorkflowArgs>> {
  match args {
    PolymorphicInferenceArgs::Cu(workflow_args) => Ok(Some(workflow_args.clone())),
    _ => return Err(anyhow!("wrong inner args for job!")),
  }
}
