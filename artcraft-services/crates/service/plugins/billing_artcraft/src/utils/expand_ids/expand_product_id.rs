use stripe_shared::Product;
use stripe_types::Expandable;

pub fn expand_product_id(expandable_product: &Expandable<Product>) -> String {
  match expandable_product {
    Expandable::Id(id) => id.to_string(),
    Expandable::Object(product) => product.id.to_string(),
  }
}
