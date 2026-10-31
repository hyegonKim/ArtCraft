use errors::AnyhowResult;
use mysql_queries::payloads::generic_inference_args::generic_inference_args::{GenericInferenceArgs, PolymorphicInferenceArgs};
use mysql_queries::queries::generic_inference::web::job_status::GenericInferenceJobStatus;

pub fn extract_polymorphic_inference_args(job: &GenericInferenceJobStatus) -> AnyhowResult<Option<PolymorphicInferenceArgs>> {
  let maybe_args = job.request_details
      .maybe_inference_args
      .as_deref()
      .map(|args| GenericInferenceArgs::from_json(args))
      .transpose()?;

  let maybe_args = maybe_args
      .map(|args| args.args)
      .flatten();

  Ok(maybe_args)
}
