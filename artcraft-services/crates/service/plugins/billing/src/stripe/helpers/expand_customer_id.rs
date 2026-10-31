use stripe::{Customer, Expandable};

pub fn expand_customer_id(expandable_customer: &Expandable<Customer>) -> String {
  match expandable_customer {
    Expandable::Id(id) => id.to_string(),
    Expandable::Object(customer) => customer.id.to_string(),
  }
}
