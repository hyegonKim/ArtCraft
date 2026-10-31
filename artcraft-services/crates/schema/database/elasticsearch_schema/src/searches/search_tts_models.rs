use elasticsearch::{Elasticsearch, SearchParts};
use serde_json::{json, Value};

use errors::AnyhowResult;

use crate::documents::tts_model_document::{TTS_MODEL_INDEX, TtsModelDocument};

pub async fn search_tts_models(
  client: &Elasticsearch,
  search_term: &str,
  maybe_language_subtag: Option<&str>,
) -> AnyhowResult<Vec<TtsModelDocument>> {

  let search_json = match maybe_language_subtag {
    None => query_tts_models(search_term),
    Some(language) => query_tts_models_with_required_language(search_term, language)
  };

  let search_response = client
      .search(SearchParts::Index(&[TTS_MODEL_INDEX]))
      .body(search_json)
      .allow_no_indices(true)
      .send()
      .await?;

  let _status_code = search_response.status_code();

  let mut response_json = search_response.json::<Value>().await?;

  let hits = response_json.get_mut("hits")
      .map(|hits| hits.take());

  let hits = hits.map(|mut hits| {
    hits.get_mut("hits")
        .map(|hits| hits.take())
  }).flatten();

  let mut documents = Vec::new();

  match hits {
    Some(Value::Array(inner_hits)) => {
      for mut hit in inner_hits {
        let maybe_object = hit.get_mut("_source")
            .map(|source| source.take());
        if let Some(value) = maybe_object {
          let document = serde_json::from_value::<TtsModelDocument>(value)?;
          documents.push(document);
        }
      }
    }
    _ => {},
  }

  Ok(documents)
}

fn query_tts_models(search_term: &str) -> Value {
  json!({
    "query": {
      "bool": {
        "must": [
          {
            "bool": {
              "should": [
                {
                  "fuzzy": {
                    "title": {
                      "value": search_term,
                      "fuzziness": 2
                    }
                  }
                },
                {
                  "match": {
                    "title": {
                      "query": search_term,
                      "boost": 1
                    }
                  }
                },
                {
                  "multi_match": {
                    "query": search_term,
                    "type": "bool_prefix",
                    "fields": [
                      "title",
                      "title._2gram",
                      "title._3gram"
                    ],
                    "boost": 50
                  }
                }
              ]
            }
          }
        ]
      }
    }
  })
}

fn query_tts_models_with_required_language(search_term: &str, language_tag: &str) -> Value {
  json!({
    "query": {
      "bool": {
        "must": [
          {
            "match": {
              "ietf_primary_language_subtag": language_tag
            }
          },
          {
            "bool": {
              "should": [
                {
                  "fuzzy": {
                    "title": {
                      "value": search_term,
                      "fuzziness": 2
                    }
                  }
                },
                {
                  "match": {
                    "title": {
                      "query": search_term,
                      "boost": 1
                    }
                  }
                },
                {
                  "multi_match": {
                    "query": search_term,
                    "type": "bool_prefix",
                    "fields": [
                      "title",
                      "title._2gram",
                      "title._3gram"
                    ],
                    "boost": 50
                  }
                }
              ]
            }
          }
        ]
      }
    }
  })
}
