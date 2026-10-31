use crate::plans::model_weights::evaluate::asserts::assert_search_term_contains_titles;
use elasticsearch::Elasticsearch;
use errors::AnyhowResult;

pub async fn test_search_term_zelda(
  client: &Elasticsearch
) -> AnyhowResult<()> {

  assert_search_term_contains_titles(client, "zeld", &vec![
    "Link (Zelda CDI) (Français)",
    "Zelda (BOTW Español Latino)",
    "Zelda (BOTW) (Ita)",
    "Zelda (BOTW)",
    "Zelda (Breath of the Wild)",
    "Zelda Rubinstein",
    "Zelda. (The Legend of Zelda: Breath of the Wild, Castillian Spanish.) Versión MEJORADA.",
  ]).await?;

  assert_search_term_contains_titles(client, "zelad", &vec![
    "Link (Zelda CDI) (Français)",
    "Zelda (BOTW Español Latino)",
    "Zelda (BOTW) (Ita)",
    "Zelda (BOTW)",
    "Zelda (Breath of the Wild)",
    "Zelda Rubinstein",
    "Zelda. (The Legend of Zelda: Breath of the Wild, Castillian Spanish.) Versión MEJORADA.",
  ]).await?;

  Ok(())
}

